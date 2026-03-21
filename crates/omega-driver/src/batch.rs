/// Batch file processing: parse a .omega file and process all commands.
use omega_syntax::desugar::{desugar_program, Command};
use omega_syntax::parser;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

use crate::commands::process_command;
use crate::session::Session;

/// Process a source file.
pub fn process_file(session: &mut Session, source: &str, filename: &str) -> Result<Vec<String>, String> {
    let sexps = parser::parse(source).map_err(|e| format!("{}:{}", filename, e))?;
    let commands = desugar_program(&sexps).map_err(|e| format!("{}:{}", filename, e))?;

    let mut results = Vec::new();
    for cmd in commands {
        match process_command(session, cmd) {
            Ok(msg) => results.push(msg),
            Err(e) => return Err(e),
        }
    }

    Ok(results)
}

/// Process a file from a path.
pub fn process_file_path(session: &mut Session, path: &str) -> Result<Vec<String>, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path, e))?;
    process_file(session, &source, path)
}

/// A single result entry in JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct ResultEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub status: String, // "valid", "invalid", "timeout"
    pub message: Option<String>,
}

/// Top-level JSON output (unified schema).
#[derive(Debug, Clone, Serialize)]
pub struct JsonOutput {
    pub status: String, // "success", "failure", "timeout"
    pub elapsed_ms: f64,
    pub results: Vec<ResultEntry>,
    pub discoveries: Vec<serde_json::Value>, // always [] for Omega
}

/// Extract node ID annotations from source comments.
/// Format: `;; @node axiom:assoc` on the line before a command.
fn extract_node_annotations(source: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut pending_node_id: Option<String> = None;

    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(";;").or_else(|| trimmed.strip_prefix(";")) {
            let rest = rest.trim();
            if let Some(node_id) = rest.strip_prefix("@node ") {
                pending_node_id = Some(node_id.trim().to_string());
            }
        } else if !trimmed.is_empty() && trimmed.starts_with('(') {
            if let Some(node_id) = pending_node_id.take() {
                let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                if tokens.len() >= 2 {
                    let name = tokens[1].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
                    map.insert(name.to_string(), node_id);
                }
            }
        } else if !trimmed.is_empty() {
            pending_node_id = None;
        }
    }
    map
}

/// Process a file and return structured JSON results.
pub fn process_file_json(session: &mut Session, source: &str, filename: &str) -> JsonOutput {
    let start = Instant::now();
    let node_map = extract_node_annotations(source);

    let sexps = match parser::parse(source) {
        Ok(s) => s,
        Err(e) => {
            return JsonOutput {
                status: "failure".into(),
                elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                results: vec![ResultEntry {
                    name: filename.to_string(),
                    node_id: None,
                    status: "invalid".into(),
                    message: Some(format!("parse error: {}", e)),
                }],
                discoveries: vec![],
            };
        }
    };

    let commands = match desugar_program(&sexps) {
        Ok(c) => c,
        Err(e) => {
            return JsonOutput {
                status: "failure".into(),
                elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                results: vec![ResultEntry {
                    name: filename.to_string(),
                    node_id: None,
                    status: "invalid".into(),
                    message: Some(format!("desugar error: {}", e)),
                }],
                discoveries: vec![],
            };
        }
    };

    let mut results = Vec::new();
    let mut had_error = false;

    for cmd in commands {
        let (_cmd_type, cmd_name, _default_node_id) = extract_command_info(&cmd);
        let node_id = node_map.get(&cmd_name).cloned();

        match process_command(session, cmd) {
            Ok(msg) => {
                results.push(ResultEntry {
                    name: cmd_name,
                    node_id,
                    status: "valid".into(),
                    message: if msg.contains("registered OK") { None } else { Some(msg) },
                });
            }
            Err(e) => {
                had_error = true;
                let status = if e.contains("fuel") || e.contains("Fuel") || e.contains("timeout") || e.contains("Timeout") {
                    "timeout"
                } else {
                    "invalid"
                };
                results.push(ResultEntry {
                    name: cmd_name,
                    node_id,
                    status: status.into(),
                    message: Some(e),
                });
            }
        }
    }

    let has_timeout = results.iter().any(|r| r.status == "timeout");

    JsonOutput {
        status: if had_error {
            if has_timeout { "timeout" } else { "failure" }
        } else {
            "success"
        }.into(),
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        results,
        discoveries: vec![],
    }
}

/// Process a file path in JSON mode.
pub fn process_file_path_json(session: &mut Session, path: &str) -> JsonOutput {
    match std::fs::read_to_string(path) {
        Ok(source) => process_file_json(session, &source, path),
        Err(e) => JsonOutput {
            status: "failure".into(),
            elapsed_ms: 0.0,
            results: vec![ResultEntry {
                name: path.to_string(),
                node_id: None,
                status: "invalid".into(),
                message: Some(format!("cannot read {}: {}", path, e)),
            }],
            discoveries: vec![],
        },
    }
}

/// Extract command type, name, and optional node_id from a Command.
fn extract_command_info(cmd: &Command) -> (String, String, Option<String>) {
    match cmd {
        Command::TheoryDef(builder) => {
            ("theory".into(), builder.name().to_string(), Some(format!("theory:{}", builder.name())))
        }
        Command::CheckTheory(name) => ("check_theory".into(), name.clone(), None),
        Command::Proof { name, .. } => {
            ("proof".into(), name.clone(), Some(format!("proof:{}", name)))
        }
        Command::TacticProof { name, .. } => {
            ("tactic_proof".into(), name.clone(), Some(format!("proof:{}", name)))
        }
        Command::MetaTheoremDef(mt) => {
            ("metatheorem".into(), mt.name.to_string(), Some(format!("metatheorem:{}", mt.name)))
        }
        Command::Reflect { rule_name, .. } => {
            ("reflect".into(), rule_name.clone(), Some(format!("reflect:{}", rule_name)))
        }
        Command::Lemma { name, .. } => {
            ("lemma".into(), name.clone(), Some(format!("lemma:{}", name)))
        }
        Command::TacticLemma { name, .. } => {
            ("tactic_lemma".into(), name.clone(), Some(format!("lemma:{}", name)))
        }
        Command::Emit { theory, .. } => ("emit".into(), theory.clone(), None),
        Command::Refute { name, .. } => {
            ("refute".into(), name.clone(), Some(format!("refute:{}", name)))
        }
    }
}

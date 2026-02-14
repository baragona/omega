/// Interactive REPL.
use omega_syntax::desugar::desugar_program;
use omega_syntax::parser;
use omega_syntax::printer;

use crate::batch;
use crate::commands::process_command;
use crate::session::Session;

/// Run the REPL using a generic line reader (to avoid hard dependency on rustyline in this crate).
pub fn run_repl<R: LineReader>(session: &mut Session, reader: &mut R) -> Result<(), String> {
    println!("Omega Logical Framework v0.1.0");
    println!("Type :help for available commands, :quit to exit.\n");

    loop {
        let line = match reader.read_line("omega> ") {
            Some(line) => line,
            None => break, // EOF
        };

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Handle REPL commands
        if line.starts_with(':') {
            match handle_repl_command(session, line) {
                ReplAction::Continue => continue,
                ReplAction::Quit => break,
                ReplAction::Load(path) => {
                    match batch::process_file_path(session, &path) {
                        Ok(results) => {
                            for r in results {
                                println!("{}", r);
                            }
                        }
                        Err(e) => eprintln!("Error: {}", e),
                    }
                    continue;
                }
            }
        }

        // Otherwise, parse and execute as Omega source
        match parser::parse(line) {
            Err(e) => eprintln!("Parse error: {}", e),
            Ok(sexps) => match desugar_program(&sexps) {
                Err(e) => eprintln!("Error: {}", e),
                Ok(commands) => {
                    for cmd in commands {
                        match process_command(session, cmd) {
                            Ok(msg) => println!("{}", msg),
                            Err(e) => eprintln!("Error: {}", e),
                        }
                    }
                }
            },
        }
    }

    println!("Goodbye.");
    Ok(())
}

/// Trait for reading lines (abstraction over rustyline/stdin).
pub trait LineReader {
    fn read_line(&mut self, prompt: &str) -> Option<String>;
}

/// A simple stdin-based line reader.
pub struct StdinReader;

impl LineReader for StdinReader {
    fn read_line(&mut self, prompt: &str) -> Option<String> {
        use std::io::Write;
        print!("{}", prompt);
        std::io::stdout().flush().ok()?;
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some(line),
            Err(_) => None,
        }
    }
}

enum ReplAction {
    Continue,
    Quit,
    Load(String),
}

fn handle_repl_command(session: &Session, line: &str) -> ReplAction {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd = parts[0];
    let arg = parts.get(1).map(|s| s.trim());

    match cmd {
        ":quit" | ":q" => ReplAction::Quit,
        ":help" | ":h" => {
            println!("Available commands:");
            println!("  :load <file>    Load an .omega file");
            println!("  :theory <name>  Show theory details");
            println!("  :rules <name>   Show rules for a theory");
            println!("  :theories       List all loaded theories");
            println!("  :proofs         List all verified proofs");
            println!("  :help           Show this help");
            println!("  :quit           Exit the REPL");
            println!();
            println!("Or enter Omega source directly (theory, proof, etc.)");
            ReplAction::Continue
        }
        ":load" | ":l" => {
            if let Some(path) = arg {
                ReplAction::Load(path.to_string())
            } else {
                eprintln!(":load requires a file path");
                ReplAction::Continue
            }
        }
        ":theory" => {
            if let Some(name) = arg {
                if let Some(theory) = session.kernel.get_theory(name) {
                    println!("{}", printer::print_theory_summary(theory));
                } else {
                    eprintln!("Unknown theory: {}", name);
                }
            } else {
                eprintln!(":theory requires a name");
            }
            ReplAction::Continue
        }
        ":rules" => {
            if let Some(name) = arg {
                if let Some(theory) = session.kernel.get_theory(name) {
                    for rule in theory.rules() {
                        println!("{}", printer::print_rule(rule));
                        println!();
                    }
                } else {
                    eprintln!("Unknown theory: {}", name);
                }
            } else {
                eprintln!(":rules requires a theory name");
            }
            ReplAction::Continue
        }
        ":theories" => {
            let names = session.kernel.theory_names();
            if names.is_empty() {
                println!("No theories loaded.");
            } else {
                for name in names {
                    println!("  {}", name);
                }
            }
            ReplAction::Continue
        }
        ":proofs" => {
            if session.proven.is_empty() {
                println!("No proofs verified.");
            } else {
                for p in &session.proven {
                    println!("  {} (in {}): {}", p.name, p.theory, p.goal);
                }
            }
            ReplAction::Continue
        }
        _ => {
            eprintln!("Unknown command: {}", cmd);
            ReplAction::Continue
        }
    }
}

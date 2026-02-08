/// Batch file processing: parse a .omega file and process all commands.
use omega_syntax::desugar::desugar_program;
use omega_syntax::parser;

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

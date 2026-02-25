pub mod rust_ast;
pub mod analyze;
pub mod emit;

use std::fs;
use std::path::Path;

use crate::error::{HyperionError, Result};
use crate::session::HyperionSession;

/// Compile a Von Neumann theory to Rust. Returns number of files written.
pub fn kompile(session: &HyperionSession, theory_name: &str, output_dir: &str) -> Result<usize> {
    let vn_theory = session.vn_theories.get(theory_name).ok_or_else(|| {
        HyperionError::Undefined {
            kind: "VonNeumann Theory".into(),
            name: theory_name.to_string(),
        }
    })?;

    let krate = analyze::analyze(vn_theory)?;
    let files = emit::emit_crate(&krate);

    let out_path = Path::new(output_dir);
    fs::create_dir_all(out_path.join("src")).map_err(|e| HyperionError::ParseError {
        block: "kompile".into(),
        detail: format!("failed to create output directory: {}", e),
    })?;

    let mut count = 0;
    for (rel_path, content) in &files {
        let full_path = out_path.join(rel_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).map_err(|e| HyperionError::ParseError {
                block: "kompile".into(),
                detail: format!("failed to create directory: {}", e),
            })?;
        }
        fs::write(&full_path, content).map_err(|e| HyperionError::ParseError {
            block: "kompile".into(),
            detail: format!("failed to write {}: {}", rel_path, e),
        })?;
        count += 1;
    }

    Ok(count)
}

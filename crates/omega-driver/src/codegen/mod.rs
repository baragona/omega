/// Theory-to-Rust compiler: `omega kompile`.
///
/// Transforms a verified Omega theory into a complete Rust crate:
/// sorts → enums, rewrite rules → match functions, effects → traits.
pub mod rust_ast;
pub mod analyze;
pub mod emit;

use crate::session::Session;

/// Compile a theory to a Rust crate, writing files to `output_dir`.
/// Returns the number of files written.
pub fn kompile(
    session: &Session,
    theory_name: &str,
    output_dir: &str,
) -> Result<usize, String> {
    let theory = session
        .kernel
        .get_theory(theory_name)
        .ok_or_else(|| format!("kompile: unknown theory '{}'", theory_name))?;

    let krate = analyze::analyze(theory);
    let files = emit::emit_crate(&krate);

    // Write files to disk
    let base = std::path::Path::new(output_dir);
    for (path, content) in &files {
        let full_path = base.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("kompile: cannot create directory: {}", e))?;
        }
        std::fs::write(&full_path, content)
            .map_err(|e| format!("kompile: cannot write {}: {}", path, e))?;
    }

    Ok(files.len())
}

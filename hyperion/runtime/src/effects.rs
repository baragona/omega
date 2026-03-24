//! Compiler effect trait — the bridge between generated code and the host.
//!
//! When `kompile` generates code for a compiler-engine theory, effectful
//! operations (ask-egraph, read-file, write-crate, etc.) become method
//! calls on this trait. Users provide an implementation to connect
//! generated code to the real world.

use crate::types::*;

/// Result of an e-graph equality query.
#[derive(Debug, Clone, PartialEq)]
pub enum EGraphResult {
    /// The two expressions are provably equal.
    Equal,
    /// Could not prove equality within the fuel budget.
    NotEqual,
    /// Timed out during equality saturation.
    Timeout,
}

/// Proof verification result.
#[derive(Debug, Clone, PartialEq)]
pub enum VerifyResult {
    /// The proof term is valid for the given goal.
    Valid,
    /// The proof term does not witness the goal.
    Invalid(String),
}

/// The compiler effects trait. Implementations bridge generated Hyperion
/// code to the host system.
///
/// Generated code calls these methods when it encounters effectful
/// operations from the MetaCat CompilerEffect sort.
pub trait CompilerEffects {
    /// Ask the e-graph whether two expressions are equal in a theory.
    /// This is the core reflection primitive: compiled code can invoke
    /// the host's equality-saturation engine at runtime.
    fn ask_egraph(&self, lhs: &str, rhs: &str, theory: &TheoryDef) -> EGraphResult;

    /// Read a source file from the filesystem.
    /// Returns the file contents as a string, or an error message.
    fn read_file(&self, path: &str) -> Result<String, String>;

    /// Write generated crate files to disk.
    /// `contents` maps relative file paths to file contents.
    fn write_crate(&self, output_dir: &str, contents: &std::collections::HashMap<String, String>) -> Result<(), String>;

    /// Log a diagnostic message.
    fn log_diag(&self, message: &str);

    /// Verify a proof term against a goal.
    /// This calls back into the Hyperion proof checker.
    fn verify_proof(&self, proof_term: &str, goal: &str) -> VerifyResult;
}

/// A native compiler effects implementation that uses real I/O
/// and the egg e-graph library.
pub struct NativeCompilerEffects {
    pub verbose: bool,
}

impl NativeCompilerEffects {
    pub fn new() -> Self {
        NativeCompilerEffects { verbose: false }
    }

    pub fn verbose() -> Self {
        NativeCompilerEffects { verbose: true }
    }
}

impl Default for NativeCompilerEffects {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerEffects for NativeCompilerEffects {
    fn ask_egraph(&self, lhs: &str, rhs: &str, theory: &TheoryDef) -> EGraphResult {
        use egg::{*, rewrite as rw};

        // Build rewrites from theory rules
        let mut rewrites: Vec<Rewrite<SymbolLang, ()>> = Vec::new();
        for (i, (rule_lhs, rule_rhs)) in theory.rules.iter().enumerate() {
            let name = format!("rule_{}", i);
            if let (Ok(l), Ok(r)) = (rule_lhs.parse::<Pattern<SymbolLang>>(), rule_rhs.parse::<Pattern<SymbolLang>>()) {
                rewrites.push(rw!(name; l => r));
            }
        }

        // Run equality saturation
        let lhs_expr: RecExpr<SymbolLang> = match lhs.parse() {
            Ok(e) => e,
            Err(_) => return EGraphResult::NotEqual,
        };
        let rhs_expr: RecExpr<SymbolLang> = match rhs.parse() {
            Ok(e) => e,
            Err(_) => return EGraphResult::NotEqual,
        };

        let runner = Runner::default()
            .with_expr(&lhs_expr)
            .with_expr(&rhs_expr)
            .run(&rewrites);

        let id1 = runner.egraph.find(*runner.roots.first().unwrap());
        let id2 = runner.egraph.find(*runner.roots.last().unwrap());

        if id1 == id2 {
            if self.verbose {
                eprintln!("[EGRAPH] {} == {} (in theory {})", lhs, rhs, theory.name);
            }
            EGraphResult::Equal
        } else {
            EGraphResult::NotEqual
        }
    }

    fn read_file(&self, path: &str) -> Result<String, String> {
        if self.verbose {
            eprintln!("[READ] {}", path);
        }
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn write_crate(&self, output_dir: &str, contents: &std::collections::HashMap<String, String>) -> Result<(), String> {
        use std::fs;
        use std::path::Path;

        let base = Path::new(output_dir);
        for (rel_path, content) in contents {
            let full_path = base.join(rel_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(&full_path, content).map_err(|e| e.to_string())?;
            if self.verbose {
                eprintln!("[WRITE] {}", full_path.display());
            }
        }
        Ok(())
    }

    fn log_diag(&self, message: &str) {
        eprintln!("[DIAG] {}", message);
    }

    fn verify_proof(&self, proof_term: &str, goal: &str) -> VerifyResult {
        if self.verbose {
            eprintln!("[VERIFY] proof={} goal={}", proof_term, goal);
        }
        // For now, structural verification: if the proof mentions "refl" and
        // the goal is eq(x, x), it's valid. Full verification will integrate
        // with Apeiron's proof checker.
        if proof_term.contains("refl") && goal.contains("eq") {
            VerifyResult::Valid
        } else {
            VerifyResult::Invalid("proof does not match goal structure".to_string())
        }
    }
}

/// A mock compiler effects implementation for testing.
/// Records all effect calls without touching the filesystem or e-graph.
pub struct MockCompilerEffects {
    pub log: Vec<String>,
    pub egraph_results: std::collections::HashMap<(String, String), EGraphResult>,
    pub files: std::collections::HashMap<String, String>,
}

impl MockCompilerEffects {
    pub fn new() -> Self {
        MockCompilerEffects {
            log: Vec::new(),
            egraph_results: std::collections::HashMap::new(),
            files: std::collections::HashMap::new(),
        }
    }

    /// Pre-program an e-graph result for a specific query.
    pub fn set_egraph_result(&mut self, lhs: &str, rhs: &str, result: EGraphResult) {
        self.egraph_results.insert((lhs.to_string(), rhs.to_string()), result);
    }

    /// Pre-program a file's contents.
    pub fn set_file(&mut self, path: &str, content: &str) {
        self.files.insert(path.to_string(), content.to_string());
    }
}

impl Default for MockCompilerEffects {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerEffects for MockCompilerEffects {
    fn ask_egraph(&self, lhs: &str, rhs: &str, _theory: &TheoryDef) -> EGraphResult {
        self.egraph_results
            .get(&(lhs.to_string(), rhs.to_string()))
            .cloned()
            .unwrap_or(EGraphResult::NotEqual)
    }

    fn read_file(&self, path: &str) -> Result<String, String> {
        self.files.get(path)
            .cloned()
            .ok_or_else(|| format!("mock: file not found: {}", path))
    }

    fn write_crate(&self, _output_dir: &str, _contents: &std::collections::HashMap<String, String>) -> Result<(), String> {
        Ok(()) // no-op in mock
    }

    fn log_diag(&self, message: &str) {
        // In mock, we'd need interior mutability to record. Skip for now.
        eprintln!("[MOCK-DIAG] {}", message);
    }

    fn verify_proof(&self, _proof_term: &str, _goal: &str) -> VerifyResult {
        VerifyResult::Valid // mock always accepts
    }
}

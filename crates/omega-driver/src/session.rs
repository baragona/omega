/// Session state: loaded theories, proven theorems, etc.
use omega_core::expr::Expr;
use omega_core::kernel::Kernel;

/// The session manages the kernel and tracks user interactions.
pub struct Session {
    pub kernel: Kernel,
    /// Named proofs that have been verified.
    pub proven: Vec<ProvenTheorem>,
    /// Whether to print verbose output.
    pub verbose: bool,
}

/// A theorem that has been proven and verified.
#[derive(Debug, Clone)]
pub struct ProvenTheorem {
    pub name: String,
    pub theory: String,
    pub goal: Expr,
}

impl Session {
    pub fn new() -> Self {
        Session {
            kernel: Kernel::new(),
            proven: Vec::new(),
            verbose: false,
        }
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

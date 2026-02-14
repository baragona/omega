/// Derivation trees and normalization.
///
/// A derivation tree records the proof structure: each node is a rule application
/// with sub-derivations for the premises. Proof checking is handled by
/// `interned_check`.
use crate::binding::{apply_meta_subst, whnf};
use crate::expr::{Expr, Name};
use crate::judgment::RewriteRule;
use crate::pattern::match_expr;

/// A derivation tree node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Derivation {
    /// Apply a rule with sub-derivations for each premise.
    RuleApp {
        rule_name: Name,
        /// Sub-derivations, one for each premise of the rule.
        premises: Vec<Derivation>,
    },
    /// An assumption: the goal must appear in the current context.
    Assumption,
    /// An assumption identified by index in the context.
    AssumptionIdx(usize),
}

/// Context for proof checking: a list of assumed judgments.
#[derive(Debug, Clone)]
pub struct Context {
    pub assumptions: Vec<Expr>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            assumptions: Vec::new(),
        }
    }

    pub fn with_assumptions(assumptions: Vec<Expr>) -> Self {
        Context { assumptions }
    }

    pub fn push(&mut self, assumption: Expr) {
        self.assumptions.push(assumption);
    }
}

/// Normalize an expression by exhaustively applying beta-reduction and
/// rewrite rules (innermost strategy).
pub fn normalize_expr(expr: &Expr, rewrites: &[RewriteRule], fuel: &mut usize) -> Expr {
    if *fuel == 0 {
        return expr.clone();
    }

    // Step 0: WHNF first to reduce any head beta-redexes
    let whnf_ed = whnf(expr);

    if rewrites.is_empty() {
        return whnf_ed;
    }

    // Step 1: Normalize children
    let children_normalized = match &whnf_ed {
        Expr::App(args) => {
            let new_args: Vec<Expr> = args
                .iter()
                .map(|a| normalize_expr(a, rewrites, fuel))
                .collect();
            if new_args == *args {
                whnf_ed.clone()
            } else {
                Expr::App(new_args)
            }
        }
        _ => whnf_ed,
    };

    // Step 1.5: WHNF again — child normalization may have exposed
    // beta-redexes (e.g. a rewrite expanded a constructor into a lambda)
    let after_whnf = whnf(&children_normalized);
    if after_whnf != children_normalized {
        return normalize_expr(&after_whnf, rewrites, fuel);
    }

    // Step 2: Try rewrite rules at the head
    let mut current = children_normalized;
    loop {
        if *fuel == 0 {
            break;
        }
        let mut matched = false;
        for rw in rewrites {
            if let Ok(subst) = match_expr(&rw.lhs, &current) {
                *fuel = fuel.saturating_sub(1);
                let replaced = apply_meta_subst(&rw.rhs, &subst);
                current = normalize_expr(&replaced, rewrites, fuel);
                matched = true;
                break;
            }
        }
        if !matched {
            break;
        }
    }

    current
}


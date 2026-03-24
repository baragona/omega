//! E-graph utilities for compiled Hyperion programs.
//!
//! Provides a simplified interface to the `egg` equality saturation library,
//! tailored for use by code generated from MetaCat theories.

pub use egg::{self, RecExpr, SymbolLang, Runner, Pattern, Rewrite, rewrite};

use crate::types::TheoryDef;

/// Build egg rewrites from a TheoryDef's rules.
/// Each rule's LHS and RHS are parsed as egg patterns.
pub fn theory_rewrites(theory: &TheoryDef) -> Vec<Rewrite<SymbolLang, ()>> {
    let mut rewrites = Vec::new();
    for (i, (lhs, rhs)) in theory.rules.iter().enumerate() {
        let name = format!("rule_{}", i);
        if let (Ok(l), Ok(r)) = (
            lhs.parse::<Pattern<SymbolLang>>(),
            rhs.parse::<Pattern<SymbolLang>>(),
        ) {
            rewrites.push(rewrite!(name; l => r));
        }
    }
    rewrites
}

/// Check if two expressions are equal under a theory's rewrite rules.
/// Uses equality saturation with default fuel (30 iterations, 10K nodes).
pub fn check_equal(lhs: &str, rhs: &str, theory: &TheoryDef) -> bool {
    let rewrites = theory_rewrites(theory);

    let lhs_expr: RecExpr<SymbolLang> = match lhs.parse() {
        Ok(e) => e,
        Err(_) => return false,
    };
    let rhs_expr: RecExpr<SymbolLang> = match rhs.parse() {
        Ok(e) => e,
        Err(_) => return false,
    };

    let runner = Runner::default()
        .with_expr(&lhs_expr)
        .with_expr(&rhs_expr)
        .run(&rewrites);

    let id1 = runner.egraph.find(*runner.roots.first().unwrap());
    let id2 = runner.egraph.find(*runner.roots.last().unwrap());
    id1 == id2
}

/// Simplify an expression by equality saturation, extracting the smallest
/// equivalent expression under the theory's rewrites.
pub fn simplify(expr: &str, theory: &TheoryDef) -> Option<String> {
    let rewrites = theory_rewrites(theory);

    let expr_parsed: RecExpr<SymbolLang> = match expr.parse() {
        Ok(e) => e,
        Err(_) => return None,
    };

    let runner = Runner::default()
        .with_expr(&expr_parsed)
        .run(&rewrites);

    let root = *runner.roots.first()?;
    let extractor = egg::Extractor::new(&runner.egraph, egg::AstSize);
    let (_, best) = extractor.find_best(root);
    Some(best.to_string())
}

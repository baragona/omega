//! Goal-Directed Proof Search pass: compile LCF-style tactic combinators
//! into iterative proof search strategies.
//!
//! Tactics:
//! - `[THEN t1 t2]`: Apply t1, then t2 to all subgoals
//! - `[ORELSE t1 t2]`: Try t1, fall back to t2
//! - `[REPEAT t]`: Apply t until it fails
//! - `[APPLY rule-name]`: Apply a named rule
//! - `[ASSUMPTION]`: Close goal from context
//! - `[ID]`: Identity tactic (no-op)
//! - `[FAIL]`: Always fails
//!
//! The pass compiles tactic expressions into a flat instruction sequence
//! that can be executed without recursion.

use apeiron::parser::{Sexp, Span};

/// A compiled tactic instruction.
#[derive(Debug, Clone)]
pub enum TacticInstr {
    /// Apply a named rule to the current goal
    Apply(String),
    /// Close goal if it matches an assumption
    Assumption,
    /// Identity — succeed without changing goal
    Id,
    /// Always fail
    Fail,
    /// Execute instrs[a], then instrs[b] on each subgoal
    Then(usize, usize),
    /// Try instrs[a], on failure try instrs[b]
    OrElse(usize, usize),
    /// Repeat instrs[a] until failure
    Repeat(usize),
}

/// A compiled tactic program (flat instruction array).
#[derive(Debug, Clone)]
pub struct TacticProgram {
    pub instructions: Vec<TacticInstr>,
    /// Entry point index
    pub entry: usize,
}

/// Proof goal for tactic execution.
#[derive(Debug, Clone)]
pub struct Goal {
    pub term: Sexp,
    pub assumptions: Vec<Sexp>,
}

/// Result of tactic execution.
#[derive(Debug, Clone)]
pub enum TacticResult {
    /// Proof complete — all goals closed
    Success,
    /// Remaining subgoals
    Subgoals(Vec<Goal>),
    /// Tactic failed
    Failure(String),
}

/// Compile a tactic Sexp into a TacticProgram.
pub fn compile_tactic(sexp: &Sexp) -> Result<TacticProgram, String> {
    let mut instrs = Vec::new();
    let entry = compile_inner(sexp, &mut instrs)?;
    Ok(TacticProgram { instructions: instrs, entry })
}

fn compile_inner(sexp: &Sexp, instrs: &mut Vec<TacticInstr>) -> Result<usize, String> {
    match sexp {
        Sexp::Atom(name, _) => {
            let instr = match name.as_str() {
                "ID" | "id" => TacticInstr::Id,
                "FAIL" | "fail" => TacticInstr::Fail,
                "ASSUMPTION" | "assumption" => TacticInstr::Assumption,
                _ => TacticInstr::Apply(name.clone()),
            };
            let idx = instrs.len();
            instrs.push(instr);
            Ok(idx)
        }
        Sexp::List(items, _) => {
            if items.is_empty() {
                return Err("empty tactic expression".to_string());
            }
            let head = items[0].as_atom().ok_or("tactic head must be atom")?;
            match head {
                "THEN" | "then" => {
                    if items.len() != 3 {
                        return Err("THEN expects 2 arguments".to_string());
                    }
                    let a = compile_inner(&items[1], instrs)?;
                    let b = compile_inner(&items[2], instrs)?;
                    let idx = instrs.len();
                    instrs.push(TacticInstr::Then(a, b));
                    Ok(idx)
                }
                "ORELSE" | "orelse" => {
                    if items.len() != 3 {
                        return Err("ORELSE expects 2 arguments".to_string());
                    }
                    let a = compile_inner(&items[1], instrs)?;
                    let b = compile_inner(&items[2], instrs)?;
                    let idx = instrs.len();
                    instrs.push(TacticInstr::OrElse(a, b));
                    Ok(idx)
                }
                "REPEAT" | "repeat" => {
                    if items.len() != 2 {
                        return Err("REPEAT expects 1 argument".to_string());
                    }
                    let a = compile_inner(&items[1], instrs)?;
                    let idx = instrs.len();
                    instrs.push(TacticInstr::Repeat(a));
                    Ok(idx)
                }
                "APPLY" | "apply" => {
                    if items.len() != 2 {
                        return Err("APPLY expects 1 argument".to_string());
                    }
                    let name = items[1].as_atom().ok_or("APPLY arg must be atom")?;
                    let idx = instrs.len();
                    instrs.push(TacticInstr::Apply(name.to_string()));
                    Ok(idx)
                }
                _ => {
                    // Treat unknown head as APPLY
                    let idx = instrs.len();
                    instrs.push(TacticInstr::Apply(head.to_string()));
                    Ok(idx)
                }
            }
        }
    }
}

/// Execute a tactic program against a goal using available rules.
/// `max_iterations` prevents infinite REPEAT loops.
pub fn execute_tactic(
    program: &TacticProgram,
    goal: &Goal,
    rules: &[crate::session::VonNeumannRule],
    max_iterations: usize,
) -> TacticResult {
    let mut fuel = max_iterations;
    execute_instr(program, program.entry, goal, rules, &mut fuel)
}

fn execute_instr(
    program: &TacticProgram,
    idx: usize,
    goal: &Goal,
    rules: &[crate::session::VonNeumannRule],
    fuel: &mut usize,
) -> TacticResult {
    if *fuel == 0 {
        return TacticResult::Failure("tactic fuel exhausted".to_string());
    }
    *fuel -= 1;

    match &program.instructions[idx] {
        TacticInstr::Id => TacticResult::Subgoals(vec![goal.clone()]),
        TacticInstr::Fail => TacticResult::Failure("FAIL tactic".to_string()),
        TacticInstr::Assumption => {
            // Check if goal matches any assumption
            let goal_str = format!("{}", goal.term);
            for assumption in &goal.assumptions {
                if format!("{}", assumption) == goal_str {
                    return TacticResult::Success;
                }
            }
            TacticResult::Failure("no matching assumption".to_string())
        }
        TacticInstr::Apply(rule_name) => {
            // Try to match rule conclusion against goal
            for rule in rules {
                if rule.name == *rule_name {
                    // Simplified: check if LHS structurally matches goal
                    let mut subst = std::collections::HashMap::new();
                    if crate::passes::logic_engine::try_match(&rule.lhs, &goal.term, &mut subst) {
                        let resolved_rhs = crate::passes::logic_engine::apply_subst(&rule.rhs, &subst);
                        // RHS becomes the new subgoal (or success if trivially true)
                        let rhs_str = format!("{}", resolved_rhs);
                        if rhs_str == "true" || rhs_str == "[true]" {
                            return TacticResult::Success;
                        }
                        return TacticResult::Subgoals(vec![Goal {
                            term: resolved_rhs,
                            assumptions: goal.assumptions.clone(),
                        }]);
                    }
                }
            }
            TacticResult::Failure(format!("rule {} does not match goal", rule_name))
        }
        TacticInstr::Then(a, b) => {
            let a = *a;
            let b = *b;
            match execute_instr(program, a, goal, rules, fuel) {
                TacticResult::Success => TacticResult::Success,
                TacticResult::Failure(e) => TacticResult::Failure(e),
                TacticResult::Subgoals(subgoals) => {
                    let mut remaining = Vec::new();
                    for sg in &subgoals {
                        match execute_instr(program, b, sg, rules, fuel) {
                            TacticResult::Success => {}
                            TacticResult::Failure(e) => return TacticResult::Failure(e),
                            TacticResult::Subgoals(more) => remaining.extend(more),
                        }
                    }
                    if remaining.is_empty() {
                        TacticResult::Success
                    } else {
                        TacticResult::Subgoals(remaining)
                    }
                }
            }
        }
        TacticInstr::OrElse(a, b) => {
            let a = *a;
            let b = *b;
            match execute_instr(program, a, goal, rules, fuel) {
                TacticResult::Failure(_) => execute_instr(program, b, goal, rules, fuel),
                other => other,
            }
        }
        TacticInstr::Repeat(a) => {
            let a = *a;
            let mut current = vec![goal.clone()];
            loop {
                if *fuel == 0 {
                    break;
                }
                let mut next = Vec::new();
                let mut any_progress = false;
                for g in &current {
                    match execute_instr(program, a, g, rules, fuel) {
                        TacticResult::Success => { any_progress = true; }
                        TacticResult::Subgoals(sgs) => {
                            if sgs.len() != 1 || format!("{}", sgs[0].term) != format!("{}", g.term) {
                                any_progress = true;
                            }
                            next.extend(sgs);
                        }
                        TacticResult::Failure(_) => { next.push(g.clone()); }
                    }
                }
                if !any_progress {
                    break;
                }
                current = next;
            }
            if current.is_empty() {
                TacticResult::Success
            } else {
                TacticResult::Subgoals(current)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(s: &str) -> Sexp { Sexp::Atom(s.to_string(), Span::default()) }
    fn list(items: Vec<Sexp>) -> Sexp { Sexp::List(items, Span::default()) }

    #[test]
    fn compile_simple_apply() {
        let tactic = list(vec![atom("APPLY"), atom("my-rule")]);
        let prog = compile_tactic(&tactic).unwrap();
        assert_eq!(prog.instructions.len(), 1);
        assert!(matches!(&prog.instructions[0], TacticInstr::Apply(n) if n == "my-rule"));
    }

    #[test]
    fn compile_then() {
        let tactic = list(vec![
            atom("THEN"),
            list(vec![atom("APPLY"), atom("r1")]),
            list(vec![atom("APPLY"), atom("r2")]),
        ]);
        let prog = compile_tactic(&tactic).unwrap();
        assert_eq!(prog.instructions.len(), 3); // r1, r2, THEN
        assert!(matches!(&prog.instructions[2], TacticInstr::Then(0, 1)));
    }

    #[test]
    fn compile_orelse() {
        let tactic = list(vec![
            atom("ORELSE"),
            atom("FAIL"),
            atom("ID"),
        ]);
        let prog = compile_tactic(&tactic).unwrap();
        assert!(matches!(&prog.instructions[prog.entry], TacticInstr::OrElse(_, _)));
    }

    #[test]
    fn compile_repeat() {
        let tactic = list(vec![atom("REPEAT"), atom("ID")]);
        let prog = compile_tactic(&tactic).unwrap();
        assert!(matches!(&prog.instructions[prog.entry], TacticInstr::Repeat(_)));
    }

    #[test]
    fn execute_assumption_success() {
        let prog = compile_tactic(&atom("ASSUMPTION")).unwrap();
        let goal = Goal {
            term: atom("P"),
            assumptions: vec![atom("P")],
        };
        let result = execute_tactic(&prog, &goal, &[], 100);
        assert!(matches!(result, TacticResult::Success));
    }

    #[test]
    fn execute_assumption_failure() {
        let prog = compile_tactic(&atom("ASSUMPTION")).unwrap();
        let goal = Goal {
            term: atom("P"),
            assumptions: vec![atom("Q")],
        };
        let result = execute_tactic(&prog, &goal, &[], 100);
        assert!(matches!(result, TacticResult::Failure(_)));
    }

    #[test]
    fn execute_orelse_fallback() {
        let tactic = list(vec![atom("ORELSE"), atom("FAIL"), atom("ID")]);
        let prog = compile_tactic(&tactic).unwrap();
        let goal = Goal { term: atom("P"), assumptions: vec![] };
        let result = execute_tactic(&prog, &goal, &[], 100);
        // ID returns the goal as subgoal
        assert!(matches!(result, TacticResult::Subgoals(_)));
    }

    #[test]
    fn execute_fail() {
        let prog = compile_tactic(&atom("FAIL")).unwrap();
        let goal = Goal { term: atom("P"), assumptions: vec![] };
        let result = execute_tactic(&prog, &goal, &[], 100);
        assert!(matches!(result, TacticResult::Failure(_)));
    }

    // === ABYSSAL: State-Leaking ORELSE ===

    #[test]
    fn orelse_no_state_leak_from_failed_first_branch() {
        // tactic_A matches the goal and transforms it, producing a subgoal.
        // But THEN(tactic_A, FAIL) will fail on the subgoal.
        // ORELSE should then try tactic_B on the ORIGINAL goal, not the
        // partially-transformed one from tactic_A.

        // Rule: "step" matches [P] and produces [Q]
        let rules = vec![
            crate::session::VonNeumannRule {
                name: "step".to_string(),
                lhs: list(vec![atom("P")]),
                rhs: list(vec![atom("Q")]),
            },
            // "finish" matches [P] and produces true
            crate::session::VonNeumannRule {
                name: "finish".to_string(),
                lhs: list(vec![atom("P")]),
                rhs: atom("true"),
            },
        ];

        // Tactic: ORELSE (THEN step FAIL) finish
        // - Branch 1: step succeeds (P→Q), then FAIL kills it
        // - Branch 2: finish should see ORIGINAL goal [P], not [Q]
        let tactic = list(vec![atom("ORELSE"),
            list(vec![atom("THEN"),
                list(vec![atom("APPLY"), atom("step")]),
                atom("FAIL")]),
            list(vec![atom("APPLY"), atom("finish")])]);

        let prog = compile_tactic(&tactic).unwrap();
        let goal = Goal {
            term: list(vec![atom("P")]),
            assumptions: vec![],
        };

        let result = execute_tactic(&prog, &goal, &rules, 100);
        assert!(matches!(result, TacticResult::Success),
            "ORELSE must fall back to finish on ORIGINAL goal [P], not leaked [Q]. Got: {:?}",
            match &result {
                TacticResult::Failure(e) => e.as_str(),
                TacticResult::Subgoals(gs) => "subgoals remaining",
                TacticResult::Success => "success",
            });
    }

    #[test]
    fn orelse_fuel_not_leaked_across_branches() {
        // Verify that fuel consumed by the failed first branch doesn't
        // starve the second branch. Give exactly enough fuel for both.
        let rules = vec![
            crate::session::VonNeumannRule {
                name: "win".to_string(),
                lhs: atom("G"),
                rhs: atom("true"),
            },
        ];

        // ORELSE FAIL win — FAIL costs 1 fuel, win costs 1 fuel, ORELSE costs 1 = 3 total
        let tactic = list(vec![atom("ORELSE"), atom("FAIL"),
            list(vec![atom("APPLY"), atom("win")])]);
        let prog = compile_tactic(&tactic).unwrap();
        let goal = Goal { term: atom("G"), assumptions: vec![] };

        // With fuel=3, should succeed (1 for ORELSE + 1 for FAIL + 1 for win)
        let result = execute_tactic(&prog, &goal, &rules, 3);
        assert!(matches!(result, TacticResult::Success),
            "Must succeed with exactly enough fuel for both branches");
    }

    #[test]
    fn compile_error_on_empty() {
        let result = compile_tactic(&list(vec![]));
        assert!(result.is_err());
    }
}

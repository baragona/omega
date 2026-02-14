/// Command dispatch: process top-level forms from the parser.
use omega_core::derivation::{normalize_expr, Context};
use omega_core::expr::Expr;
use omega_core::judgment::Rule;
use omega_core::reflection;
use omega_elaborate::elaborate::elaborate;
use omega_elaborate::tactic::Tactic;
use omega_syntax::desugar::{Command, TacticCmd};
use omega_syntax::printer;

use crate::session::{ProvenTheorem, Session};

/// Process a single command, returning a human-readable result message.
pub fn process_command(session: &mut Session, cmd: Command) -> Result<String, String> {
    match cmd {
        Command::TheoryDef(mut theory) => {
            // Resolve imports: merge declarations from imported theories
            let imports = theory.imports.clone();
            for import in &imports {
                let imported = session
                    .kernel
                    .get_theory(&import.theory_name)
                    .ok_or_else(|| format!("import error: unknown theory '{}'", import.theory_name))?
                    .clone();

                if import.args.is_empty() && import.alias.is_none() {
                    // Simple import, no alias: merge as-is (existing behavior)
                    theory
                        .merge_from(&imported)
                        .map_err(|e| format!("import error in theory {}: {}", theory.name, e))?;
                } else {
                    // Parameterized or aliased import: instantiate then merge
                    let alias = import.alias.as_deref().unwrap_or(&import.theory_name);
                    let instance = imported
                        .instantiate(&import.args, alias)
                        .map_err(|e| format!("import error in theory {}: {}", theory.name, e))?;
                    theory
                        .merge_from(&instance)
                        .map_err(|e| format!("import error in theory {}: {}", theory.name, e))?;
                }
            }
            let name = theory.name.clone();
            session
                .kernel
                .register_theory(theory)
                .map_err(|e| format!("Error registering theory: {}", e))?;
            Ok(format!("Theory {}: registered OK", name))
        }

        Command::CheckTheory(name) => {
            let theory = session
                .kernel
                .get_theory(&name)
                .ok_or_else(|| format!("Unknown theory: {}", name))?;
            let summary = printer::print_theory_summary(theory);
            Ok(format!("Theory {} is valid.\n{}", name, summary))
        }

        Command::Proof {
            name,
            theory,
            goal,
            derivation,
            assumptions,
        } => {
            let ctx = Context::with_assumptions(assumptions);
            session
                .kernel
                .check_derivation(&theory, &goal, &derivation, &ctx)
                .map_err(|e| format!("Proof {} INVALID: {}", name, e))?;

            session.proven.push(ProvenTheorem {
                name: name.clone(),
                theory: theory.clone(),
                goal: goal.clone(),
            });

            Ok(format!("Proof {}: VALID", name))
        }

        Command::TacticProof {
            name,
            theory: theory_name,
            goal,
            tactics,
            assumptions,
        } => {
            let theory = session
                .kernel
                .get_theory(&theory_name)
                .ok_or_else(|| format!("Unknown theory: {}", theory_name))?
                .clone();

            let ctx = Context::with_assumptions(assumptions);

            // Convert tactic commands to Tactic values
            let tactics: Vec<Tactic> = tactics
                .into_iter()
                .map(|tc| convert_tactic(tc))
                .collect::<Result<Vec<_>, _>>()?;

            // Elaborate tactics into a derivation
            let derivation = elaborate(&tactics, goal.clone(), ctx.clone(), &theory)
                .map_err(|e| format!("Tactic elaboration failed for {}: {}", name, e))?;

            if session.verbose {
                eprintln!(
                    "  Elaborated derivation: {}",
                    printer::print_derivation(&derivation)
                );
            }

            // Verify the derivation with the kernel
            session
                .kernel
                .check_derivation(&theory_name, &goal, &derivation, &ctx)
                .map_err(|e| format!("Proof {} INVALID (after elaboration): {}", name, e))?;

            session.proven.push(ProvenTheorem {
                name: name.clone(),
                theory: theory_name.clone(),
                goal: goal.clone(),
            });

            Ok(format!("Proof {}: VALID (via tactics)", name))
        }

        Command::MetaTheoremDef(mt) => {
            let name = mt.name.clone();
            session
                .kernel
                .check_metatheorem(mt)
                .map_err(|e| format!("Metatheorem {} INVALID: {}", name, e))?;
            Ok(format!("Metatheorem {}: VERIFIED", name))
        }

        Command::Reflect {
            metatheorem,
            rule_name,
            theory,
        } => {
            // Reflection is a driver-level operation: look up the verified
            // metatheorem, build the admissible rule, and add it to the theory.
            let mt = session
                .kernel
                .get_verified_metatheorem(&metatheorem)
                .ok_or_else(|| format!("Reflection failed: unproven metatheorem: {}", metatheorem))?
                .clone();
            let th = session
                .kernel
                .get_theory(&mt.theory_name)
                .ok_or_else(|| format!("Reflection failed: unknown theory: {}", mt.theory_name))?;
            let (rule, _record) = reflection::reflect(&mt, &rule_name, th)
                .map_err(|e| format!("Reflection failed: {}", e))?;
            session
                .kernel
                .add_rule(&mt.theory_name, rule)
                .map_err(|e| format!("Reflection failed: {}", e))?;
            Ok(format!(
                "Reflected {} as {} in theory {}",
                metatheorem, rule_name, theory
            ))
        }

        Command::Lemma {
            name,
            theory,
            premises,
            conclusion,
            derivation,
        } => {
            let ctx = Context::with_assumptions(premises.clone());
            session
                .kernel
                .check_derivation(&theory, &conclusion, &derivation, &ctx)
                .map_err(|e| format!("Lemma {} INVALID: {}", name, e))?;
            finish_lemma(session, &name, &theory, premises, conclusion)?;
            Ok(format!("Lemma {}: VALID [DERIVED]", name))
        }

        Command::TacticLemma {
            name,
            theory: theory_name,
            premises,
            conclusion,
            tactics,
        } => {
            let theory = session
                .kernel
                .get_theory(&theory_name)
                .ok_or_else(|| format!("Unknown theory: {}", theory_name))?
                .clone();

            let ctx = Context::with_assumptions(premises.clone());

            let tactics: Vec<Tactic> = tactics
                .into_iter()
                .map(|tc| convert_tactic(tc))
                .collect::<Result<Vec<_>, _>>()?;

            let derivation = elaborate(&tactics, conclusion.clone(), ctx.clone(), &theory)
                .map_err(|e| format!("Tactic elaboration failed for lemma {}: {}", name, e))?;

            if session.verbose {
                eprintln!(
                    "  Elaborated derivation: {}",
                    printer::print_derivation(&derivation)
                );
            }

            session
                .kernel
                .check_derivation(&theory_name, &conclusion, &derivation, &ctx)
                .map_err(|e| format!("Lemma {} INVALID (after elaboration): {}", name, e))?;
            finish_lemma(session, &name, &theory_name, premises, conclusion)?;
            Ok(format!("Lemma {}: VALID [DERIVED] (via tactics)", name))
        }

        Command::Emit { theory, expr } => {
            let th = session
                .kernel
                .get_theory(&theory)
                .ok_or_else(|| format!("emit: unknown theory '{}'", theory))?
                .clone();

            // Normalize the expression using the theory's rewrite rules
            let mut fuel = 10_000usize;
            let normalized = normalize_expr(&expr, &th.rewrites, &mut fuel);

            // Flatten the rope tree to text
            let mut buf = String::new();
            flatten_rope(&normalized, &mut buf);
            Ok(format!("Emit:\n{}", buf))
        }
    }
}

/// Shared logic for finishing a lemma: register derived rule + record as proven.
fn finish_lemma(
    session: &mut Session,
    name: &str,
    theory_name: &str,
    premises: Vec<Expr>,
    conclusion: Expr,
) -> Result<(), String> {
    let mut rule = Rule::new(name.to_string(), premises, conclusion.clone());
    rule.provenance = Some(format!("lemma:{}", name));
    session
        .kernel
        .add_rule(theory_name, rule)
        .map_err(|e| format!("Lemma {} failed to register rule: {}", name, e))?;
    session.proven.push(ProvenTheorem {
        name: name.to_string(),
        theory: theory_name.to_string(),
        goal: conclusion,
    });
    Ok(())
}

/// Flatten a rope expression tree into a string buffer.
/// Recognizes: cat(a, b), empty, newline, and Sym(s) as literal text.
fn flatten_rope(expr: &Expr, buf: &mut String) {
    match expr {
        Expr::Sym(s) => match s.as_str() {
            "empty" => {}
            "newline" => buf.push('\n'),
            _ => buf.push_str(s),
        },
        Expr::App(args) if args.len() == 3 && matches!(&args[0], Expr::Sym(s) if s == "cat") => {
            flatten_rope(&args[1], buf);
            flatten_rope(&args[2], buf);
        }
        // Fallback: print the expression debug form
        other => buf.push_str(&format!("{:?}", other)),
    }
}

fn convert_tactic(tc: TacticCmd) -> Result<Tactic, String> {
    match tc {
        TacticCmd::Apply(name) => Ok(Tactic::Apply(name)),
        TacticCmd::Assumption => Ok(Tactic::Assumption),
        TacticCmd::Intro(name) => Ok(Tactic::Intro(name)),
        TacticCmd::Exact(deriv) => Ok(Tactic::Exact(deriv)),
        TacticCmd::Auto(depth) => Ok(Tactic::Auto(depth.unwrap_or(5))),
        TacticCmd::Qed => Ok(Tactic::Assumption), // Qed is just a marker
    }
}

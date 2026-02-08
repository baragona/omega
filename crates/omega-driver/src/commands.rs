/// Command dispatch: process top-level forms from the parser.
use omega_core::derivation::Context;
use omega_elaborate::elaborate::elaborate;
use omega_elaborate::tactic::Tactic;
use omega_syntax::desugar::{Command, TacticCmd};
use omega_syntax::printer;

use crate::session::{ProvenTheorem, Session};

/// Process a single command, returning a human-readable result message.
pub fn process_command(session: &mut Session, cmd: Command) -> Result<String, String> {
    match cmd {
        Command::TheoryDef(theory) => {
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
            session
                .kernel
                .reflect(&metatheorem, &rule_name)
                .map_err(|e| format!("Reflection failed: {}", e))?;
            Ok(format!(
                "Reflected {} as {} in theory {}",
                metatheorem, rule_name, theory
            ))
        }
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

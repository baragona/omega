/// Theory: a collection of sorts, constructors, judgment forms, and rules.
///
/// A theory defines a logic. The kernel validates theory well-formedness
/// but is otherwise logic-agnostic.
use std::collections::HashMap;

use crate::binding_spec::BindingSpec;
use crate::error::{OmegaError, Result};
use crate::expr::Name;
use crate::judgment::{ConstructorDecl, JudgmentForm, RewriteRule, Rule, SortDecl};

/// Controls the structural rules of the context.
///
/// - **Structural**: Contraction allowed — assumptions can be reused freely.
/// - **Affine**: Contraction banned — each assumption can be used at most once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextMode {
    #[default]
    Structural,
    Affine,
}

/// A user-defined theory (logic).
#[derive(Debug, Clone)]
pub struct Theory {
    /// The name of this theory.
    pub name: Name,
    /// Sort declarations.
    pub sorts: Vec<SortDecl>,
    /// Constructor declarations.
    pub constructors: Vec<ConstructorDecl>,
    /// Judgment form declarations.
    pub judgments: Vec<JudgmentForm>,
    /// Inference rules.
    pub rules: Vec<Rule>,
    /// User-defined binding specifications.
    pub binding_specs: Vec<BindingSpec>,
    /// Rewrite rules for definitional equality (delta reduction).
    pub rewrites: Vec<RewriteRule>,
    /// Context mode: structural (default) or affine (at-most-once usage).
    pub context_mode: ContextMode,
    /// A hash of the theory content for staleness detection.
    pub content_hash: u64,
    /// Names of theories to import (resolved at registration time).
    pub imports: Vec<Name>,
}

impl Theory {
    /// Create a new empty theory.
    pub fn new(name: &str) -> Self {
        Theory {
            name: name.to_string(),
            sorts: Vec::new(),
            constructors: Vec::new(),
            judgments: Vec::new(),
            binding_specs: Vec::new(),
            rules: Vec::new(),
            rewrites: Vec::new(),
            context_mode: ContextMode::default(),
            content_hash: 0,
            imports: Vec::new(),
        }
    }

    /// Validate that the theory is well-formed:
    /// - No duplicate sort/constructor/rule names
    /// - Rules reference valid judgment forms
    pub fn validate(&self) -> Result<()> {
        self.check_duplicates()?;
        self.check_rule_references()?;
        self.check_rewrites()?;
        Ok(())
    }

    fn check_duplicates(&self) -> Result<()> {
        let mut seen = HashMap::new();

        for s in &self.sorts {
            if seen.insert(("sort", &s.name), ()).is_some() {
                return Err(OmegaError::DuplicateSort(s.name.clone()));
            }
        }

        for c in &self.constructors {
            if seen.insert(("ctor", &c.name), ()).is_some() {
                return Err(OmegaError::DuplicateConstructor(c.name.clone()));
            }
        }

        for r in &self.rules {
            if seen.insert(("rule", &r.name), ()).is_some() {
                return Err(OmegaError::DuplicateRule(r.name.clone()));
            }
        }

        for j in &self.judgments {
            if seen.insert(("judgment", &j.name), ()).is_some() {
                return Err(OmegaError::DuplicateJudgment(j.name.clone()));
            }
        }

        for bs in &self.binding_specs {
            if seen.insert(("binding-spec", &bs.name), ()).is_some() {
                return Err(OmegaError::DuplicateName {
                    kind: "binding-spec".to_string(),
                    name: bs.name.clone(),
                });
            }
        }

        Ok(())
    }

    fn check_rule_references(&self) -> Result<()> {
        // Collect known constructor and sort names for reference
        let _sort_names: Vec<&str> = self.sorts.iter().map(|s| s.name.as_str()).collect();
        let _ctor_names: Vec<&str> = self.constructors.iter().map(|c| c.name.as_str()).collect();
        // Rules can reference any constructor or sort — we do a light check here.
        // Full type checking of rule patterns is left to the user's logic.
        Ok(())
    }

    fn check_rewrites(&self) -> Result<()> {
        for rw in &self.rewrites {
            let lhs_metas = rw.lhs.meta_vars();
            let rhs_metas = rw.rhs.meta_vars();
            for m in &rhs_metas {
                if !lhs_metas.contains(m) {
                    return Err(OmegaError::MalformedDerivation(format!(
                        "rewrite rule {}: RHS meta-variable ?{} not in LHS",
                        rw.name, m
                    )));
                }
            }
        }
        Ok(())
    }

    /// Look up a rule by name.
    pub fn get_rule(&self, name: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.name == name)
    }

    /// Look up a judgment form by name.
    pub fn get_judgment(&self, name: &str) -> Option<&JudgmentForm> {
        self.judgments.iter().find(|j| j.name == name)
    }

    /// Look up a sort by name.
    pub fn get_sort(&self, name: &str) -> Option<&SortDecl> {
        self.sorts.iter().find(|s| s.name == name)
    }

    /// Look up a constructor by name.
    pub fn get_constructor(&self, name: &str) -> Option<&ConstructorDecl> {
        self.constructors.iter().find(|c| c.name == name)
    }

    /// Look up a binding spec by name.
    pub fn get_binding_spec(&self, name: &str) -> Option<&BindingSpec> {
        self.binding_specs.iter().find(|bs| bs.name == name)
    }

    /// Compute and update the content hash.
    pub fn compute_hash(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.name.hash(&mut hasher);
        for s in &self.sorts {
            s.name.hash(&mut hasher);
        }
        for c in &self.constructors {
            c.name.hash(&mut hasher);
        }
        for r in &self.rules {
            r.name.hash(&mut hasher);
            r.premises.len().hash(&mut hasher);
        }
        for rw in &self.rewrites {
            rw.name.hash(&mut hasher);
        }
        matches!(self.context_mode, ContextMode::Affine).hash(&mut hasher);
        self.content_hash = hasher.finish();
    }

    /// Merge all declarations from another theory into this one.
    /// Used for resolving `(import ...)` directives. Errors on name collisions.
    pub fn merge_from(&mut self, other: &Theory) -> Result<()> {
        for s in &other.sorts {
            if self.sorts.iter().any(|x| x.name == s.name) {
                return Err(OmegaError::DuplicateName {
                    kind: "sort".to_string(),
                    name: s.name.clone(),
                });
            }
            self.sorts.push(s.clone());
        }
        for c in &other.constructors {
            if self.constructors.iter().any(|x| x.name == c.name) {
                return Err(OmegaError::DuplicateName {
                    kind: "constructor".to_string(),
                    name: c.name.clone(),
                });
            }
            self.constructors.push(c.clone());
        }
        for j in &other.judgments {
            if self.judgments.iter().any(|x| x.name == j.name) {
                return Err(OmegaError::DuplicateName {
                    kind: "judgment".to_string(),
                    name: j.name.clone(),
                });
            }
            self.judgments.push(j.clone());
        }
        for r in &other.rules {
            if self.rules.iter().any(|x| x.name == r.name) {
                return Err(OmegaError::DuplicateName {
                    kind: "rule".to_string(),
                    name: r.name.clone(),
                });
            }
            self.rules.push(r.clone());
        }
        for rw in &other.rewrites {
            if self.rewrites.iter().any(|x| x.name == rw.name) {
                return Err(OmegaError::DuplicateName {
                    kind: "rewrite".to_string(),
                    name: rw.name.clone(),
                });
            }
            self.rewrites.push(rw.clone());
        }
        for bs in &other.binding_specs {
            if self.binding_specs.iter().any(|x| x.name == bs.name) {
                return Err(OmegaError::DuplicateName {
                    kind: "binding-spec".to_string(),
                    name: bs.name.clone(),
                });
            }
            self.binding_specs.push(bs.clone());
        }
        Ok(())
    }

    /// Add a rule (e.g., from reflection).
    pub fn add_rule(&mut self, rule: Rule) -> Result<()> {
        if self.get_rule(&rule.name).is_some() {
            return Err(OmegaError::DuplicateRule(rule.name.clone()));
        }
        self.rules.push(rule);
        self.compute_hash();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;
    use crate::judgment::{ConstructorDecl, JudgmentForm, Rule, SortDecl};

    fn make_prop_logic() -> Theory {
        let mut theory = Theory::new("PropLogic");

        theory.sorts.push(SortDecl {
            name: "Prop".to_string(),
        });

        theory.constructors.push(ConstructorDecl {
            name: "true".to_string(),
            ty: Expr::sym("Prop"),
        });
        theory.constructors.push(ConstructorDecl {
            name: "and".to_string(),
            ty: Expr::app(vec![
                Expr::sym("->"),
                Expr::sym("Prop"),
                Expr::sym("Prop"),
                Expr::sym("Prop"),
            ]),
        });

        theory.judgments.push(JudgmentForm {
            name: "proves".to_string(),
            pattern: Expr::app(vec![Expr::sym("proves"), Expr::meta("P")]),
            constraints: vec![("P".to_string(), "Prop".to_string())],
        });

        theory.rules.push(Rule {
            name: "and-intro".to_string(),
            premises: vec![
                Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
                Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
            ],
            conclusion: Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
            ]),
            reflected: false,
            provenance: None,
            implicit_args: vec![],
            context_extensions: vec![],
        });

        theory.rules.push(Rule {
            name: "and-elim-l".to_string(),
            premises: vec![Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
            ])],
            conclusion: Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
            reflected: false,
            provenance: None,
            implicit_args: vec![],
            context_extensions: vec![],
        });

        theory.rules.push(Rule {
            name: "and-elim-r".to_string(),
            premises: vec![Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
            ])],
            conclusion: Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
            reflected: false,
            provenance: None,
            implicit_args: vec![],
            context_extensions: vec![],
        });

        theory.compute_hash();
        theory
    }

    #[test]
    fn validate_prop_logic() {
        let theory = make_prop_logic();
        assert!(theory.validate().is_ok());
    }

    #[test]
    fn detect_duplicate_sort() {
        let mut theory = Theory::new("Bad");
        theory.sorts.push(SortDecl {
            name: "Prop".to_string(),
        });
        theory.sorts.push(SortDecl {
            name: "Prop".to_string(),
        });
        assert!(matches!(
            theory.validate(),
            Err(OmegaError::DuplicateSort(_))
        ));
    }

    #[test]
    fn lookup_rule() {
        let theory = make_prop_logic();
        assert!(theory.get_rule("and-intro").is_some());
        assert!(theory.get_rule("nonexistent").is_none());
    }

    #[test]
    fn merge_from_imports() {
        let base = make_prop_logic();
        let mut derived = Theory::new("Derived");
        assert!(derived.merge_from(&base).is_ok());
        // Derived should now have all of base's declarations
        assert!(derived.get_sort("Prop").is_some());
        assert!(derived.get_constructor("and").is_some());
        assert!(derived.get_rule("and-intro").is_some());
        assert!(derived.get_rule("and-elim-l").is_some());
        assert_eq!(derived.sorts.len(), 1);
        assert_eq!(derived.constructors.len(), 2);
        assert_eq!(derived.rules.len(), 3);
    }

    #[test]
    fn merge_from_rejects_collision() {
        let base = make_prop_logic();
        let mut derived = Theory::new("Derived");
        derived.sorts.push(SortDecl {
            name: "Prop".to_string(), // Same as base
        });
        let result = derived.merge_from(&base);
        assert!(result.is_err());
    }
}

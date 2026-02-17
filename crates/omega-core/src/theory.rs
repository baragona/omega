/// Theory: a collection of sorts, constructors, judgment forms, and rules.
///
/// A theory defines a logic. The kernel validates theory well-formedness
/// but is otherwise logic-agnostic.
///
/// # Construction
///
/// Theories are built via the [`TheoryBuilder`] type:
/// ```ignore
/// let mut tb = Theory::builder("MyLogic");
/// tb.add_sort(SortDecl::new("Prop"));
/// tb.push_rule(Rule::new("and-intro", premises, conclusion));
/// let theory = tb.build()?; // validates + hashes
/// ```
///
/// Once built, a `Theory` is immutable except for `add_rule()` (reflection).
use std::collections::{HashMap, HashSet};

use crate::binding::subst_syms;
use crate::binding_spec::BindingSpec;
use crate::error::{DeclKind, OmegaError, Result};
use crate::expr::{Expr, Name};
use crate::judgment::{ConstructorDecl, JudgmentForm, RewriteRule, Rule, SortDecl};

/// Symbol attributes declared by the user.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Attribute {
    /// Associative-Commutative: flatten, sort, rebuild at intern time.
    AC,
    /// AC + Idempotent: also deduplicate after sorting.
    ACI,
}

/// An import directive: import a theory, optionally with parameterized arguments and alias.
#[derive(Debug, Clone)]
pub struct Import {
    /// Name of the theory to import.
    pub theory_name: Name,
    /// Arguments for parameterized imports (empty for simple imports).
    pub args: Vec<Expr>,
    /// Optional alias prefix for imported names.
    pub alias: Option<Name>,
}

/// Controls the structural rules of the context.
///
/// - **Structural**: Contraction allowed — assumptions can be reused freely.
/// - **Affine**: Contraction banned — each assumption can be used at most once.
/// - **Linear**: Each assumption must be used exactly once (no contraction, no weakening).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextMode {
    #[default]
    Structural,
    Affine,
    Linear,
}

/// A validated, immutable theory. Created by [`TheoryBuilder::build()`].
///
/// Once built, the only mutation allowed is `add_rule()` (for reflection),
/// which re-validates and re-hashes internally.
#[derive(Debug, Clone)]
pub struct Theory {
    name: Name,
    sorts: Vec<SortDecl>,
    constructors: Vec<ConstructorDecl>,
    judgments: Vec<JudgmentForm>,
    rules: Vec<Rule>,
    binding_specs: Vec<BindingSpec>,
    rewrites: Vec<RewriteRule>,
    context_mode: ContextMode,
    content_hash: u64,
    imports: Vec<Import>,
    params: Vec<(Name, Expr)>,
    substitutive_binders: HashSet<Name>,
    eta_binders: HashSet<Name>,
    linear_binders: HashSet<Name>,
    affine_binders: HashSet<Name>,
    attributes: HashMap<Name, HashSet<Attribute>>,
}

/// A mutable theory under construction. Use [`Theory::builder()`] to create one.
///
/// Call [`build()`](TheoryBuilder::build) to validate and finalize into a [`Theory`].
#[derive(Debug, Clone)]
pub struct TheoryBuilder {
    name: Name,
    sorts: Vec<SortDecl>,
    constructors: Vec<ConstructorDecl>,
    judgments: Vec<JudgmentForm>,
    rules: Vec<Rule>,
    binding_specs: Vec<BindingSpec>,
    rewrites: Vec<RewriteRule>,
    context_mode: ContextMode,
    imports: Vec<Import>,
    params: Vec<(Name, Expr)>,
    substitutive_binders: HashSet<Name>,
    eta_binders: HashSet<Name>,
    linear_binders: HashSet<Name>,
    affine_binders: HashSet<Name>,
    attributes: HashMap<Name, HashSet<Attribute>>,
}

impl Theory {
    /// Create a new theory builder.
    pub fn builder(name: &str) -> TheoryBuilder {
        TheoryBuilder {
            name: name.into(),
            sorts: Vec::new(),
            constructors: Vec::new(),
            judgments: Vec::new(),
            binding_specs: Vec::new(),
            rules: Vec::new(),
            rewrites: Vec::new(),
            context_mode: ContextMode::default(),
            imports: Vec::new(),
            params: Vec::new(),
            substitutive_binders: HashSet::new(),
            eta_binders: HashSet::new(),
            linear_binders: HashSet::new(),
            affine_binders: HashSet::new(),
            attributes: HashMap::new(),
        }
    }

    // --- Read accessors ---

    /// The theory name.
    pub fn name(&self) -> &str { &self.name }
    /// Get the content hash for staleness detection.
    pub fn content_hash(&self) -> u64 { self.content_hash }
    /// All sort declarations.
    pub fn sorts(&self) -> &[SortDecl] { &self.sorts }
    /// All constructor declarations.
    pub fn constructors(&self) -> &[ConstructorDecl] { &self.constructors }
    /// All judgment form declarations.
    pub fn judgments(&self) -> &[JudgmentForm] { &self.judgments }
    /// All inference rules.
    pub fn rules(&self) -> &[Rule] { &self.rules }
    /// All binding specifications.
    pub fn binding_specs(&self) -> &[BindingSpec] { &self.binding_specs }
    /// All rewrite rules.
    pub fn rewrites(&self) -> &[RewriteRule] { &self.rewrites }
    /// The context mode (structural or affine).
    pub fn context_mode(&self) -> ContextMode { self.context_mode }
    /// Import directives.
    pub fn imports(&self) -> &[Import] { &self.imports }
    /// Theory parameters.
    pub fn params(&self) -> &[(Name, Expr)] { &self.params }
    /// Binder kinds that trigger beta-reduction.
    pub fn substitutive_binders(&self) -> &HashSet<Name> { &self.substitutive_binders }
    /// Binder kinds that trigger eta-contraction.
    pub fn eta_binders(&self) -> &HashSet<Name> { &self.eta_binders }
    /// Binder kinds requiring linear usage (exactly once).
    pub fn linear_binders(&self) -> &HashSet<Name> { &self.linear_binders }
    /// Binder kinds requiring affine usage (at most once).
    pub fn affine_binders(&self) -> &HashSet<Name> { &self.affine_binders }
    /// Symbol attributes (AC, ACI, etc.).
    pub fn attributes(&self) -> &HashMap<Name, HashSet<Attribute>> { &self.attributes }

    // --- Lookup methods ---

    /// Look up a rule by name.
    pub fn get_rule(&self, name: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| *r.name() == name)
    }

    /// Look up a judgment form by name.
    pub fn get_judgment(&self, name: &str) -> Option<&JudgmentForm> {
        self.judgments.iter().find(|j| *j.name() == name)
    }

    /// Look up a sort by name.
    pub fn get_sort(&self, name: &str) -> Option<&SortDecl> {
        self.sorts.iter().find(|s| *s.name() == name)
    }

    /// Look up a constructor by name.
    pub fn get_constructor(&self, name: &str) -> Option<&ConstructorDecl> {
        self.constructors.iter().find(|c| *c.name() == name)
    }

    /// Look up a binding spec by name.
    pub fn get_binding_spec(&self, name: &str) -> Option<&BindingSpec> {
        self.binding_specs.iter().find(|bs| bs.name == name)
    }

    // --- Post-registration mutation ---

    /// Add a rule to an already-registered theory (e.g., from reflection).
    /// Re-validates and re-hashes internally.
    pub fn add_rule(&mut self, rule: Rule) -> Result<()> {
        if self.get_rule(rule.name()).is_some() {
            return Err(OmegaError::DuplicateName { kind: DeclKind::Rule, name: rule.name().clone() });
        }
        self.rules.push(rule);
        self.compute_hash();
        Ok(())
    }

    /// Create a concrete theory by substituting parameters with arguments
    /// and prefixing all internal names with the alias.
    ///
    /// The resulting theory is fully concrete (params = []) and can be
    /// merged into the importing theory via [`TheoryBuilder::merge_from`].
    pub fn instantiate(&self, args: &[Expr], alias: &str) -> Result<Theory> {
        if args.len() != self.params.len() {
            return Err(OmegaError::ParamCountMismatch {
                theory: self.name.clone(),
                expected: self.params.len(),
                got: args.len(),
            });
        }

        // 1. Build param_map: parameter name → argument expr
        let mut combined_map: HashMap<Name, Expr> = HashMap::new();
        for (i, (param_name, _param_ty)) in self.params.iter().enumerate() {
            combined_map.insert(param_name.clone(), args[i].clone());
        }

        // 2. Build rename_map: internal name → Sym("alias.name")
        //    Only add if not already in param_map (param substitution wins)
        let mut internal_names: Vec<Name> = Vec::new();
        for s in &self.sorts {
            internal_names.push(s.name().clone());
        }
        for c in &self.constructors {
            internal_names.push(c.name().clone());
        }
        for j in &self.judgments {
            internal_names.push(j.name().clone());
        }
        for r in &self.rules {
            internal_names.push(r.name().clone());
        }
        for rw in &self.rewrites {
            internal_names.push(rw.name().clone());
        }

        for name in &internal_names {
            if !combined_map.contains_key(name) {
                combined_map.insert(
                    name.clone(),
                    Expr::Sym(format!("{}.{}", alias, name).into()),
                );
            }
        }

        // 3. Apply subst_syms to all Expr fields, rename string name fields
        let sorts: Vec<SortDecl> = self
            .sorts
            .iter()
            .map(|s| SortDecl::new(format!("{}.{}", alias, s.name())))
            .collect();

        let constructors: Vec<ConstructorDecl> = self
            .constructors
            .iter()
            .map(|c| ConstructorDecl::new(
                format!("{}.{}", alias, c.name()),
                subst_syms(c.ty(), &combined_map),
            ))
            .collect();

        let judgments: Vec<JudgmentForm> = self
            .judgments
            .iter()
            .map(|j| JudgmentForm::new(
                format!("{}.{}", alias, j.name()),
                subst_syms(j.pattern(), &combined_map),
                j.constraints()
                    .iter()
                    .map(|(var, sort)| {
                        // Sort names in constraints that reference internal sorts need renaming
                        let new_sort: Name = if internal_names.contains(sort) && !self.params.iter().any(|(p, _)| p == sort) {
                            format!("{}.{}", alias, sort).into()
                        } else {
                            sort.clone()
                        };
                        (var.clone(), new_sort)
                    })
                    .collect(),
            ))
            .collect();

        let rules: Vec<Rule> = self
            .rules
            .iter()
            .map(|r| {
                let mut rule = Rule::new(
                    format!("{}.{}", alias, r.name()),
                    r.premises().iter().map(|p| subst_syms(p, &combined_map)).collect(),
                    subst_syms(r.conclusion(), &combined_map),
                );
                if r.reflected() {
                    rule = rule.with_reflected();
                }
                if let Some(prov) = r.provenance() {
                    rule = rule.with_provenance(prov.clone());
                }
                rule = rule.with_implicit(r.implicit_args().to_vec());
                rule = rule.with_context(
                    r.context_extensions()
                        .iter()
                        .map(|(idx, expr)| (*idx, subst_syms(expr, &combined_map)))
                        .collect(),
                );
                rule
            })
            .collect();

        let rewrites: Vec<RewriteRule> = self
            .rewrites
            .iter()
            .map(|rw| RewriteRule::new(
                format!("{}.{}", alias, rw.name()),
                subst_syms(rw.lhs(), &combined_map),
                subst_syms(rw.rhs(), &combined_map),
            ))
            .collect();

        let binding_specs: Vec<BindingSpec> = self
            .binding_specs
            .iter()
            .map(|bs| BindingSpec {
                name: format!("{}.{}", alias, bs.name).into(),
                ..bs.clone()
            })
            .collect();

        let mut theory = Theory {
            name: format!("{}${}", self.name, alias).into(),
            sorts,
            constructors,
            judgments,
            rules,
            rewrites,
            binding_specs,
            context_mode: self.context_mode,
            content_hash: 0,
            imports: Vec::new(),
            params: Vec::new(), // instance is concrete
            substitutive_binders: self.substitutive_binders.clone(),
            eta_binders: self.eta_binders.clone(),
            linear_binders: self.linear_binders.clone(),
            affine_binders: self.affine_binders.clone(),
            attributes: self.attributes.clone(),
        };
        theory.compute_hash();
        Ok(theory)
    }

    // --- Private helpers ---

    fn validate(&self) -> Result<()> {
        self.check_duplicates()?;
        self.check_rewrites()?;
        Ok(())
    }

    fn check_duplicates(&self) -> Result<()> {
        let mut seen = HashMap::new();

        for s in &self.sorts {
            if seen.insert(("sort", s.name()), ()).is_some() {
                return Err(OmegaError::DuplicateName { kind: DeclKind::Sort, name: s.name().clone() });
            }
        }

        for c in &self.constructors {
            if seen.insert(("ctor", c.name()), ()).is_some() {
                return Err(OmegaError::DuplicateName { kind: DeclKind::Constructor, name: c.name().clone() });
            }
        }

        for r in &self.rules {
            if seen.insert(("rule", r.name()), ()).is_some() {
                return Err(OmegaError::DuplicateName { kind: DeclKind::Rule, name: r.name().clone() });
            }
        }

        for j in &self.judgments {
            if seen.insert(("judgment", j.name()), ()).is_some() {
                return Err(OmegaError::DuplicateName { kind: DeclKind::Judgment, name: j.name().clone() });
            }
        }

        for bs in &self.binding_specs {
            if seen.insert(("binding-spec", &bs.name), ()).is_some() {
                return Err(OmegaError::DuplicateName {
                    kind: DeclKind::BindingSpec,
                    name: bs.name.clone(),
                });
            }
        }

        Ok(())
    }

    fn check_rewrites(&self) -> Result<()> {
        for rw in &self.rewrites {
            let lhs_metas = rw.lhs().meta_vars();
            let rhs_metas = rw.rhs().meta_vars();
            for m in &rhs_metas {
                if !lhs_metas.contains(m) {
                    return Err(OmegaError::RewriteMetaEscape {
                        rule: rw.name().clone(),
                        meta: m.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn compute_hash(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.name.hash(&mut hasher);
        for s in &self.sorts {
            s.name().hash(&mut hasher);
        }
        for c in &self.constructors {
            c.name().hash(&mut hasher);
        }
        for r in &self.rules {
            r.name().hash(&mut hasher);
            r.premises().len().hash(&mut hasher);
        }
        for rw in &self.rewrites {
            rw.name().hash(&mut hasher);
        }
        (self.context_mode as u8).hash(&mut hasher);
        self.content_hash = hasher.finish();
    }
}

impl TheoryBuilder {
    /// The theory name (available during construction for error messages).
    pub fn name(&self) -> &str { &self.name }
    /// All sort declarations.
    pub fn sorts(&self) -> &[SortDecl] { &self.sorts }
    /// All constructor declarations.
    pub fn constructors(&self) -> &[ConstructorDecl] { &self.constructors }
    /// All judgment form declarations.
    pub fn judgments(&self) -> &[JudgmentForm] { &self.judgments }
    /// All inference rules.
    pub fn rules(&self) -> &[Rule] { &self.rules }
    /// All binding specifications (for inspection during construction).
    pub fn binding_specs(&self) -> &[BindingSpec] { &self.binding_specs }
    /// All rewrite rules.
    pub fn rewrites(&self) -> &[RewriteRule] { &self.rewrites }
    /// Import directives (needed for resolution during registration).
    pub fn imports(&self) -> &[Import] { &self.imports }
    /// Theory parameters.
    pub fn params(&self) -> &[(Name, Expr)] { &self.params }

    // --- Mutators ---

    /// Add a sort declaration.
    pub fn add_sort(&mut self, sort: SortDecl) { self.sorts.push(sort); }
    /// Add a constructor declaration.
    pub fn add_constructor(&mut self, ctor: ConstructorDecl) { self.constructors.push(ctor); }
    /// Add a judgment form declaration.
    pub fn add_judgment(&mut self, judgment: JudgmentForm) { self.judgments.push(judgment); }
    /// Push a rule during theory construction.
    pub fn push_rule(&mut self, rule: Rule) { self.rules.push(rule); }
    /// Add a binding specification.
    pub fn add_binding_spec(&mut self, bs: BindingSpec) { self.binding_specs.push(bs); }
    /// Add a rewrite rule.
    pub fn add_rewrite(&mut self, rw: RewriteRule) { self.rewrites.push(rw); }
    /// Set the context mode.
    pub fn set_context_mode(&mut self, mode: ContextMode) { self.context_mode = mode; }
    /// Add an import directive.
    pub fn add_import(&mut self, import: Import) { self.imports.push(import); }
    /// Add a theory parameter.
    pub fn add_param(&mut self, name: Name, ty: Expr) { self.params.push((name, ty)); }
    /// Set all theory parameters at once.
    pub fn set_params(&mut self, params: Vec<(Name, Expr)>) { self.params = params; }
    /// Register a binder kind as substitutive (triggers beta-reduction).
    pub fn add_substitutive_binder(&mut self, name: Name) { self.substitutive_binders.insert(name); }
    /// Register a binder kind for eta-contraction.
    pub fn add_eta_binder(&mut self, name: Name) { self.eta_binders.insert(name); }
    /// Register a binder kind as linear (exactly-once usage).
    pub fn add_linear_binder(&mut self, name: Name) { self.linear_binders.insert(name); }
    /// Register a binder kind as affine (at-most-once usage).
    pub fn add_affine_binder(&mut self, name: Name) { self.affine_binders.insert(name); }
    /// Set an attribute on a symbol.
    pub fn add_attribute(&mut self, sym_name: Name, attr: Attribute) {
        self.attributes.entry(sym_name).or_default().insert(attr);
    }

    /// Merge all declarations from another theory into this builder.
    /// Used for resolving `(import ...)` directives. Errors on name collisions.
    pub fn merge_from(&mut self, other: &Theory) -> Result<()> {
        for s in &other.sorts {
            if self.sorts.iter().any(|x| x.name() == s.name()) {
                return Err(OmegaError::DuplicateName { kind: DeclKind::Sort, name: s.name().clone() });
            }
            self.sorts.push(s.clone());
        }
        for c in &other.constructors {
            if self.constructors.iter().any(|x| x.name() == c.name()) {
                return Err(OmegaError::DuplicateName { kind: DeclKind::Constructor, name: c.name().clone() });
            }
            self.constructors.push(c.clone());
        }
        for j in &other.judgments {
            if self.judgments.iter().any(|x| x.name() == j.name()) {
                return Err(OmegaError::DuplicateName { kind: DeclKind::Judgment, name: j.name().clone() });
            }
            self.judgments.push(j.clone());
        }
        for r in &other.rules {
            if self.rules.iter().any(|x| x.name() == r.name()) {
                return Err(OmegaError::DuplicateName { kind: DeclKind::Rule, name: r.name().clone() });
            }
            self.rules.push(r.clone());
        }
        for rw in &other.rewrites {
            if self.rewrites.iter().any(|x| x.name() == rw.name()) {
                return Err(OmegaError::DuplicateName { kind: DeclKind::Rewrite, name: rw.name().clone() });
            }
            self.rewrites.push(rw.clone());
        }
        for bs in &other.binding_specs {
            if self.binding_specs.iter().any(|x| x.name == bs.name) {
                return Err(OmegaError::DuplicateName { kind: DeclKind::BindingSpec, name: bs.name.clone() });
            }
            self.binding_specs.push(bs.clone());
        }
        // Merge substitutive binders
        for sb in &other.substitutive_binders {
            self.substitutive_binders.insert(sb.clone());
        }
        // Merge eta/linear/affine binders
        for b in &other.eta_binders {
            self.eta_binders.insert(b.clone());
        }
        for b in &other.linear_binders {
            self.linear_binders.insert(b.clone());
        }
        for b in &other.affine_binders {
            self.affine_binders.insert(b.clone());
        }
        // Merge attributes
        for (name, attrs) in &other.attributes {
            self.attributes.entry(name.clone()).or_default().extend(attrs.iter().cloned());
        }
        Ok(())
    }

    /// Validate the theory and finalize it into an immutable [`Theory`].
    ///
    /// This computes the content hash and checks well-formedness (no duplicate
    /// names, rewrite RHS metas present in LHS). If validation fails, returns
    /// an error and the builder can be fixed and retried.
    pub fn build(self) -> Result<Theory> {
        let mut theory = Theory {
            name: self.name,
            sorts: self.sorts,
            constructors: self.constructors,
            judgments: self.judgments,
            rules: self.rules,
            binding_specs: self.binding_specs,
            rewrites: self.rewrites,
            context_mode: self.context_mode,
            content_hash: 0,
            imports: self.imports,
            params: self.params,
            substitutive_binders: self.substitutive_binders,
            eta_binders: self.eta_binders,
            linear_binders: self.linear_binders,
            affine_binders: self.affine_binders,
            attributes: self.attributes,
        };
        theory.validate()?;
        theory.compute_hash();
        Ok(theory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;
    use crate::judgment::{JudgmentForm, Rule, SortDecl};
    use crate::test_util::make_prop_logic;

    #[test]
    fn validate_prop_logic() {
        let theory = make_prop_logic();
        // Theory is already validated by build()
        assert!(theory.content_hash() != 0);
    }

    #[test]
    fn detect_duplicate_sort() {
        let mut tb = Theory::builder("Bad");
        tb.add_sort(SortDecl::new("Prop"));
        tb.add_sort(SortDecl::new("Prop"));
        assert!(matches!(
            tb.build(),
            Err(OmegaError::DuplicateName { kind, .. }) if kind == DeclKind::Sort
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
        let mut derived = Theory::builder("Derived");
        assert!(derived.merge_from(&base).is_ok());
        let derived = derived.build().unwrap();
        // Derived should now have all of base's declarations
        assert!(derived.get_sort("Prop").is_some());
        assert!(derived.get_constructor("and").is_some());
        assert!(derived.get_rule("and-intro").is_some());
        assert!(derived.get_rule("and-elim-l").is_some());
        assert_eq!(derived.sorts().len(), 1);
        assert_eq!(derived.constructors().len(), 3);
        assert_eq!(derived.rules().len(), 5);
    }

    #[test]
    fn merge_from_rejects_collision() {
        let base = make_prop_logic();
        let mut derived = Theory::builder("Derived");
        derived.add_sort(SortDecl::new("Prop")); // Same as base
        let result = derived.merge_from(&base);
        assert!(result.is_err());
    }

    #[test]
    fn instantiate_basic() {
        // A parameterized theory with one sort, one constructor, one rule
        let mut tb = Theory::builder("EqT");
        tb.set_params(vec![
            ("T".into(), Expr::sym("Type")),
            ("eq-T".into(), Expr::app(vec![
                Expr::sym("->"), Expr::sym("T"), Expr::sym("T"), Expr::sym("Prop"),
            ])),
        ]);
        tb.add_sort(SortDecl::new("Prop"));
        tb.add_judgment(JudgmentForm::new(
            "proves",
            Expr::app(vec![Expr::sym("proves"), Expr::meta("P")]),
            vec![("P".into(), "Prop".into())],
        ));
        tb.push_rule(Rule::new(
            "refl",
            vec![],
            Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("eq-T"), Expr::meta("a"), Expr::meta("a")]),
            ]),
        ));
        let theory = tb.build().unwrap();

        let instance = theory.instantiate(
            &[Expr::sym("Nat"), Expr::sym("nat-eq")],
            "NE",
        ).unwrap();

        // Check renamed declarations
        assert_eq!(*instance.sorts()[0].name(), "NE.Prop");
        assert_eq!(*instance.judgments()[0].name(), "NE.proves");
        assert_eq!(*instance.rules()[0].name(), "NE.refl");

        // Check parameter substitution in rule conclusion
        // eq-T should be replaced with nat-eq, proves should be renamed to NE.proves
        let expected_conclusion = Expr::app(vec![
            Expr::sym("NE.proves"),
            Expr::app(vec![Expr::sym("nat-eq"), Expr::meta("a"), Expr::meta("a")]),
        ]);
        assert_eq!(*instance.rules()[0].conclusion(), expected_conclusion);

        // Instance is concrete
        assert!(instance.params().is_empty());
    }

    #[test]
    fn instantiate_wrong_arg_count() {
        let mut tb = Theory::builder("T");
        tb.set_params(vec![("X".into(), Expr::sym("Type"))]);
        let theory = tb.build().unwrap();
        let result = theory.instantiate(&[], "A");
        assert!(result.is_err());
    }
}

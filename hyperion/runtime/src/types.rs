//! Opaque handle types for Hyperion compiler objects.
//!
//! These types represent compiler-internal structures that are passed
//! through generated code but can only be meaningfully inspected or
//! manipulated by the runtime.

/// A theory definition handle. Wraps the theory's name and its rewrite rules
/// as S-expression strings, allowing the runtime to feed them to the e-graph.
#[derive(Debug, Clone, PartialEq)]
pub struct TheoryDef {
    pub name: String,
    pub rules: Vec<(String, String)>, // (lhs, rhs) as S-expression strings
}

/// A sort name handle.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sort(pub String);

/// A cost function handle. Wraps a function pointer or closure that
/// maps expression depth/structure to a numeric cost.
#[derive(Debug, Clone, PartialEq)]
pub struct CostFn {
    pub name: String,
    // The actual cost logic is in the compiled Hyperion code (eval_cost function).
    // This handle just identifies which cost model to use.
}

/// A universe definition handle.
#[derive(Debug, Clone, PartialEq)]
pub struct UniDef {
    pub name: String,
    pub category: String,
    pub substrate: String,
}

/// A category definition handle.
#[derive(Debug, Clone, PartialEq)]
pub struct CatDef {
    pub name: String,
}

/// A substrate definition handle.
#[derive(Debug, Clone, PartialEq)]
pub struct SubDef {
    pub name: String,
}

/// A compilation pass handle.
#[derive(Debug, Clone, PartialEq)]
pub struct PassDef {
    pub name: String,
    pub description: String,
}

/// A rewrite rule definition handle.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleDef {
    pub name: String,
    pub lhs: String, // S-expression string
    pub rhs: String, // S-expression string
}

impl TheoryDef {
    pub fn new(name: &str) -> Self {
        TheoryDef {
            name: name.to_string(),
            rules: Vec::new(),
        }
    }

    pub fn with_rule(mut self, _name: &str, lhs: &str, rhs: &str) -> Self {
        self.rules.push((lhs.to_string(), rhs.to_string()));
        self
    }
}

impl Sort {
    pub fn new(name: &str) -> Self {
        Sort(name.to_string())
    }
}

impl CostFn {
    pub fn new(name: &str) -> Self {
        CostFn { name: name.to_string() }
    }
}

/// Lightweight Rust AST for code generation.
/// Pure data types — no logic.
///
/// Copied from omega's codegen module (no omega-core deps).

/// A complete generated Rust crate.
pub struct RustCrate {
    pub name: String,
    pub modules: Vec<RustModule>,
    /// Whether any pattern uses `box` (requires `#![feature(box_patterns)]`).
    pub has_box_patterns: bool,
}

/// A module within the crate (maps to a file).
pub struct RustModule {
    pub name: String,
    pub items: Vec<RustItem>,
    pub uses: Vec<String>,
}

/// A top-level item in a module.
pub enum RustItem {
    Enum(RustEnum),
    Function(RustFunction),
}

/// A Rust enum definition.
pub struct RustEnum {
    pub name: String,
    pub variants: Vec<RustVariant>,
}

/// A variant of an enum.
pub struct RustVariant {
    pub name: String,
    pub fields: Vec<RustField>,
}

/// A field within a variant (unnamed/positional).
pub struct RustField {
    pub ty: RustType,
}

/// A Rust type.
#[derive(Clone)]
pub enum RustType {
    Named(String),
    Boxed(Box<RustType>),
}

/// A function definition.
pub struct RustFunction {
    pub name: String,
    pub params: Vec<RustParam>,
    pub ret: RustType,
    pub body: RustExpr,
}

/// A function parameter.
pub struct RustParam {
    pub name: String,
    pub ty: RustType,
}

/// A Rust expression.
pub enum RustExpr {
    /// A variable reference.
    Var(String),
    /// Enum constructor: `Type::Variant(args...)`.
    Constructor {
        enum_name: String,
        variant: String,
        args: Vec<RustExpr>,
    },
    /// Function call: `f(args...)`.
    Call {
        func: String,
        args: Vec<RustExpr>,
    },
    /// Match expression.
    Match {
        scrutinee: Box<RustExpr>,
        arms: Vec<RustMatchArm>,
    },
    /// Box::new(expr).
    BoxNew(Box<RustExpr>),
    /// expr.clone().
    Clone(Box<RustExpr>),
    /// unreachable!().
    Unreachable,
}

/// A match arm.
pub struct RustMatchArm {
    pub pattern: RustPattern,
    pub guard: Option<String>,
    pub body: RustExpr,
}

/// A pattern in a match arm.
pub enum RustPattern {
    /// A variable binding.
    Var(String),
    /// Wildcard `_`.
    Wildcard,
    /// Enum pattern: `Type::Variant(pats...)`.
    Constructor {
        enum_name: String,
        variant: String,
        fields: Vec<RustPattern>,
    },
    /// Box pattern: `box inner` (requires `#![feature(box_patterns)]`).
    Box(Box<RustPattern>),
}

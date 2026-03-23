/// Lightweight Rust AST for code generation.
/// Pure data types — no logic.
///
/// Copied from omega's codegen module (no omega-core deps).

/// A complete generated Rust crate.
#[derive(Debug)]
pub struct RustCrate {
    pub name: String,
    pub modules: Vec<RustModule>,
    /// Whether any pattern uses `box` (requires `#![feature(box_patterns)]`).
    pub has_box_patterns: bool,
}

/// A module within the crate (maps to a file).
#[derive(Debug)]
pub struct RustModule {
    pub name: String,
    pub items: Vec<RustItem>,
    pub uses: Vec<String>,
}

/// A top-level item in a module.
#[derive(Debug)]
pub enum RustItem {
    Enum(RustEnum),
    Function(RustFunction),
    Trait(RustTrait),
}

/// A Rust enum definition.
#[derive(Debug)]
pub struct RustEnum {
    pub name: String,
    pub variants: Vec<RustVariant>,
}

/// A variant of an enum.
#[derive(Debug)]
pub struct RustVariant {
    pub name: String,
    pub fields: Vec<RustField>,
}

/// A field within a variant (unnamed/positional).
#[derive(Debug)]
pub struct RustField {
    pub ty: RustType,
}

/// A Rust type.
#[derive(Debug, Clone)]
pub enum RustType {
    Named(String),
    Boxed(Box<RustType>),
    /// Tuple type: `(A, B, ...)`.
    Tuple(Vec<RustType>),
    /// Unit type: `()`.
    Unit,
}

/// A function definition.
#[derive(Debug)]
pub struct RustFunction {
    pub name: String,
    pub params: Vec<RustParam>,
    pub ret: RustType,
    pub body: RustExpr,
    /// If Some, this function takes `&mut impl Trait` as first param
    pub effects_trait: Option<String>,
}

/// A function parameter.
#[derive(Debug)]
pub struct RustParam {
    pub name: String,
    pub ty: RustType,
}

/// A Rust trait definition (generated from Effect sort).
#[derive(Debug)]
pub struct RustTrait {
    pub name: String,
    pub methods: Vec<RustTraitMethod>,
}

/// A method in a trait definition.
#[derive(Debug)]
pub struct RustTraitMethod {
    pub name: String,
    pub params: Vec<RustParam>,
    pub ret: RustType,
}

/// A Rust expression.
#[derive(Debug)]
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
    /// Method call on effects: `effects.method(args...)`.
    EffectCall {
        method: String,
        args: Vec<RustExpr>,
    },
    /// Tuple expression: `(a, b, ...)`.
    TupleExpr(Vec<RustExpr>),
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
#[derive(Debug)]
pub struct RustMatchArm {
    pub pattern: RustPattern,
    pub guard: Option<String>,
    pub body: RustExpr,
}

/// A pattern in a match arm.
#[derive(Debug)]
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
    /// Tuple pattern: `(a, b, ...)`.
    TuplePattern(Vec<RustPattern>),
    /// Box pattern: `box inner` (requires `#![feature(box_patterns)]`).
    Box(Box<RustPattern>),
}

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
    /// If set, emit a main.rs with a runtime harness implementing this trait.
    pub runtime_trait: Option<String>,
    /// Whether any expression uses `rayon::join` for parallel tensor execution.
    pub has_parallel: bool,
    /// Generated tests: each rewrite rule becomes an assert_eq! test.
    pub tests: Vec<RustTest>,
    /// Extra Cargo.toml dependencies: `(name, version)` pairs.
    /// e.g., `("hyperion-runtime", "0.1")` for compiler-engine theories.
    pub extra_deps: Vec<(String, String)>,
    /// Algebraic effect handlers: named implementations of effect traits.
    pub handlers: Vec<RustHandler>,
}

/// An algebraic effect handler: a struct implementing an effect trait.
#[derive(Debug)]
pub struct RustHandler {
    /// The handler struct name (e.g., "DiskHandler").
    pub name: String,
    /// The trait it implements (e.g., "ReflTacticsEffects").
    pub trait_name: String,
    /// Whether the trait is concurrent (uses `&self` vs `&mut self`).
    pub is_concurrent: bool,
    /// Method implementations: each has a name and a body expression.
    pub methods: Vec<RustHandlerMethod>,
}

/// A method implementation in an effect handler.
#[derive(Debug)]
pub struct RustHandlerMethod {
    pub name: String,
    pub params: Vec<RustParam>,
    pub body: RustExpr,
}

/// A generated test case from a rewrite rule.
#[derive(Debug)]
pub struct RustTest {
    pub name: String,
    /// The LHS expression (function call with witness values).
    pub lhs: RustExpr,
    /// The expected RHS expression (constructor/value with witness values).
    pub rhs: RustExpr,
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
    /// If Some, this function takes `&mut impl Trait` (or `&impl Trait + Sync` for concurrent) as first param
    pub effects_trait: Option<String>,
    /// If true, effects param uses `&(impl Trait + Sync)` for thread-safe parallel I/O.
    pub effects_concurrent: bool,
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
    /// If true, trait uses `&self` (not `&mut self`) and requires `Send + Sync`.
    /// Enables parallel effect calls via rayon::join on disjoint linear resources.
    pub is_concurrent: bool,
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
    /// Parallel execution via `rayon::join(|| left, || right)`.
    /// Results are wrapped in the tensor constructor: `{ let (l, r) = rayon::join(...); Enum::Tensor(Box::new(l), Box::new(r)) }`.
    /// Only emitted when the proof-carrying parallel tensor pass verifies
    /// free-variable disjointness (strictly-linear guarantees no sharing).
    Parallel {
        left: Box<RustExpr>,
        right: Box<RustExpr>,
        /// The enum name to wrap the result (e.g., "Val")
        result_enum: String,
        /// The tensor variant name (e.g., "Tensor")
        result_variant: String,
        /// Whether the result sort is recursive (needs Box::new wrapping)
        result_boxed: bool,
    },
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

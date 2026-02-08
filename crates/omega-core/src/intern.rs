/// Hash-consing arena for maximal sharing of expressions.
///
/// Every expression is stored exactly once. Structural equality becomes
/// pointer (index) comparison in O(1). This is critical for large proofs
/// where the same sub-expression appears thousands of times.
///
/// Design:
/// - `HExpr` is a lightweight handle (u32 index) into the arena.
/// - The arena deduplicates on insertion: if you construct `App(f, x)` and
///   it already exists, you get the existing handle back.
/// - The arena is append-only (expressions are never removed).
/// - Thread-local for zero-cost access (no Arc/Mutex).
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

use crate::expr::{BinderKind, Expr, Name};

/// A handle to an interned expression. O(1) equality via index comparison.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HExpr(u32);

impl HExpr {
    /// Get the raw index (for debugging).
    pub fn index(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for HExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "H#{}", self.0)
    }
}

/// The internal representation stored in the arena.
#[derive(Clone, PartialEq, Eq, Hash)]
enum HNode {
    Free(Name),
    Bound(usize),
    Meta(Name),
    Sym(Name),
    App(Vec<HExpr>),
    Binder {
        kind: BinderKind,
        hint: Name,
        ty: HExpr,
        body: HExpr,
    },
    /// User-defined binding specification: `(bind spec body)`
    UserBind {
        spec: Name,
        params: Vec<HExpr>,
        body: HExpr,
    },
}

/// The hash-consing arena.
pub struct Arena {
    /// All stored nodes, indexed by HExpr.
    nodes: Vec<HNode>,
    /// Reverse lookup: node → index.
    dedup: HashMap<HNode, u32>,
    /// Cached: which nodes contain meta-variables.
    has_metas_cache: Vec<Option<bool>>,
}

impl Arena {
    pub fn new() -> Self {
        Arena {
            nodes: Vec::new(),
            dedup: HashMap::new(),
            has_metas_cache: Vec::new(),
        }
    }

    /// Intern a node, returning its handle. If it already exists, returns the existing one.
    fn intern(&mut self, node: HNode) -> HExpr {
        if let Some(&idx) = self.dedup.get(&node) {
            return HExpr(idx);
        }
        let idx = self.nodes.len() as u32;
        self.dedup.insert(node.clone(), idx);
        self.nodes.push(node);
        self.has_metas_cache.push(None);
        HExpr(idx)
    }

    /// Create a free variable.
    pub fn free(&mut self, name: &str) -> HExpr {
        self.intern(HNode::Free(name.to_string()))
    }

    /// Create a bound variable.
    pub fn bound(&mut self, idx: usize) -> HExpr {
        self.intern(HNode::Bound(idx))
    }

    /// Create a meta-variable.
    pub fn meta(&mut self, name: &str) -> HExpr {
        self.intern(HNode::Meta(name.to_string()))
    }

    /// Create a symbol.
    pub fn sym(&mut self, name: &str) -> HExpr {
        self.intern(HNode::Sym(name.to_string()))
    }

    /// Create an application.
    pub fn app(&mut self, args: Vec<HExpr>) -> HExpr {
        self.intern(HNode::App(args))
    }

    /// Create a binder.
    pub fn binder(&mut self, kind: BinderKind, hint: &str, ty: HExpr, body: HExpr) -> HExpr {
        self.intern(HNode::Binder {
            kind,
            hint: hint.to_string(),
            ty,
            body,
        })
    }

    /// Create a user-defined binding form.
    pub fn user_bind(&mut self, spec: &str, params: Vec<HExpr>, body: HExpr) -> HExpr {
        self.intern(HNode::UserBind {
            spec: spec.to_string(),
            params,
            body,
        })
    }

    /// Look up the node for a handle.
    fn node(&self, h: HExpr) -> &HNode {
        &self.nodes[h.0 as usize]
    }

    /// Get the children of an App node, or None if not an App.
    pub fn app_args(&self, h: HExpr) -> Option<Vec<HExpr>> {
        match self.node(h) {
            HNode::App(args) => Some(args.clone()),
            _ => None,
        }
    }

    /// Check if this is a Sym node with the given name.
    pub fn is_sym(&self, h: HExpr, name: &str) -> bool {
        matches!(self.node(h), HNode::Sym(n) if n == name)
    }

    /// Check if an expression contains meta-variables (cached).
    pub fn has_metas(&mut self, h: HExpr) -> bool {
        let idx = h.0 as usize;
        if let Some(cached) = self.has_metas_cache[idx] {
            return cached;
        }
        let result = match self.nodes[idx].clone() {
            HNode::Meta(_) => true,
            HNode::Free(_) | HNode::Bound(_) | HNode::Sym(_) => false,
            HNode::App(args) => args.iter().any(|a| self.has_metas(*a)),
            HNode::Binder { ty, body, .. } => self.has_metas(ty) || self.has_metas(body),
            HNode::UserBind { params, body, .. } => {
                params.iter().any(|p| self.has_metas(*p)) || self.has_metas(body)
            }
        };
        self.has_metas_cache[idx] = Some(result);
        result
    }

    /// Collect meta-variable names from an HExpr (no tree conversion).
    pub fn meta_vars(&self, h: HExpr) -> Vec<Name> {
        let mut result = Vec::new();
        self.meta_vars_inner(h, &mut result);
        result
    }

    fn meta_vars_inner(&self, h: HExpr, acc: &mut Vec<Name>) {
        match self.node(h) {
            HNode::Meta(n) => {
                if !acc.contains(n) {
                    acc.push(n.clone());
                }
            }
            HNode::Free(_) | HNode::Bound(_) | HNode::Sym(_) => {}
            HNode::App(args) => {
                let args = args.clone();
                for a in &args {
                    self.meta_vars_inner(*a, acc);
                }
            }
            HNode::Binder { ty, body, .. } => {
                let (ty, body) = (*ty, *body);
                self.meta_vars_inner(ty, acc);
                self.meta_vars_inner(body, acc);
            }
            HNode::UserBind { params, body, .. } => {
                let params = params.clone();
                let body = *body;
                for p in &params {
                    self.meta_vars_inner(*p, acc);
                }
                self.meta_vars_inner(body, acc);
            }
        }
    }

    /// Bidirectional unification on HExprs (no tree conversion).
    /// Both sides can have metas; returns a substitution if successful.
    pub fn unify_exprs(
        &self,
        a: HExpr,
        b: HExpr,
        subst: &mut HashMap<Name, HExpr>,
    ) -> bool {
        if a == b {
            return true;
        }
        match (self.node(a), self.node(b)) {
            (HNode::Meta(name), _) => {
                if let Some(&existing) = subst.get(name) {
                    existing == b
                } else {
                    subst.insert(name.clone(), b);
                    true
                }
            }
            (_, HNode::Meta(name)) => {
                if let Some(&existing) = subst.get(name) {
                    existing == a
                } else {
                    subst.insert(name.clone(), a);
                    true
                }
            }
            (HNode::Sym(a_n), HNode::Sym(b_n)) => a_n == b_n,
            (HNode::Free(a_n), HNode::Free(b_n)) => a_n == b_n,
            (HNode::Bound(a_i), HNode::Bound(b_i)) => a_i == b_i,
            (HNode::App(aa), HNode::App(ba)) => {
                if aa.len() != ba.len() {
                    return false;
                }
                let aa = aa.clone();
                let ba = ba.clone();
                aa.iter()
                    .zip(ba.iter())
                    .all(|(&x, &y)| self.unify_exprs(x, y, subst))
            }
            (
                HNode::Binder {
                    kind: k1,
                    ty: t1,
                    body: b1,
                    ..
                },
                HNode::Binder {
                    kind: k2,
                    ty: t2,
                    body: b2,
                    ..
                },
            ) => {
                if k1 != k2 {
                    return false;
                }
                let (t1, b1) = (*t1, *b1);
                let (t2, b2) = (*t2, *b2);
                self.unify_exprs(t1, t2, subst) && self.unify_exprs(b1, b2, subst)
            }
            _ => false,
        }
    }

    /// Convert a tree-based Expr into a hash-consed HExpr.
    pub fn from_expr(&mut self, expr: &Expr) -> HExpr {
        match expr {
            Expr::Free(n) => self.free(n),
            Expr::Bound(i) => self.bound(*i),
            Expr::Meta(n) => self.meta(n),
            Expr::Sym(n) => self.sym(n),
            Expr::App(args) => {
                let hargs: Vec<HExpr> = args.iter().map(|a| self.from_expr(a)).collect();
                self.app(hargs)
            }
            Expr::Binder {
                kind,
                hint,
                ty,
                body,
            } => {
                let hty = self.from_expr(ty);
                let hbody = self.from_expr(body);
                self.binder(kind.clone(), hint, hty, hbody)
            }
        }
    }

    /// Convert a hash-consed HExpr back to a tree-based Expr.
    pub fn to_expr(&self, h: HExpr) -> Expr {
        match self.node(h) {
            HNode::Free(n) => Expr::Free(n.clone()),
            HNode::Bound(i) => Expr::Bound(*i),
            HNode::Meta(n) => Expr::Meta(n.clone()),
            HNode::Sym(n) => Expr::Sym(n.clone()),
            HNode::App(args) => Expr::App(args.iter().map(|a| self.to_expr(*a)).collect()),
            HNode::Binder {
                kind,
                hint,
                ty,
                body,
            } => Expr::Binder {
                kind: kind.clone(),
                hint: hint.clone(),
                ty: Box::new(self.to_expr(*ty)),
                body: Box::new(self.to_expr(*body)),
            },
            HNode::UserBind {
                spec,
                params,
                body,
            } => {
                // Convert back as an App with special head
                let mut args = vec![Expr::Sym(format!("@bind:{}", spec))];
                for p in params {
                    args.push(self.to_expr(*p));
                }
                args.push(self.to_expr(*body));
                Expr::App(args)
            }
        }
    }

    /// Apply a meta-substitution to an HExpr (with sharing).
    pub fn apply_meta_subst(&mut self, h: HExpr, subst: &HashMap<Name, HExpr>) -> HExpr {
        if subst.is_empty() {
            return h;
        }
        match self.nodes[h.0 as usize].clone() {
            HNode::Meta(n) => {
                if let Some(&replacement) = subst.get(&n) {
                    replacement
                } else {
                    h
                }
            }
            HNode::Free(_) | HNode::Bound(_) | HNode::Sym(_) => h,
            HNode::App(args) => {
                let new_args: Vec<HExpr> =
                    args.iter().map(|a| self.apply_meta_subst(*a, subst)).collect();
                if new_args == args {
                    h // No change — maximal sharing
                } else {
                    self.app(new_args)
                }
            }
            HNode::Binder {
                kind,
                hint,
                ty,
                body,
            } => {
                let new_ty = self.apply_meta_subst(ty, subst);
                let new_body = self.apply_meta_subst(body, subst);
                if new_ty == ty && new_body == body {
                    h
                } else {
                    self.binder(kind, &hint, new_ty, new_body)
                }
            }
            HNode::UserBind {
                spec,
                params,
                body,
            } => {
                let new_params: Vec<HExpr> =
                    params.iter().map(|p| self.apply_meta_subst(*p, subst)).collect();
                let new_body = self.apply_meta_subst(body, subst);
                if new_params == params && new_body == body {
                    h
                } else {
                    self.user_bind(&spec, new_params, new_body)
                }
            }
        }
    }

    /// Pattern match an HExpr pattern against an HExpr expression.
    pub fn match_expr(
        &self,
        pattern: HExpr,
        expr: HExpr,
    ) -> Result<HashMap<Name, HExpr>, String> {
        let mut subst = HashMap::new();
        self.match_inner(pattern, expr, &mut subst)?;
        Ok(subst)
    }

    fn match_inner(
        &self,
        pattern: HExpr,
        expr: HExpr,
        subst: &mut HashMap<Name, HExpr>,
    ) -> Result<(), String> {
        // Fast path: identical handles mean identical expressions
        if pattern == expr {
            return Ok(());
        }

        match (self.node(pattern), self.node(expr)) {
            (HNode::Meta(name), _) => {
                if let Some(&existing) = subst.get(name) {
                    if existing == expr {
                        Ok(())
                    } else {
                        Err(format!("meta ?{} conflict", name))
                    }
                } else {
                    subst.insert(name.clone(), expr);
                    Ok(())
                }
            }
            (HNode::Sym(a), HNode::Sym(b)) if a == b => Ok(()),
            (HNode::Free(a), HNode::Free(b)) if a == b => Ok(()),
            (HNode::Bound(a), HNode::Bound(b)) if a == b => Ok(()),
            (HNode::App(pa), HNode::App(ea)) => {
                if pa.len() != ea.len() {
                    return Err("arity mismatch".to_string());
                }
                // Clone to avoid borrow issues
                let pa = pa.clone();
                let ea = ea.clone();
                for (p, e) in pa.iter().zip(ea.iter()) {
                    self.match_inner(*p, *e, subst)?;
                }
                Ok(())
            }
            (
                HNode::Binder {
                    kind: k1,
                    ty: t1,
                    body: b1,
                    ..
                },
                HNode::Binder {
                    kind: k2,
                    ty: t2,
                    body: b2,
                    ..
                },
            ) if k1 == k2 => {
                let (t1, b1) = (*t1, *b1);
                let (t2, b2) = (*t2, *b2);
                self.match_inner(t1, t2, subst)?;
                self.match_inner(b1, b2, subst)?;
                Ok(())
            }
            _ => Err(format!("structural mismatch")),
        }
    }

    /// Number of unique expressions stored.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Display an HExpr as a string.
    pub fn display(&self, h: HExpr) -> String {
        match self.node(h) {
            HNode::Free(n) => n.clone(),
            HNode::Bound(i) => format!("#{}", i),
            HNode::Meta(n) => format!("?{}", n),
            HNode::Sym(n) => n.clone(),
            HNode::App(args) => {
                let inner: Vec<String> = args.iter().map(|a| self.display(*a)).collect();
                format!("({})", inner.join(" "))
            }
            HNode::Binder {
                kind,
                hint,
                ty,
                body,
            } => {
                let kw = match kind {
                    BinderKind::Lambda => "lambda",
                    BinderKind::Forall => "forall",
                    BinderKind::Arrow => "->",
                };
                format!(
                    "({} ({} : {}) {})",
                    kw,
                    hint,
                    self.display(*ty),
                    self.display(*body)
                )
            }
            HNode::UserBind {
                spec,
                params,
                body,
            } => {
                let ps: Vec<String> = params.iter().map(|p| self.display(*p)).collect();
                if ps.is_empty() {
                    format!("(@bind:{} {})", spec, self.display(*body))
                } else {
                    format!("(@bind:{} {} {})", spec, ps.join(" "), self.display(*body))
                }
            }
        }
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

// Thread-local arena for convenient access
thread_local! {
    static GLOBAL_ARENA: RefCell<Arena> = RefCell::new(Arena::new());
}

/// Access the thread-local arena.
pub fn with_arena<F, R>(f: F) -> R
where
    F: FnOnce(&mut Arena) -> R,
{
    GLOBAL_ARENA.with(|arena| f(&mut arena.borrow_mut()))
}

/// Intern an Expr into the global arena.
pub fn intern(expr: &Expr) -> HExpr {
    with_arena(|a| a.from_expr(expr))
}

/// Retrieve an Expr from the global arena.
pub fn retrieve(h: HExpr) -> Expr {
    with_arena(|a| a.to_expr(h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;

    #[test]
    fn deduplication() {
        let mut arena = Arena::new();
        let a1 = arena.sym("and");
        let a2 = arena.sym("and");
        assert_eq!(a1, a2); // Same handle
        assert_eq!(arena.len(), 1); // Only stored once
    }

    #[test]
    fn complex_dedup() {
        let mut arena = Arena::new();
        let proves = arena.sym("proves");
        let and = arena.sym("and");
        let a = arena.meta("A");
        let b = arena.meta("B");

        // Build (proves (and ?A ?B)) twice
        let inner1 = arena.app(vec![and, a, b]);
        let outer1 = arena.app(vec![proves, inner1]);

        let inner2 = arena.app(vec![and, a, b]);
        let outer2 = arena.app(vec![proves, inner2]);

        assert_eq!(outer1, outer2);
        assert_eq!(inner1, inner2);
    }

    #[test]
    fn roundtrip() {
        let mut arena = Arena::new();
        let expr = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ]);
        let h = arena.from_expr(&expr);
        let back = arena.to_expr(h);
        assert_eq!(expr, back);
    }

    #[test]
    fn equality_is_o1() {
        let mut arena = Arena::new();
        // Build a large expression
        let mut e = arena.sym("base");
        for _ in 0..100 {
            let f = arena.sym("f");
            e = arena.app(vec![f, e]);
        }
        // Build the same thing again
        let mut e2 = arena.sym("base");
        for _ in 0..100 {
            let f = arena.sym("f");
            e2 = arena.app(vec![f, e2]);
        }
        // Equality is just comparing two u32s
        assert_eq!(e, e2);
    }

    #[test]
    fn pattern_matching() {
        let mut arena = Arena::new();
        let pat = arena.from_expr(&Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ]));
        let expr = arena.from_expr(&Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![
                Expr::sym("and"),
                Expr::free("p"),
                Expr::free("q"),
            ]),
        ]));

        let subst = arena.match_expr(pat, expr).unwrap();
        let p = arena.free("p");
        let q = arena.free("q");
        assert_eq!(subst.get("A"), Some(&p));
        assert_eq!(subst.get("B"), Some(&q));
    }

    #[test]
    fn meta_substitution_with_sharing() {
        let mut arena = Arena::new();
        let orig = arena.from_expr(&Expr::app(vec![
            Expr::sym("proves"),
            Expr::meta("A"),
        ]));
        let replacement = arena.sym("true");
        let mut subst = HashMap::new();
        subst.insert("A".to_string(), replacement);

        let result = arena.apply_meta_subst(orig, &subst);
        let expected = arena.from_expr(&Expr::app(vec![
            Expr::sym("proves"),
            Expr::sym("true"),
        ]));
        assert_eq!(result, expected);
    }

    #[test]
    fn sharing_stats() {
        let mut arena = Arena::new();
        // Build 1000 copies of (f (g x))
        let x = arena.sym("x");
        let g = arena.sym("g");
        let f = arena.sym("f");
        let gx = arena.app(vec![g, x]);
        let fgx = arena.app(vec![f, gx]);

        // Building it "again" 1000 times doesn't allocate
        for _ in 0..1000 {
            let x2 = arena.sym("x");
            let g2 = arena.sym("g");
            let f2 = arena.sym("f");
            let gx2 = arena.app(vec![g2, x2]);
            let fgx2 = arena.app(vec![f2, gx2]);
            assert_eq!(fgx, fgx2);
        }

        // Only 5 unique nodes: x, g, f, (g x), (f (g x))
        assert_eq!(arena.len(), 5);
    }

    #[test]
    fn user_binding() {
        let mut arena = Arena::new();
        let body = arena.bound(0);
        let ty = arena.sym("Nat");
        let ub = arena.user_bind("pi", vec![ty], body);
        let display = arena.display(ub);
        assert_eq!(display, "(@bind:pi Nat #0)");
    }
}

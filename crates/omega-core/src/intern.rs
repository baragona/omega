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
use std::collections::{HashMap, HashSet};
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
    /// Structural hash per node — for stable AC ordering.
    structural_hash: Vec<u64>,
    /// Symbols declared as associative-commutative.
    pub ac_symbols: HashSet<Name>,
    /// Symbols declared as AC + idempotent.
    pub aci_symbols: HashSet<Name>,
}

impl Arena {
    pub fn new() -> Self {
        Arena {
            nodes: Vec::new(),
            dedup: HashMap::new(),
            has_metas_cache: Vec::new(),
            structural_hash: Vec::new(),
            ac_symbols: HashSet::new(),
            aci_symbols: HashSet::new(),
        }
    }

    /// Intern a node, returning its handle. If it already exists, returns the existing one.
    fn intern(&mut self, node: HNode) -> HExpr {
        if let Some(&idx) = self.dedup.get(&node) {
            return HExpr(idx);
        }
        let idx = self.nodes.len() as u32;
        let h = self.compute_structural_hash(&node);
        self.dedup.insert(node.clone(), idx);
        self.nodes.push(node);
        self.has_metas_cache.push(None);
        self.structural_hash.push(h);
        HExpr(idx)
    }

    // Large random primes for salting — avoid collisions between node kinds
    const META_SALT: u64 = 0x9E3779B97F4A7C15;
    const FREE_SALT: u64 = 0x517CC1B727220A95;
    const BOUND_SALT: u64 = 0x6C62272E07BB0142;
    const BINDER_SALT: u64 = 0xBF58476D1CE4E5B9;
    const USERBIND_SALT: u64 = 0xD1342543DE82EF95;

    fn hash_str(s: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325; // FNV offset
        for b in s.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3); // FNV prime
        }
        h
    }

    fn compute_structural_hash(&self, node: &HNode) -> u64 {
        match node {
            HNode::Sym(n) => Self::hash_str(n),
            HNode::Meta(n) => Self::hash_str(n) ^ Self::META_SALT,
            HNode::Free(n) => Self::hash_str(n) ^ Self::FREE_SALT,
            HNode::Bound(i) => (*i as u64).wrapping_mul(Self::BOUND_SALT),
            HNode::App(args) => args.iter()
                .fold(0u64, |acc, a| acc.wrapping_mul(31) ^ self.structural_hash[a.0 as usize]),
            HNode::Binder { kind, ty, body, .. } =>
                Self::hash_str(kind).wrapping_mul(Self::BINDER_SALT)
                ^ self.structural_hash[ty.0 as usize].wrapping_mul(17)
                ^ self.structural_hash[body.0 as usize],
            HNode::UserBind { spec, params, body } => {
                let mut h = Self::hash_str(spec) ^ Self::USERBIND_SALT;
                for p in params {
                    h = h.wrapping_mul(31) ^ self.structural_hash[p.0 as usize];
                }
                h ^ self.structural_hash[body.0 as usize]
            }
        }
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

    /// Create an application. If the head is an AC/ACI symbol, canonicalize.
    pub fn app(&mut self, args: Vec<HExpr>) -> HExpr {
        // Check for AC/ACI canonicalization: binary op App([head, a, b])
        if args.len() == 3 {
            if let HNode::Sym(ref name) = self.nodes[args[0].0 as usize] {
                let is_aci = self.aci_symbols.contains(name);
                let is_ac = is_aci || self.ac_symbols.contains(name);
                if is_ac {
                    let op_name = name.clone();
                    return self.intern_app_ac(&op_name, vec![args[1], args[2]], is_aci);
                }
            }
        }
        self.intern(HNode::App(args))
    }

    /// Intern directly without AC canonicalization (used during rebuild).
    fn raw_app(&mut self, args: Vec<HExpr>) -> HExpr {
        self.intern(HNode::App(args))
    }

    /// Flatten nested AC applications into a spine of operands.
    fn flatten_ac(&self, op_name: &str, h: HExpr, out: &mut Vec<HExpr>) {
        if let HNode::App(ref args) = self.nodes[h.0 as usize] {
            if args.len() == 3 {
                if let HNode::Sym(ref name) = self.nodes[args[0].0 as usize] {
                    if name == op_name {
                        self.flatten_ac(op_name, args[1], out);
                        self.flatten_ac(op_name, args[2], out);
                        return;
                    }
                }
            }
        }
        out.push(h);
    }

    /// Recursive structural comparison for tie-breaking when hashes collide.
    fn structural_cmp(&self, a: HExpr, b: HExpr) -> std::cmp::Ordering {
        if a == b {
            return std::cmp::Ordering::Equal;
        }
        use std::cmp::Ordering::*;
        let node_ord = |n: &HNode| -> u8 {
            match n {
                HNode::Sym(_) => 0,
                HNode::Bound(_) => 1,
                HNode::Meta(_) => 2,
                HNode::Free(_) => 3,
                HNode::App(_) => 4,
                HNode::Binder { .. } => 5,
                HNode::UserBind { .. } => 6,
            }
        };
        let na = &self.nodes[a.0 as usize];
        let nb = &self.nodes[b.0 as usize];
        let oa = node_ord(na);
        let ob = node_ord(nb);
        if oa != ob {
            return oa.cmp(&ob);
        }
        match (na, nb) {
            (HNode::Sym(sa), HNode::Sym(sb)) => sa.cmp(sb),
            (HNode::Bound(ia), HNode::Bound(ib)) => ia.cmp(ib),
            (HNode::Meta(ma), HNode::Meta(mb)) => ma.cmp(mb),
            (HNode::Free(fa), HNode::Free(fb)) => fa.cmp(fb),
            (HNode::App(aa), HNode::App(ab)) => {
                for (x, y) in aa.iter().zip(ab.iter()) {
                    let c = self.structural_cmp(*x, *y);
                    if c != Equal { return c; }
                }
                aa.len().cmp(&ab.len())
            }
            _ => a.0.cmp(&b.0), // fallback for complex cases
        }
    }

    /// AC-canonicalize: flatten → sort by structural hash → deduplicate (if ACI) → rebuild right-associative.
    fn intern_app_ac(&mut self, op_name: &str, operands: Vec<HExpr>, idempotent: bool) -> HExpr {
        // 1. Flatten
        let mut spine = Vec::new();
        for arg in &operands {
            self.flatten_ac(op_name, *arg, &mut spine);
        }

        // 2. Pre-compute has_metas for each spine element (needed for stable AC ordering).
        //    Meta-containing operands sort LAST so that patterns like (op concrete ?x)
        //    have the same concrete-first ordering as fully-concrete terms.
        let meta_flags: Vec<bool> = spine.iter().map(|h| self.has_metas(*h)).collect();

        // 3. Sort: concrete operands first (by structural hash), meta operands last (by structural hash)
        let hashes = &self.structural_hash;
        let mut indices: Vec<usize> = (0..spine.len()).collect();
        indices.sort_by(|&ai, &bi| {
            let a = spine[ai];
            let b = spine[bi];
            let a_meta = meta_flags[ai];
            let b_meta = meta_flags[bi];
            // Concrete before meta
            match (a_meta, b_meta) {
                (false, true) => return std::cmp::Ordering::Less,
                (true, false) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            let ha = hashes[a.0 as usize];
            let hb = hashes[b.0 as usize];
            ha.cmp(&hb).then_with(|| self.structural_cmp(a, b))
        });
        spine = indices.into_iter().map(|i| spine[i]).collect();

        // 3. Deduplicate if idempotent
        if idempotent {
            spine.dedup();
        }

        // 4. Handle degenerate cases
        if spine.len() == 1 {
            return spine[0];
        }
        if spine.is_empty() {
            // shouldn't happen in practice
            return self.sym(op_name);
        }

        // 5. Rebuild right-associative: (op a (op b (op c d)))
        let head = self.sym(op_name);
        let mut result = *spine.last().unwrap();
        for item in spine[..spine.len() - 1].iter().rev() {
            result = self.raw_app(vec![head, *item, result]);
        }
        result
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
    /// Includes WHNF reduction and Miller fragment for arity mismatch.
    pub fn unify_exprs(
        &mut self,
        a: HExpr,
        b: HExpr,
        subst: &mut HashMap<Name, HExpr>,
    ) -> bool {
        // WHNF both sides
        let a = self.whnf(a);
        let b = self.whnf(b);
        if a == b {
            return true;
        }
        // Clone nodes to avoid borrow conflicts
        let a_node = self.nodes[a.0 as usize].clone();
        let b_node = self.nodes[b.0 as usize].clone();

        match (&a_node, &b_node) {
            (HNode::Meta(name), _) => {
                if let Some(&existing) = subst.get(name) {
                    self.unify_exprs(existing, b, subst)
                } else {
                    // Occurs check
                    if self.meta_vars(b).contains(name) {
                        return false;
                    }
                    subst.insert(name.clone(), b);
                    true
                }
            }
            (_, HNode::Meta(name)) => {
                if let Some(&existing) = subst.get(name) {
                    self.unify_exprs(a, existing, subst)
                } else {
                    if self.meta_vars(a).contains(name) {
                        return false;
                    }
                    subst.insert(name.clone(), a);
                    true
                }
            }
            (HNode::Sym(a_n), HNode::Sym(b_n)) => a_n == b_n,
            (HNode::Free(a_n), HNode::Free(b_n)) => a_n == b_n,
            (HNode::Bound(a_i), HNode::Bound(b_i)) => a_i == b_i,
            (HNode::App(aa), HNode::App(ba)) => {
                if aa.len() != ba.len() {
                    // Miller fragment: try if either side is meta-headed
                    if let HNode::Meta(m) = self.nodes[aa[0].0 as usize].clone() {
                        let args: Vec<HExpr> = aa[1..].to_vec();
                        if self.try_miller_unify_h(&m, &args, b, subst) {
                            return true;
                        }
                    }
                    if let HNode::Meta(m) = self.nodes[ba[0].0 as usize].clone() {
                        let args: Vec<HExpr> = ba[1..].to_vec();
                        if self.try_miller_unify_h(&m, &args, a, subst) {
                            return true;
                        }
                    }
                    return false;
                }
                let aa = aa.clone();
                let ba = ba.clone();

                // Check if this is a binary AC/ACI operator — try swapped order on failure
                let is_binary_ac = aa.len() == 3 && {
                    if let HNode::Sym(ref name) = self.nodes[aa[0].0 as usize] {
                        self.ac_symbols.contains(name) || self.aci_symbols.contains(name)
                    } else {
                        false
                    }
                };

                if is_binary_ac {
                    // Attempt 1: direct positional
                    let subst_backup = subst.clone();
                    let direct_ok = aa.iter()
                        .zip(ba.iter())
                        .all(|(&x, &y)| self.unify_exprs(x, y, subst));

                    if direct_ok {
                        return true;
                    }

                    // Revert and try swapped operands
                    *subst = subst_backup;
                    self.unify_exprs(aa[0], ba[0], subst)
                        && self.unify_exprs(aa[1], ba[2], subst)
                        && self.unify_exprs(aa[2], ba[1], subst)
                } else {
                    aa.iter()
                        .zip(ba.iter())
                        .all(|(&x, &y)| self.unify_exprs(x, y, subst))
                }
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
                // Skip type comparison if either side is the wildcard `_`
                let t1_wild = matches!(self.nodes[t1.0 as usize], HNode::Sym(ref s) if s == "_");
                let t2_wild = matches!(self.nodes[t2.0 as usize], HNode::Sym(ref s) if s == "_");
                let ty_ok = t1_wild || t2_wild || self.unify_exprs(t1, t2, subst);
                ty_ok && self.unify_exprs(b1, b2, subst)
            }
            _ => false,
        }
    }

    /// Miller pattern fragment for HExpr unification.
    fn try_miller_unify_h(
        &mut self,
        meta_name: &str,
        args: &[HExpr],
        target: HExpr,
        subst: &mut HashMap<Name, HExpr>,
    ) -> bool {
        // Apply current subst to each arg
        let resolved_args: Vec<HExpr> = args
            .iter()
            .map(|&a| {
                if let HNode::Meta(m) = self.nodes[a.0 as usize].clone() {
                    if let Some(&v) = subst.get(&m) { v } else { a }
                } else { a }
            })
            .collect();

        // Check strict Miller condition
        let mut arg_values = Vec::new();
        for &arg in &resolved_args {
            match self.nodes[arg.0 as usize].clone() {
                HNode::Free(_) | HNode::Bound(_) | HNode::Sym(_) => {
                    if arg_values.contains(&arg) { return false; }
                    arg_values.push(arg);
                }
                HNode::Meta(m) => {
                    let target_vars = self.abstractable_vars(target);
                    let mut found = false;
                    for &candidate in &target_vars {
                        if !arg_values.contains(&candidate) {
                            subst.insert(m.clone(), candidate);
                            arg_values.push(candidate);
                            found = true;
                            break;
                        }
                    }
                    if !found { return false; }
                }
                _ => return false,
            }
        }

        // Abstract and wrap
        let mut body = target;
        for &arg_val in arg_values.iter().rev() {
            body = self.abstract_over(body, arg_val, 0);
        }
        let mut result = body;
        for (i, _) in arg_values.iter().enumerate().rev() {
            let hint = format!("x{}", i);
            let ty = self.sym("_");
            result = self.binder(crate::expr::LAMBDA.to_string(),&hint, ty, result);
        }

        if let Some(&existing) = subst.get(meta_name) {
            existing == result
        } else {
            subst.insert(meta_name.to_string(), result);
            true
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
    /// Depth-aware: shifts replacements when substituting under binders,
    /// so that rewrite rule RHS terms containing Bound vars are safe.
    pub fn apply_meta_subst(&mut self, h: HExpr, subst: &HashMap<Name, HExpr>) -> HExpr {
        self.apply_meta_subst_depth(h, subst, 0)
    }

    fn apply_meta_subst_depth(&mut self, h: HExpr, subst: &HashMap<Name, HExpr>, depth: usize) -> HExpr {
        if subst.is_empty() {
            return h;
        }
        match self.nodes[h.0 as usize].clone() {
            HNode::Meta(n) => {
                if let Some(&replacement) = subst.get(&n) {
                    if depth > 0 {
                        self.shift_pub(replacement, 0, depth as i32)
                    } else {
                        replacement
                    }
                } else {
                    h
                }
            }
            HNode::Free(_) | HNode::Bound(_) | HNode::Sym(_) => h,
            HNode::App(args) => {
                let new_args: Vec<HExpr> =
                    args.iter().map(|a| self.apply_meta_subst_depth(*a, subst, depth)).collect();
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
                let new_ty = self.apply_meta_subst_depth(ty, subst, depth);
                let new_body = self.apply_meta_subst_depth(body, subst, depth + 1);
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
                    params.iter().map(|p| self.apply_meta_subst_depth(*p, subst, depth)).collect();
                let new_body = self.apply_meta_subst_depth(body, subst, depth);
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
        &mut self,
        pattern: HExpr,
        expr: HExpr,
    ) -> Result<HashMap<Name, HExpr>, String> {
        let mut subst = HashMap::new();
        self.match_inner(pattern, expr, &mut subst)?;
        Ok(subst)
    }

    fn match_inner(
        &mut self,
        pattern: HExpr,
        expr: HExpr,
        subst: &mut HashMap<Name, HExpr>,
    ) -> Result<(), String> {
        // Fast path: identical handles mean identical expressions
        if pattern == expr {
            return Ok(());
        }

        // Clone nodes to avoid borrow conflicts with &mut self
        let pat_node = self.nodes[pattern.0 as usize].clone();
        let expr_node = self.nodes[expr.0 as usize].clone();

        match (&pat_node, &expr_node) {
            (HNode::Meta(name), _) => {
                if let Some(&existing) = subst.get(name) {
                    if existing == expr {
                        Ok(())
                    } else {
                        // Try structural match (handles wildcard _ in binder types)
                        let mut dummy_subst = HashMap::new();
                        if self.match_inner(existing, expr, &mut dummy_subst).is_ok()
                            && dummy_subst.is_empty()
                        {
                            Ok(())
                        } else {
                            // Try WHNF comparison for beta-equivalent terms
                            let existing_whnf = self.whnf(existing);
                            let expr_whnf = self.whnf(expr);
                            if existing_whnf == expr_whnf {
                                Ok(())
                            } else {
                                // Try structural match on WHNF'd terms
                                let mut dummy_subst2 = HashMap::new();
                                if self.match_inner(existing_whnf, expr_whnf, &mut dummy_subst2).is_ok()
                                    && dummy_subst2.is_empty()
                                {
                                    Ok(())
                                } else {
                                    Err(format!("meta ?{} conflict", name))
                                }
                            }
                        }
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
                    // Miller fragment: check if pattern head is a meta
                    if let HNode::Meta(meta_name) = self.nodes[pa[0].0 as usize].clone() {
                        let args: Vec<HExpr> = pa[1..].to_vec();
                        return self.try_miller_match_h(&meta_name, &args, expr, subst);
                    }
                    return Err("arity mismatch".to_string());
                }
                let pa = pa.clone();
                let ea = ea.clone();

                // Check if this is a binary AC/ACI operator — try swapped order on failure
                let is_binary_ac = pa.len() == 3 && {
                    if let HNode::Sym(ref name) = self.nodes[pa[0].0 as usize] {
                        self.ac_symbols.contains(name) || self.aci_symbols.contains(name)
                    } else {
                        false
                    }
                };

                if is_binary_ac {
                    // Attempt 1: direct positional match
                    let subst_backup = subst.clone();
                    let direct_ok = (|| {
                        for (p, e) in pa.iter().zip(ea.iter()) {
                            self.match_inner(*p, *e, subst)?;
                        }
                        Ok::<(), String>(())
                    })();

                    if direct_ok.is_ok() {
                        return Ok(());
                    }

                    // Revert partial bindings from failed attempt
                    *subst = subst_backup;

                    // Attempt 2: swap the two operands (head stays, args swap)
                    self.match_inner(pa[0], ea[0], subst)?; // heads must match
                    self.match_inner(pa[1], ea[2], subst)?;
                    self.match_inner(pa[2], ea[1], subst)?;
                    Ok(())
                } else {
                    for (p, e) in pa.iter().zip(ea.iter()) {
                        self.match_inner(*p, *e, subst)?;
                    }
                    Ok(())
                }
            }
            (HNode::App(pa), _) => {
                // Pattern is App but expr is not: might be Miller if meta-headed
                if pa.len() >= 2 {
                    if let HNode::Meta(meta_name) = self.nodes[pa[0].0 as usize].clone() {
                        let args: Vec<HExpr> = pa[1..].to_vec();
                        return self.try_miller_match_h(&meta_name, &args, expr, subst);
                    }
                }
                Err("structural mismatch".to_string())
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
                // Skip type comparison if either side is the wildcard `_`
                let t1_wild = matches!(self.nodes[t1.0 as usize], HNode::Sym(ref s) if s == "_");
                let t2_wild = matches!(self.nodes[t2.0 as usize], HNode::Sym(ref s) if s == "_");
                if !t1_wild && !t2_wild {
                    self.match_inner(t1, t2, subst)?;
                }
                self.match_inner(b1, b2, subst)?;
                Ok(())
            }
            _ => Err("structural mismatch".to_string()),
        }
    }

    /// Miller pattern match on HExprs.
    fn try_miller_match_h(
        &mut self,
        meta_name: &str,
        args: &[HExpr],
        target: HExpr,
        subst: &mut HashMap<Name, HExpr>,
    ) -> Result<(), String> {
        // Apply current subst to each arg
        let resolved_args: Vec<HExpr> = args
            .iter()
            .map(|&a| {
                match self.node(a) {
                    HNode::Meta(m) => {
                        let m = m.clone();
                        if let Some(&v) = subst.get(&m) { v } else { a }
                    }
                    _ => a,
                }
            })
            .collect();

        // Check strict Miller condition
        let mut arg_values = Vec::new();
        for &arg in &resolved_args {
            match self.node(arg).clone() {
                HNode::Free(_) | HNode::Bound(_) | HNode::Sym(_) => {
                    if arg_values.contains(&arg) {
                        return Err("Miller: duplicate arg".to_string());
                    }
                    arg_values.push(arg);
                }
                HNode::Meta(m) => {
                    let target_vars = self.abstractable_vars(target);
                    let mut found = false;
                    for &candidate in &target_vars {
                        if !arg_values.contains(&candidate) {
                            subst.insert(m.clone(), candidate);
                            arg_values.push(candidate);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(format!("Miller: no candidate for ?{}", m));
                    }
                }
                _ => return Err("Miller: compound arg".to_string()),
            }
        }

        // Abstract each arg from target and wrap in lambdas
        let mut body = target;
        for &arg_val in arg_values.iter().rev() {
            body = self.abstract_over(body, arg_val, 0);
        }
        let mut result = body;
        for (i, _) in arg_values.iter().enumerate().rev() {
            let hint = format!("x{}", i);
            let ty = self.sym("_");
            result = self.binder(crate::expr::LAMBDA.to_string(),&hint, ty, result);
        }

        // Check existing binding
        if let Some(&existing) = subst.get(meta_name) {
            if existing == result {
                Ok(())
            } else {
                Err(format!("meta ?{} conflict", meta_name))
            }
        } else {
            subst.insert(meta_name.to_string(), result);
            Ok(())
        }
    }

    /// Construct a lambda binder.
    pub fn make_lambda(&mut self, hint: &str, body: HExpr) -> HExpr {
        let ty = self.sym("_");
        self.binder(crate::expr::LAMBDA.to_string(),hint, ty, body)
    }

    /// Replace all occurrences of `target` in `expr` with Bound(depth).
    /// Shifts existing bound vars >= depth up by 1 to avoid capture.
    pub fn abstract_over(&mut self, expr: HExpr, target: HExpr, depth: usize) -> HExpr {
        if expr == target {
            return self.bound(depth);
        }
        match self.nodes[expr.0 as usize].clone() {
            HNode::Free(_) | HNode::Meta(_) | HNode::Sym(_) => expr,
            HNode::Bound(i) => {
                if i >= depth {
                    self.bound(i + 1)
                } else {
                    expr
                }
            }
            HNode::App(args) => {
                let new_args: Vec<HExpr> = args
                    .iter()
                    .map(|&a| self.abstract_over(a, target, depth))
                    .collect();
                if new_args == args {
                    expr
                } else {
                    self.app(new_args)
                }
            }
            HNode::Binder { kind, hint, ty, body } => {
                let new_ty = self.abstract_over(ty, target, depth);
                let new_body = self.abstract_over(body, target, depth + 1);
                if new_ty == ty && new_body == body {
                    expr
                } else {
                    self.binder(kind, &hint, new_ty, new_body)
                }
            }
            HNode::UserBind { spec, params, body } => {
                let new_params: Vec<HExpr> = params
                    .iter()
                    .map(|&p| self.abstract_over(p, target, depth))
                    .collect();
                let new_body = self.abstract_over(body, target, depth);
                if new_params == params && new_body == body {
                    expr
                } else {
                    self.user_bind(&spec, new_params, new_body)
                }
            }
        }
    }

    /// Open: replace Bound(index) with replacement in expr.
    pub fn open(&mut self, expr: HExpr, index: usize, replacement: HExpr) -> HExpr {
        match self.nodes[expr.0 as usize].clone() {
            HNode::Bound(i) => {
                if i == index {
                    replacement
                } else {
                    expr
                }
            }
            HNode::Free(_) | HNode::Meta(_) | HNode::Sym(_) => expr,
            HNode::App(args) => {
                let shifted_rep = self.shift(replacement, 0, 0); // no shift needed at this level
                let _ = shifted_rep;
                let new_args: Vec<HExpr> = args
                    .iter()
                    .map(|&a| self.open(a, index, replacement))
                    .collect();
                if new_args == args {
                    expr
                } else {
                    self.app(new_args)
                }
            }
            HNode::Binder { kind, hint, ty, body } => {
                let new_ty = self.open(ty, index, replacement);
                let shifted_rep = self.shift(replacement, 0, 1);
                let new_body = self.open(body, index + 1, shifted_rep);
                if new_ty == ty && new_body == body {
                    expr
                } else {
                    self.binder(kind, &hint, new_ty, new_body)
                }
            }
            HNode::UserBind { spec, params, body } => {
                let new_params: Vec<HExpr> = params
                    .iter()
                    .map(|&p| self.open(p, index, replacement))
                    .collect();
                let new_body = self.open(body, index, replacement);
                if new_params == params && new_body == body {
                    expr
                } else {
                    self.user_bind(&spec, new_params, new_body)
                }
            }
        }
    }

    /// Shift bound variable indices >= cutoff by amount (public version).
    pub fn shift_pub(&mut self, expr: HExpr, cutoff: usize, amount: i32) -> HExpr {
        self.shift(expr, cutoff, amount)
    }

    /// Shift bound variable indices >= cutoff by amount.
    fn shift(&mut self, expr: HExpr, cutoff: usize, amount: i32) -> HExpr {
        match self.nodes[expr.0 as usize].clone() {
            HNode::Bound(i) => {
                if i >= cutoff {
                    self.bound((i as i32 + amount) as usize)
                } else {
                    expr
                }
            }
            HNode::Free(_) | HNode::Meta(_) | HNode::Sym(_) => expr,
            HNode::App(args) => {
                let new_args: Vec<HExpr> = args
                    .iter()
                    .map(|&a| self.shift(a, cutoff, amount))
                    .collect();
                if new_args == args {
                    expr
                } else {
                    self.app(new_args)
                }
            }
            HNode::Binder { kind, hint, ty, body } => {
                let new_ty = self.shift(ty, cutoff, amount);
                let new_body = self.shift(body, cutoff + 1, amount);
                if new_ty == ty && new_body == body {
                    expr
                } else {
                    self.binder(kind, &hint, new_ty, new_body)
                }
            }
            HNode::UserBind { spec, params, body } => {
                let new_params: Vec<HExpr> = params
                    .iter()
                    .map(|&p| self.shift(p, cutoff, amount))
                    .collect();
                let new_body = self.shift(body, cutoff, amount);
                if new_params == params && new_body == body {
                    expr
                } else {
                    self.user_bind(&spec, new_params, new_body)
                }
            }
        }
    }

    /// Weak Head Normal Form: reduce head beta-redexes only.
    pub fn whnf(&mut self, expr: HExpr) -> HExpr {
        match self.nodes[expr.0 as usize].clone() {
            HNode::App(args) if args.len() >= 2 => {
                let head = self.whnf(args[0]);
                match self.nodes[head.0 as usize].clone() {
                    HNode::Binder { ref kind, body, .. } if kind == crate::expr::LAMBDA => {
                        let reduced = self.open(body, 0, args[1]);
                        if args.len() == 2 {
                            self.whnf(reduced)
                        } else {
                            let mut new_args = vec![reduced];
                            new_args.extend_from_slice(&args[2..]);
                            let app = self.app(new_args);
                            self.whnf(app)
                        }
                    }
                    _ => {
                        if head != args[0] {
                            let mut new_args = vec![head];
                            new_args.extend_from_slice(&args[1..]);
                            self.app(new_args)
                        } else {
                            expr
                        }
                    }
                }
            }
            _ => expr,
        }
    }

    /// Full beta-normalize an HExpr (for definitional equality only).
    pub fn beta_normalize(&mut self, expr: HExpr, fuel: &mut usize) -> HExpr {
        if *fuel == 0 {
            return expr;
        }
        match self.nodes[expr.0 as usize].clone() {
            HNode::Free(_) | HNode::Bound(_) | HNode::Meta(_) | HNode::Sym(_) => expr,
            HNode::App(args) => {
                *fuel = fuel.saturating_sub(1);
                let normalized: Vec<HExpr> = args
                    .iter()
                    .map(|&a| self.beta_normalize(a, fuel))
                    .collect();
                if normalized.len() >= 2 {
                    if let HNode::Binder { ref kind, body, .. } =
                        self.nodes[normalized[0].0 as usize].clone()
                    {
                        if kind == crate::expr::LAMBDA {
                            let reduced = self.open(body, 0, normalized[1]);
                            if normalized.len() == 2 {
                                return self.beta_normalize(reduced, fuel);
                            } else {
                                let mut new_args = vec![reduced];
                                new_args.extend_from_slice(&normalized[2..]);
                                let app = self.app(new_args);
                                return self.beta_normalize(app, fuel);
                            }
                        }
                    }
                }
                if normalized == args {
                    expr
                } else {
                    self.app(normalized)
                }
            }
            HNode::Binder { kind, hint, ty, body } => {
                *fuel = fuel.saturating_sub(1);
                let new_ty = self.beta_normalize(ty, fuel);
                let new_body = self.beta_normalize(body, fuel);
                if new_ty == ty && new_body == body {
                    expr
                } else {
                    self.binder(kind, &hint, new_ty, new_body)
                }
            }
            HNode::UserBind { spec, params, body } => {
                let new_params: Vec<HExpr> = params
                    .iter()
                    .map(|&p| self.beta_normalize(p, fuel))
                    .collect();
                let new_body = self.beta_normalize(body, fuel);
                if new_params == params && new_body == body {
                    expr
                } else {
                    self.user_bind(&spec, new_params, new_body)
                }
            }
        }
    }

    /// Collect free variable names from an HExpr.
    pub fn free_vars(&self, h: HExpr) -> Vec<Name> {
        let mut result = Vec::new();
        self.free_vars_inner(h, &mut result);
        result
    }

    fn free_vars_inner(&self, h: HExpr, acc: &mut Vec<Name>) {
        match self.node(h) {
            HNode::Free(n) => {
                if !acc.contains(n) {
                    acc.push(n.clone());
                }
            }
            HNode::Bound(_) | HNode::Meta(_) | HNode::Sym(_) => {}
            HNode::App(args) => {
                let args = args.clone();
                for a in &args {
                    self.free_vars_inner(*a, acc);
                }
            }
            HNode::Binder { ty, body, .. } => {
                let (ty, body) = (*ty, *body);
                self.free_vars_inner(ty, acc);
                self.free_vars_inner(body, acc);
            }
            HNode::UserBind { params, body, .. } => {
                let params = params.clone();
                let body = *body;
                for p in &params {
                    self.free_vars_inner(*p, acc);
                }
                self.free_vars_inner(body, acc);
            }
        }
    }

    /// Collect abstractable variable HExprs (both Free and Meta) from an HExpr.
    /// Used by Miller matching where Meta variables in the target can serve
    /// as abstraction targets.
    pub fn abstractable_vars(&self, h: HExpr) -> Vec<HExpr> {
        let mut result = Vec::new();
        self.abstractable_vars_inner(h, &mut result);
        result
    }

    fn abstractable_vars_inner(&self, h: HExpr, acc: &mut Vec<HExpr>) {
        match self.node(h) {
            HNode::Free(_) | HNode::Meta(_) => {
                if !acc.contains(&h) {
                    acc.push(h);
                }
            }
            HNode::Bound(_) | HNode::Sym(_) => {}
            HNode::App(args) => {
                let args = args.clone();
                for a in &args {
                    self.abstractable_vars_inner(*a, acc);
                }
            }
            HNode::Binder { ty, body, .. } => {
                let (ty, body) = (*ty, *body);
                self.abstractable_vars_inner(ty, acc);
                self.abstractable_vars_inner(body, acc);
            }
            HNode::UserBind { params, body, .. } => {
                let params = params.clone();
                let body = *body;
                for p in &params {
                    self.abstractable_vars_inner(*p, acc);
                }
                self.abstractable_vars_inner(body, acc);
            }
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
                format!(
                    "({} ({} : {}) {})",
                    kind,
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

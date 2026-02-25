# Hyperion

**A Logical Framework Framework Framework.**

Hyperion is a meta-system for building logical frameworks. You describe the *mathematics* you want (a category) and the *computational physics* you want it to run on (a substrate), and Hyperion compiles the two together into a working logical system, verifying that your math is actually implementable on your chosen physics.

The core insight: not every mathematical structure can run on every computational substrate. Lambda calculus needs an engine that supports closures. Modal logic needs scope isolation. Tensor products need parallel composition. Hyperion enforces these constraints at compile time, then generates the correct backend configuration automatically.

But Hyperion's real power is *self-application*. Categories and substrates are just data. Nothing stops you from defining a category whose objects are themselves categories, whose morphisms are functors, and whose paths are natural isomorphisms --- and then running it on a substrate. The framework frameworks itself, all the way up.

## The Three Layers

```
 Category          Substrate            Universe
 (the math)    +   (the physics)    =   (the system)
 ──────────        ──────────           ──────────
 Objects            Engine               Apeiron System
 Morphisms          ResourceMode         with correct binding,
 Structure          BarrierMode          checking, and
 (CCC, Monoidal,    EqualityMode         scoping config
  Modal, HoTT,
  Preorder, ...)
```

A **Category** declares pure mathematical structure: what your sorts are (Objects), what operations exist (Morphisms), and what higher structure is present (Exponentials for lambda, TensorProducts for parallel composition, ModalOperators for necessity, PathType for HoTT, Preorder for reflexive relations).

A **Substrate** declares computational physics: what engine executes your terms (interaction graphs, term trees, cellular automata, symmetric monoidal nets, von Neumann machines), how resources are managed (optimal sharing, linear, affine, deep copy), how scoping works (transparent, contextual membranes, cryptographic), and what notion of equality the system uses (rewriting, hashing, unification, homotopy, equality saturation).

A **Universe** binds a Category to a Substrate. Hyperion checks compatibility, then compiles both into an [Apeiron](../apeiron/) system configuration with the correct binding mode, checking strategy, and scope declarations.

## Quick Start

```bash
cargo build
```

Check a file:

```bash
hyperion check examples/weak-lf.hyp -v
```

Compile a theory to Rust:

```bash
hyperion kompile examples/peano-vn.hyp --theory PeanoRules -o /tmp/peano/
```

## A Complete Example

Here is a full Hyperion file that defines a lambda calculus, runs it on an interaction net, and proves that beta reduction works:

```
;; 1. The math: a Cartesian Closed Category
[Category CartesianClosed
  [Object Type]
  [Object Term]
  [Morphism arrow :domain [Type Type] :codomain Type]
  [Morphism app   :domain [Term Term] :codomain Term]
  [Exponential lam :object Term]
  [Evaluator app]
]

;; 2. The physics: an interaction graph with optimal sharing
[Substrate InteractionNet
  @engine interaction-graph
  @resource-mode optimal-sharing
  @barrier transparent
  @equality topological-hash
]

;; 3. Compile math into physics
[Universe WeakLF :category CartesianClosed :substrate InteractionNet]

;; 4. Write a theory in the compiled universe
[Theory SimpleLogic :in WeakLF
  [const z Term]
  [const s Term]
]

;; 5. Prove things
[Proofs Check :in SimpleLogic
  [assert-eq beta    [app [lam x x] z]                z]
  [assert-eq nested  [app [lam x x] [app [lam y y] z]] z]
  [eval      church  [app [app [lam f [lam x [app f x]]] s] z]]
]
```

Hyperion automatically verifies that the CCC structure is compatible with the interaction graph engine (it is --- interaction graphs support lambda abstraction), generates the Apeiron system `__hyp_CartesianClosed_InteractionNet`, and passes the theory and proofs through for execution.

## Categorical Structures

Hyperion supports several categorical structures, each with specific auto-injection behavior and law verification:

### Cartesian Closed (CCC)

Lambda abstraction and beta reduction. Requires a lambda-capable engine.

```
[Exponential lam :object Term]   ;; Lambda abstraction
[Evaluator app]                  ;; Beta reduction via application
```

Auto-verified laws (3 witness tests): beta-identity, beta-constant, beta-nested.

### Symmetric Monoidal

Parallel composition with a unit object. Two equivalent syntaxes:

```
;; Compound syntax (recommended)
[SymmetricMonoidal tensor unit]

;; Explicit syntax (equivalent)
[TensorProduct tensor]
[Unit unit]
```

The compound syntax `[SymmetricMonoidal tensor unit]` desugars into `[TensorProduct tensor]` + `[Unit unit]`. Use whichever reads better.

**Important:** Unlike CCC where beta reduction is built into the engine, monoidal laws must be provided as user-written rewrite rules (either `@rule` or `@law`):

```
[Theory MonoidalTheory :in SomeUniverse
  [@rule mon-assoc  [tensor [tensor ?a ?b] ?c] ==> [tensor ?a [tensor ?b ?c]]]
  [@rule mon-lunit  [tensor unit ?a] ==> ?a]
  [@rule mon-runit  [tensor ?a unit] ==> ?a]
]
```

Auto-verified laws (6 witness tests): associativity, left-unit, right-unit, nested-right-unit, mixed-unit, double-unit.

### PathType (Homotopy Type Theory)

Built-in path algebra for HoTT. See the [dedicated section below](#homotopy-type-theory-via-pathtype).

### Preorder

A reflexive relation with an auto-injected reflexivity rule:

```
[Category PreorderCat
  [Object Prop]
  [Morphism leq :domain [Prop Prop] :codomain Prop]
  [Preorder leq]
]
```

When a theory is loaded in a Preorder universe, Hyperion auto-injects:

```
[@rule [leq ?a ?a] ==> true]
```

The `true` op is also auto-injected into the generated Apeiron system. No manual rules needed for reflexivity --- it just works:

```
[Theory PreorderTheory :in PreorderU
  [const p Prop]
  [const q Prop]
]

[Proofs PreorderChecks :in PreorderTheory
  [assert-eq reflexive        [leq p p]                   true]
  [assert-eq reflexive-nested [leq [leq p q] [leq p q]]  true]
]
```

Auto-verified laws (2 witness tests): reflexivity on atoms and structured terms.

### Modal Operators

Necessity/possibility modalities with scope isolation:

```
[ModalOperator box]
[Context WorldA]
[Context WorldB]
```

Requires a substrate with barrier support (`contextual-membranes` or `cryptographic`). A barriered term is *stuck* until its scope is activated --- modeling necessity, possible worlds, and staged computation.

## Categorical Law Verification

When a theory is loaded into a universe, Hyperion auto-generates and verifies the foundational laws of that universe's category. If your rewrite rules don't satisfy the category's axioms, compilation fails.

Laws are tested on both flat irreducible witness atoms (`__law_a`, `__law_b`, etc.) and structured witnesses that exercise rule interactions. For left-linear first-order rewrite rules, irreducible witnesses are universal: if `tensor(__law_a, unit)` reduces to `__law_a`, then `tensor(X, unit)` reduces to `X` for any `X`, because the rewrite doesn't inspect the matched variable. Structured witnesses additionally test interactions between rules (e.g., nested applications of associativity + unit laws).

The output honestly reports the number of witness tests performed:

```
[LAWS] Arithmetic passed categorical law verification (6 witness tests)
```

| Category Structure | Witness Tests | What's Verified |
|---|---|---|
| SymmetricMonoidal | 6 | Associativity, left/right unit, nested interactions |
| Cartesian Closed | 3 | Beta reduction (identity, constant, nested) |
| PathType (with Evaluator) | 5 | Left/right unit, inverse, associativity, ap-refl |
| PathType (without Evaluator) | 4 | Left/right unit, inverse, associativity |
| Preorder | 2 | Reflexivity on atoms and structured terms |

If normalization runs out of fuel, the result is reported as **INCONCLUSIVE** rather than a failure --- an honest acknowledgment that finite testing cannot prove universal properties.

Use `--skip-laws` to disable verification during development.

## Homotopy Type Theory via PathType

The **PathType** categorical structure provides built-in path algebra for HoTT. Instead of manually writing rewrite rules for path operations, declare `[PathType]` in your category and Hyperion auto-injects the path algebra operations and rewrite rules:

```
[Category PathSpace
  [Object Type]
  [Object Term]
  [Morphism app :domain [Term Term] :codomain Term]
  [Exponential lam :object Term]
  [Evaluator app]
  [PathType :refl refl :concat concat :inv inv :ap ap]
]

[Substrate HomotopyEngine
  @engine interaction-graph
  @resource-mode optimal-sharing
  @barrier transparent
  @equality topological-homotopy
]

[Universe HoTTWorld :category PathSpace :substrate HomotopyEngine]

;; Empty theory body --- all path rules auto-injected by PathType!
[Theory PathAlgebra :in HoTTWorld]

;; Proofs work immediately --- no manual rule writing needed
[Proofs PathCheck :in PathAlgebra
  [assert-eq left-unit  [concat [refl a] p]     p]
  [assert-eq right-unit [concat p [refl a]]     p]
  [assert-eq assoc      [concat [concat p q] r] [concat p [concat q r]]]
  [assert-eq ap-refl    [ap f [refl a]]         [refl [app f a]]]
  [assert-eq inv-refl   [inv [refl a]]          [refl a]]
]
```

### Auto-Injected Rules

PathType auto-injects up to 5 rewrite rules into every theory in a PathType universe:

1. `concat(refl(a), p) ==> p` --- left identity
2. `concat(p, refl(a)) ==> p` --- right identity
3. `inv(refl(a)) ==> refl(a)` --- inverse of reflexivity
4. `concat(concat(p,q), r) ==> concat(p, concat(q,r))` --- associativity
5. `ap(f, refl(a)) ==> refl(app(f, a))` --- functorial action (**only if Evaluator is present**)

These model the groupoid structure of identity types: paths can be composed, inverted, and paths between paths form higher-dimensional structure.

### PathType Without Evaluator

PathType does **not** always require a lambda-capable engine. When the category has no `[Evaluator]`, rule 5 (ap-refl) is not injected and the path algebra is purely first-order. This means PathType works on engines like `symmetric-monoidal` or even `term-tree`:

```
;; First-order path algebra on a symmetric monoidal engine
[Category MonoidalHoTT
  [Object Type]
  [Object Term]
  [Morphism tensor :domain [Term Term] :codomain Term]
  [SymmetricMonoidal tensor unit]
  [PathType :refl refl :concat concat :inv inv :ap ap]
  ;; No Evaluator --- 4 path rules auto-injected, not 5
]

[Substrate SymmetricEngine
  @engine symmetric-monoidal       ;; Not lambda-capable, but that's fine
  @resource-mode deep-copy
  @barrier transparent
  @equality rewrite-equivalence
]
```

Law verification generates 4 witness tests (no ap-refl) instead of 5.

### Combining PathType with Other Structures

PathType composes freely with other categorical structures. Users can add rules that make their structures interact with paths:

```
;; Tensor is functorial on paths
[@rule tensor-refl [tensor [refl ?x] [refl ?y]] ==> [refl [tensor ?x ?y]]]

;; Modal box preserves paths
[@rule box-path [box [refl ?x]] ==> [refl [box ?x]]]
```

See `modal-hott.hyp` and `monoidal-hott.hyp` for full examples of PathType composed with CCC+Modal and SymmetricMonoidal respectively.

## Equational Laws (`@law`) and E-Graph Simplification

Hyperion theories support two kinds of declarations for equational reasoning:

- **`@rule`** --- directed computation rules (forward-only in e-graph, compiled to inet rules)
- **`@law`** --- equational laws (bidirectional in e-graph, NOT compiled to inet rules)

Use `@rule` for computation that simplifies in one direction (e.g., `add(z, n) ==> n`). Use `@law` for algebraic identities that hold in both directions (e.g., commutativity: `add(x, y) === add(y, x)`).

```
[Theory CommMonoid :in AlgebraWorld
  ;; Directed computation
  [@rule add-z [add z ?n] ==> ?n]
  [@rule add-s [add [s ?n] ?m] ==> [s [add ?n ?m]]]

  ;; Equational law (bidirectional)
  [@law add-comm [add ?x ?y] === [add ?y ?x]]
]
```

Laws require a substrate with `@equality equality-saturation` to activate the e-graph fallback in `assert-eq`. The e-graph proves equivalences that directed normalization cannot:

```
[Substrate EGraphEngine
  @engine interaction-graph
  @resource-mode optimal-sharing
  @barrier transparent
  @equality equality-saturation    ;; Enables @law + eval-simplify
]
```

### E-Graph Extraction (`eval-simplify`)

The `eval-simplify` command finds the simplest equivalent expression via `egg::Extractor`:

```
[Proofs Check :in CommMonoid
  ;; Directed computation works normally
  [assert-eq one-plus-one [add [s z] [s z]] [s [s z]]]

  ;; E-graph proves commutativity: 1+2 = 2+1
  [assert-eq comm [add [s z] [s [s z]]] [add [s [s z]] [s z]]]

  ;; E-graph extraction: find smallest equivalent form
  [eval-simplify simplify [add z [s [s z]]]]   ;; → s(s(z))
]
```

### Law Propagation Through Imports

`@law` declarations propagate through `[import]`, preserving their bidirectional semantics:

```
[Theory Ring :in AlgebraWorld
  [import CommMonoid]              ;; add-comm law is inherited
  [@rule mul-z [mul z ?n] ==> z]
  [@law mul-comm [mul ?x ?y] === [mul ?y ?x]]
]
```

### Laws in Cross-Substrate Verification

`VerifyFunctor` correctly verifies `@law` declarations across substrates. Laws are checked via the e-graph fallback (shown as `passed (e-graph)` in output), while directed rules are checked via inet normalization:

```
[ASSERT] verify-rule-9 passed            ;; @rule verified via inet
[ASSERT] verify-rule-10 passed (e-graph) ;; @law verified via e-graph
```

See `equational-algebra.hyp` and `egraph-transport.hyp` for full examples.

## Cross-Substrate Functors

The same mathematical category can be compiled into different substrates. A **Functor** transports terms between them:

```
[Universe PeanoCompute :category SimpleMath :substrate ComputeNet]
[Universe PeanoOracle  :category SimpleMath :substrate OracleTree]

[Functor NetToTree :from ComputeNet :to OracleTree]

[Theory Source :in PeanoCompute
  [@rule [plus z ?n] ==> ?n]
  [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
  [def two-plus-one [plus [s [s z]] [s z]]]
]

;; Transport: normalize on compute substrate, then send to oracle
[Theory Target :in PeanoOracle
  [Import result [NetToTree two-plus-one]]
]

[Proofs Check :in Target
  [assert-eq ok result [s [s [s z]]]]  ;; 3
]
```

The functor normalizes `plus(s(s(z)), s(z))` to `s(s(s(z)))` on the compute substrate, then transports the normal form to the oracle substrate where it can be checked structurally.

### Verifying Equational Theory Preservation

A functor is only mathematically valid if it preserves the equational theory. Use `[VerifyFunctor]` to check that every rewrite rule from the source theory, when transformed by the functor's operator mapping, holds in the target theory:

```
[Functor NetToTree :from ComputeNet :to OracleTree :verify]

[Theory Source :in PeanoCompute
  [@rule [plus z ?n] ==> ?n]
  [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
]

[Theory Target :in PeanoOracle
  [@rule [plus z ?n] ==> ?n]
  [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
]

[VerifyFunctor NetToTree :source Source :target Target]
;; Output: [VERIFY-FUNCTOR] NetToTree preserves equational theory (Source -> Target, 2 rules verified)
```

With operator mappings, `VerifyFunctor` correctly transforms rule atoms before checking:

```
[Functor F :from A :to B :map-morphism [z zero] :map-morphism [s succ] :map-morphism [plus add]]
;; Verifies: [add zero ?n] ==> ?n and [add [succ ?n] ?m] ==> [succ [add ?n ?m]]
```

Both named and unnamed rule formats are handled:

```
[@rule [plus z ?n] ==> ?n]              ;; unnamed: captured correctly
[@rule plus-z [plus z ?n] ==> ?n]       ;; named: also captured correctly
```

### Natural Transformations and Adjunctions

**Natural Transformations** relate parallel functors (both must go from the same source to the same target substrate):

```
[Functor F :from ComputeNet :to OracleNet]
[Functor G :from ComputeNet :to OracleNet]    ;; Parallel to F

[NatTrans eta :from F :to G
  :component [Type id_type]
  :component [Term id_term]
]
```

**Adjunctions** formalize free/forgetful relationships between parallel functors:

```
[NatTrans eta :from F :to G :component [Nat unit_nat]]
[NatTrans eps :from F :to G :component [Nat counit_nat]]

[Adjunction FreeForget :left F :right G :unit eta :counit eps]
```

## Meta-Categories: The Infinite Ascent

Nothing in Hyperion restricts what your objects and morphisms *mean*. You can define a category whose objects are categories and whose morphisms are functors --- then reason about the category of categories:

```
;; Objects are categories, morphisms are functors
[Category MetaCat
  [Object Cat]
  [Morphism functor :domain [Cat Cat] :codomain Cat]
  [PathType :refl refl_cat :concat concat_cat :inv inv_cat :ap ap_cat]
]

[Substrate MetaSub
  @engine interaction-graph
  @resource-mode optimal-sharing
  @barrier transparent
  @equality topological-homotopy
]

[Universe MetaUniverse :category MetaCat :substrate MetaSub]

[Theory MetaTheory :in MetaUniverse
  [const PreCat Cat]
  [const id_func Cat]

  ;; Functor composition is associative
  [@rule func-comp  [functor [functor ?F ?G] ?H] ==> [functor ?F [functor ?G ?H]]]
  [@rule func-id-l  [functor id_func ?G] ==> ?G]
  [@rule func-id-r  [functor ?F id_func] ==> ?F]
  [@rule func-refl  [functor ?F [refl_cat ?G]] ==> [refl_cat [functor ?F ?G]]]
]

[Proofs MetaChecks :in MetaTheory
  ;; Identity functor law
  [assert-eq meta-id [functor id_func PreCat] PreCat]

  ;; Functors respect paths: functor(id, refl(X)) = refl(X)
  [assert-eq meta-refl [functor id_func [refl_cat PreCat]] [refl_cat PreCat]]

  ;; Path composition at meta-level (auto-injected by PathType)
  [assert-eq meta-path [concat_cat [refl_cat PreCat] [refl_cat PreCat]] [refl_cat PreCat]]
]
```

PathType auto-injects path rules at the meta-level too. Paths between functors are natural isomorphisms. Paths between paths between functors are modifications. The ascent doesn't stop --- you can define categories of meta-categories, and so on.

See `wild-linear-meta.hyp` for a full example combining meta-categories with linear resource logic, preorders, adjunctions, and cross-substrate functors at multiple levels.

## Resource-Aware Logic

Hyperion models resource sensitivity through the interaction between substrate resource modes and categorical structure:

```
;; Linear resources: no duplication, no discarding
[Substrate LinearGraph
  @engine interaction-graph
  @resource-mode strictly-linear
  @barrier contextual-membranes
  @equality topological-homotopy
]

;; Affine resources: can discard but not duplicate
[Substrate AffineGraph
  @engine interaction-graph
  @resource-mode affine
  @barrier contextual-membranes
  @equality topological-homotopy
]
```

A functor from linear to affine substrate models the *promotion* of linear resources to affine ones. Combined with natural transformations and adjunctions, this captures the standard promotion/dereliction adjunction from linear logic:

```
[Functor LinearToAffine  :from LinearGraph :to AffineGraph :verify]
[Functor LinearToAffine2 :from LinearGraph :to AffineGraph]

[NatTrans Promotion   :from LinearToAffine :to LinearToAffine2
  :component [Type prom_type] :component [Term prom_term]]

[NatTrans Dereliction :from LinearToAffine :to LinearToAffine2
  :component [Type der_type]  :component [Term der_term]]

[Adjunction ResourceAdj :left LinearToAffine :right LinearToAffine2
  :unit Promotion :counit Dereliction]
```

The affine theory can have extra rules that linear rejects (e.g., `box(box(x)) ==> box(x)` for idempotent storage), while VerifyFunctor ensures the linear rules are preserved.

## Von Neumann Backend (Optional)

When the substrate uses `@engine von-neumann`, Hyperion bypasses the Apeiron rewriting engine entirely and compiles theories directly to Rust:

```
[Substrate VonNeumannMachine
  @engine von-neumann
  @resource-mode deep-copy
  @barrier transparent
  @equality rewrite-equivalence
]

[Theory PeanoRules :in PeanoVN
  [@rule plus-z [plus z ?n] ==> ?n]
  [@rule plus-s [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
]
```

```bash
$ hyperion kompile peano-vn.hyp --theory PeanoRules -o /tmp/peano/
```

Generates a complete Rust crate:

```rust
// types.rs
#[derive(Debug, Clone, PartialEq)]
pub enum Nat {
    Z,
    S(Box<Nat>),
}

// functions.rs
pub fn plus(nat: Nat, nat_2: Nat) -> Nat {
    match (nat, nat_2) {
        (Nat::Z, n) => n,
        (Nat::S(box n), m) => Nat::S(Box::new(plus(n, m))),
        _ => unreachable!(),
    }
}
```

Von Neumann substrates reject higher-order features (Exponential, ModalOperator, TensorProduct) since sequential machines can't natively support closures, scope isolation, or parallel composition. This backend is an optional "applied" feature --- the primary Hyperion experience is the abstract categorical framework.

## Compatibility Rules

Hyperion enforces that your mathematical structure is realizable on your chosen physics:

| Category Feature | Required Substrate Properties |
|---|---|
| Exponential / Evaluator | Engine must support lambda (interaction-graph, term-tree, abstract-machine) |
| ModalOperator / Context | Barrier must support scoping (contextual-membranes, cryptographic) |
| TensorProduct | Engine must support parallel composition (interaction-graph, symmetric-monoidal) |
| PathType + Evaluator | Engine must support lambda (ap-refl rule needs application) |
| PathType (no Evaluator) | No engine restriction (purely first-order path algebra) |
| Preorder | No engine restriction (rewriting-based reflexivity) |
| topological-homotopy | Engine must support lambda (path spaces need higher-order structure) |

Always-rejected combinations:

| Combination | Reason |
|---|---|
| StrictlyLinear + Exponential | Linear resources can't duplicate closures |
| VonNeumann + Exponential | No lambda at hardware level |
| VonNeumann + ModalOperator | No scope isolation in sequential model |
| VonNeumann + TensorProduct | No parallel composition in sequential model |

## Common Pitfalls

### Functor `:from` / `:to` take substrate names, not universe names

```
;; WRONG:
[Functor F :from PeanoCompute :to PeanoOracle]       ;; These are universe names!

;; RIGHT:
[Functor F :from ComputeNet :to OracleTree]           ;; These are substrate names
```

### NatTrans requires parallel functors

Both functors in a natural transformation must go from the same source substrate to the same target substrate:

```
;; WRONG: F goes A→B, G goes B→A (anti-parallel)
[Functor F :from A :to B]
[Functor G :from B :to A]
[NatTrans eta :from F :to G ...]        ;; Error!

;; RIGHT: Both go A→B (parallel)
[Functor F :from A :to B]
[Functor G :from A :to B]
[NatTrans eta :from F :to G ...]        ;; OK
```

### VerifyFunctor source and target must be distinct theories

The source and target theories must be different theory names, each in its own universe. They can (and usually do) have the same rules:

```
;; WRONG: same theory for both
[VerifyFunctor F :source MyTheory :target MyTheory]

;; RIGHT: two theories with the same rules in different universes
[Theory SourceTheory :in Universe1 ...]
[Theory TargetTheory :in Universe2 ...]   ;; Same rules
[VerifyFunctor F :source SourceTheory :target TargetTheory]
```

### Monoidal rules must be user-provided

Unlike CCC beta reduction (built into the engine) and PathType rules (auto-injected), monoidal laws require explicit `@rule` (or `@law`) declarations. Without them, law verification will fail:

```
[Theory Monoidal :in MonoidalWorld
  ;; These three rules are REQUIRED for monoidal law verification:
  [@rule [tensor [tensor ?a ?b] ?c] ==> [tensor ?a [tensor ?b ?c]]]
  [@rule [tensor unit ?a] ==> ?a]
  [@rule [tensor ?a unit] ==> ?a]
]
```

### Design rules for termination

Rewrite rules must terminate. Common traps:

```
;; WRONG: non-terminating (doubles the argument each step)
[@rule box-expand [app [box ?a] ?f] ==> [app [box ?a] [app [box ?a] ?f]]]

;; WRONG: conflicts with monoidal right-unit law
[@rule bad-weak [tensor ?a unit] ==> unit]   ;; Competes with [tensor ?a unit] ==> ?a

;; RIGHT: idempotent (always reduces)
[@rule box-idem [box [box ?x]] ==> [box ?x]]
```

### Resource mode must support rule patterns

If a rewrite rule uses a meta-variable more than once on the RHS (non-linear), the substrate's resource mode must permit duplication:

```
;; This rule uses ?r twice on the RHS:
[@rule tensor-concat [tensor [concat ?p ?q] ?r] ==> [concat [tensor ?p ?r] [tensor ?q ?r]]]

;; Requires: @resource-mode deep-copy (or optimal-sharing)
;; Will not work with: @resource-mode strictly-linear
```

### NatTrans components must reference category objects

Natural transformation components name *objects from the category*, not arbitrary sort names:

```
[Category MyCat
  [Object Type]
  [Object Term]
  ...
]

;; RIGHT: Type and Term are objects of MyCat
[NatTrans eta :from F :to G
  :component [Type id_type]
  :component [Term id_term]]

;; WRONG: Nat is not an object of MyCat
[NatTrans eta :from F :to G
  :component [Nat tau_nat]]
```

## Prelude

Hyperion ships with a standard prelude (`prelude.hyp`) that provides common categories and substrates:

**Categories:**
- `CartesianClosed` --- Objects: Type, Term. CCC with `lam`/`app`.
- `SymmetricMonoidal` --- Objects: Obj. Tensor product with unit.
- `Preorder` --- Objects: Elem. Reflexive relation `leq`.

**Substrates:**
- `ApeironStandard` --- interaction-graph, optimal-sharing, transparent, rewrite-equivalence
- `ApeironLinear` --- interaction-graph, strictly-linear, transparent, rewrite-equivalence
- `ApeironOracle` --- interaction-graph, optimal-sharing, transparent, topological-hash
- `ApeironTree` --- term-tree, deep-copy, transparent, rewrite-equivalence

The prelude is auto-loaded unless `--no-prelude` is passed.

## Auto-Injection Summary

Several categorical structures auto-inject rules or ops into theories. This table summarizes what you get for free:

| Structure | Auto-Injected Ops | Auto-Injected Rules | User Must Provide |
|---|---|---|---|
| Exponential + Evaluator | `lam` | (beta built into engine) | Nothing |
| SymmetricMonoidal | `tensor`, `unit` | None | Associativity + unit `@rule`s |
| PathType (with Evaluator) | `refl`, `concat`, `inv`, `ap` | 5 path algebra `@rule`s | Nothing |
| PathType (no Evaluator) | `refl`, `concat`, `inv`, `ap` | 4 path algebra `@rule`s (no ap-refl) | Nothing |
| Preorder | `true` | Reflexivity `@rule` | Nothing |
| ModalOperator | `box` | None | Modal distribution rules |

User theories can also declare `@law` equational laws (bidirectional in e-graph) alongside `@rule` declarations. See [Equational Laws](#equational-laws-law-and-e-graph-simplification).

## Examples

| File | What it demonstrates |
|---|---|
| `weak-lf.hyp` | Lambda calculus on an interaction net --- beta reduction, Church numerals |
| `simple-ccc.hyp` | CCC with Church encoding, cross-substrate functor transport |
| `modal-logic.hyp` | Modal necessity with barrier scopes --- locked/unlocked evaluation |
| `cross-substrate.hyp` | Functor transporting terms across substrates + verified equational preservation |
| `adjunction-demo.hyp` | Natural transformations and adjunctions between parallel functors |
| `law-check-demo.hyp` | Automatic categorical law verification (monoidal + CCC) with witness counts |
| `hott-demo.hyp` | PathType auto-injected path algebra --- empty theory, full proofs |
| `modal-hott.hyp` | CCC + PathType + ModalOperator --- modal functoriality over paths |
| `monoidal-hott.hyp` | SymmetricMonoidal + PathType --- first-order path algebra without lambda |
| `wild-linear-meta.hyp` | Linear/affine logic + meta-categories + preorder + adjunctions |
| `meta-coherence.hyp` | The framework³ stress test: meta-category across two equality physics with verified transport |
| `equational-algebra.hyp` | `@law` vs `@rule` + `eval-simplify` + theory composition with law propagation |
| `egraph-transport.hyp` | `@law` preservation through cross-substrate functors + PathType + e-graph |
| `peano-vn.hyp` | Peano arithmetic compiled to Rust via Von Neumann backend |
| `prelude-demo.hyp` | Using prelude categories and substrates |

## Architecture

```
hyperion/
  src/
    category.rs      Category definitions and parsing (CCC, Monoidal, PathType, Preorder, Modal)
    substrate.rs     Substrate definitions (Engine, ResourceMode, BarrierMode, EqualityMode)
    universe.rs      Universe binding and naming
    compile.rs       Compatibility checking + Apeiron system generation
    functor.rs       Cross-substrate functor definitions (incl. :verify flag)
    nat_trans.rs     Natural transformation definitions
    adjunction.rs    Adjunction definitions
    laws.rs          Categorical law auto-generation (flat + structural witnesses)
    session.rs       Main session orchestration (VerifyFunctor, PathType/Preorder injection)
    codegen/
      mod.rs         Von Neumann kompile entry point
      rust_ast.rs    Lightweight Rust AST types
      analyze.rs     VN theory -> Rust AST (sort analysis, boxing, pattern matching)
      emit.rs        Rust AST -> source files
    main.rs          CLI (check + kompile subcommands)
  prelude.hyp        Standard prelude (categories + substrates)
  examples/          15 example files
  tests/
    integration.rs   80 integration tests
```

Hyperion depends on [Apeiron](../apeiron/) for term rewriting, beta reduction, and oracle checking. The Von Neumann backend is the only path that bypasses Apeiron entirely.

## CLI Reference

```
hyperion check <file.hyp> [-v] [--no-prelude] [--skip-laws]
hyperion kompile <file.hyp> --theory <name> -o <output_dir/>
```

| Flag | Effect |
|---|---|
| `-v` | Print stats (categories, substrates, universes, Apeiron node counts) |
| `--no-prelude` | Don't load the standard prelude |
| `--skip-laws` | Don't verify categorical laws after theory loading |
| `--theory` | Theory name for kompile |
| `-o` | Output directory for generated Rust crate |

## Tests

125 tests (27 unit + 98 integration), covering:

- Category/substrate/universe parsing and validation
- All compatibility rejection rules (PathType with/without Evaluator, modal, tensor, VN)
- Functor transport with normalization
- Functor equational theory verification (VerifyFunctor, both named and unnamed rules)
- Natural transformation and adjunction validation
- Von Neumann theory capture and Rust code generation
- Categorical law pass/fail/inconclusive cases with witness counts
- PathType auto-injection (with and without Evaluator)
- Preorder auto-injection and law verification
- SymmetricMonoidal compound syntax
- Meta-coherence: self-application + cross-substrate transport + falsification
- `@law` vs `@rule` distinction, `eval-simplify`, and law propagation through imports
- All 15 example files

```bash
cargo test
```

## Design Philosophy

Hyperion takes three positions:

**Math and physics are orthogonal.** The same lambda calculus can run on interaction graphs, term trees, or von Neumann machines. The same monoidal structure can live on linear or affine substrates. Hyperion enforces the necessary constraints but otherwise stays out of the way.

**Honesty over ceremony.** Law verification tests on witness atoms and reports the count. It doesn't claim universal truth. VerifyFunctor checks rule preservation concretely. Fuel exhaustion is inconclusive, not failure. The system tells you exactly what it checked and what it couldn't.

**Self-application is the test.** If Hyperion can't framework itself --- if you can't define a category of categories and reason about functors between functors --- then the abstraction leaks. The examples prove it doesn't: `wild-linear-meta.hyp` has meta-categories with PathType at the meta-level, and it all just works.

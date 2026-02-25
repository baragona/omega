# Hyperion

**A Logical Framework Framework Framework.**

Hyperion is a meta-system for building logical frameworks. You describe the *mathematics* you want (a category) and the *computational physics* you want it to run on (a substrate), and Hyperion compiles the two together into a working logical system, verifying that your math is actually implementable on your chosen physics.

The core insight: not every mathematical structure can run on every computational substrate. Lambda calculus needs an engine that supports closures. Modal logic needs scope isolation. Tensor products need parallel composition. Hyperion enforces these constraints at compile time, then generates the correct backend configuration automatically.

## The Three Layers

```
 Category          Substrate            Universe
 (the math)    +   (the physics)    =   (the system)
 ──────────        ──────────           ──────────
 Objects            Engine               Apeiron System
 Morphisms          ResourceMode         with correct binding,
 Structure          BarrierMode          checking, and
 (CCC, Monoidal,    EqualityMode         scoping config
  Modal, HoTT, ...)
```

A **Category** declares pure mathematical structure: what your sorts are (Objects), what operations exist (Morphisms), and what higher structure is present (Exponentials for lambda, TensorProducts for parallel composition, ModalOperators for necessity, PathType for HoTT).

A **Substrate** declares computational physics: what engine executes your terms (interaction graphs, term trees, cellular automata, von Neumann machines), how resources are managed (optimal sharing, linear, affine, deep copy), how scoping works (transparent, contextual membranes, cryptographic), and what notion of equality the system uses (rewriting, hashing, unification, homotopy).

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

## Categorical Law Verification

When a theory is loaded into a universe, Hyperion auto-generates and verifies the foundational laws of that universe's category. If your rewrite rules don't satisfy the category's axioms, compilation fails.

Laws are tested on both flat irreducible witness atoms and structured witnesses that exercise rule interactions. The output honestly reports the number of witness tests performed:

For a **Symmetric Monoidal** category (TensorProduct + Unit) --- 6 witness tests:
- Associativity: `tensor(tensor(a,b),c) = tensor(a,tensor(b,c))`
- Left unit: `tensor(unit,a) = a`
- Right unit: `tensor(a,unit) = a`
- Nested right unit: `tensor(tensor(a,b),unit) = tensor(a,b)`
- Mixed unit: `tensor(unit,tensor(a,unit)) = a`
- Double unit: `tensor(unit,unit) = unit`

For a **Cartesian Closed** category (Exponential + Evaluator) --- 3 witness tests:
- Beta reduction (identity): `app(lam(x,x), a) = a`
- Beta reduction (constant): `app(lam(x,a), b) = a`
- Beta reduction (nested): `app(lam(x, app(lam(y,y), x)), a) = a`

For **PathType** categories --- 4-5 witness tests (see HoTT section below).

```
[Theory Arithmetic :in MonoidalWorld
  ;; These rules must satisfy monoidal laws, or compilation fails
  [@rule [pair [pair ?a ?b] ?c] ==> [pair ?a [pair ?b ?c]]]
  [@rule [pair unit ?a] ==> ?a]
  [@rule [pair ?a unit] ==> ?a]
]
;; Output: [LAWS] Arithmetic passed categorical law verification (6 witness tests)
```

If normalization runs out of fuel, the result is reported as **INCONCLUSIVE** rather than a failure --- an honest acknowledgment that finite testing cannot prove universal properties.

Use `--skip-laws` to disable verification during development.

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

**Natural Transformations** relate parallel functors, and **Adjunctions** formalize free/forgetful relationships:

```
[NatTrans eta :from F :to G :component [Nat tau_nat]]
[Adjunction FreeForget :left F :right G :unit eta :counit eps]
```

## Modal Logic and Scope Isolation

Categories with **ModalOperators** and **Contexts** require substrates with barrier support. Hyperion enforces this:

```
[Category ModalSpace
  [Object Term]
  [ModalOperator box]
  [Context WorldA]
  [Context WorldB]
]

;; Requires contextual membranes for scope isolation
[Substrate CompartmentNet
  @engine interaction-graph
  @resource-mode optimal-sharing
  @barrier contextual-membranes
  @equality rewrite-equivalence
]
```

A barriered term is *stuck* until its scope is activated. This models necessity (box), possible worlds, and staged computation.

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

PathType auto-injects 5 rewrite rules into every theory in a PathType universe:

1. `concat(refl(a), p) ==> p` --- left identity
2. `concat(p, refl(a)) ==> p` --- right identity
3. `inv(refl(a)) ==> refl(a)` --- inverse of reflexivity
4. `concat(concat(p,q), r) ==> concat(p, concat(q,r))` --- associativity
5. `ap(f, refl(a)) ==> refl(app(f, a))` --- functorial action (requires Evaluator)

These model the groupoid structure of identity types: paths can be composed, inverted, and paths between paths form higher-dimensional structure. Categorical law verification automatically tests these rules with 4-5 witness tests.

PathType requires a lambda-capable engine (interaction-graph, term-tree, or abstract-machine).

## Von Neumann Backend

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

Von Neumann substrates reject higher-order features (Exponential, ModalOperator, TensorProduct) since sequential machines can't natively support closures, scope isolation, or parallel composition.

## Compatibility Rules

Hyperion enforces that your mathematical structure is realizable on your chosen physics:

| Category Feature | Required Substrate Properties |
|---|---|
| Exponential / Evaluator | Engine must support lambda (interaction-graph, term-tree, abstract-machine) |
| ModalOperator / Context | Barrier must support scoping (contextual-membranes, cryptographic) |
| TensorProduct | Engine must support parallel composition (interaction-graph, symmetric-monoidal) |
| PathType | Engine must support lambda (path spaces need higher-order structure) |
| topological-homotopy | Engine must support lambda (path spaces need higher-order structure) |

And certain combinations are always rejected:

| Combination | Reason |
|---|---|
| StrictlyLinear + Exponential | Linear resources can't duplicate closures |
| VonNeumann + Exponential | No lambda at hardware level |
| VonNeumann + ModalOperator | No scope isolation in sequential model |
| VonNeumann + TensorProduct | No parallel composition in sequential model |

## Prelude

Hyperion ships with a standard prelude (`prelude.hyp`) that provides common categories and substrates:

**Categories:** CartesianClosed, SymmetricMonoidal, Preorder

**Substrates:** ApeironStandard (interaction-graph + rewriting), ApeironLinear (strictly-linear), ApeironOracle (topological-hash), ApeironTree (term-tree + deep-copy)

The prelude is auto-loaded unless `--no-prelude` is passed.

## Examples

| File | What it demonstrates |
|---|---|
| `weak-lf.hyp` | Lambda calculus on an interaction net --- beta reduction, Church numerals |
| `modal-logic.hyp` | Modal necessity with barrier scopes --- locked/unlocked evaluation |
| `cross-substrate.hyp` | Functor transporting terms across substrates + verified equational preservation |
| `adjunction-demo.hyp` | Natural transformations and adjunctions between parallel functors |
| `law-check-demo.hyp` | Automatic categorical law verification (monoidal + CCC) with witness counts |
| `hott-demo.hyp` | PathType auto-injected path algebra --- empty theory, full proofs |
| `peano-vn.hyp` | Peano arithmetic compiled to Rust via Von Neumann backend |
| `prelude-demo.hyp` | Using prelude categories and substrates |

## Architecture

```
hyperion/
  src/
    category.rs      Category definitions and parsing (incl. PathType)
    substrate.rs     Substrate definitions (Engine, ResourceMode, BarrierMode, EqualityMode)
    universe.rs      Universe binding and naming
    compile.rs       Compatibility checking + Apeiron system generation
    functor.rs       Cross-substrate functor definitions (incl. :verify flag)
    nat_trans.rs     Natural transformation definitions
    adjunction.rs    Adjunction definitions
    laws.rs          Categorical law auto-generation (flat + structural witnesses)
    session.rs       Main session orchestration (incl. VerifyFunctor, PathType injection)
    codegen/
      mod.rs         Von Neumann kompile entry point
      rust_ast.rs    Lightweight Rust AST types
      analyze.rs     VN theory -> Rust AST (sort analysis, boxing, pattern matching)
      emit.rs        Rust AST -> source files
    main.rs          CLI (check + kompile subcommands)
  prelude.hyp        Standard prelude (categories + substrates)
  examples/          8 example files
  tests/
    integration.rs   65 integration tests
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

92 tests (27 unit + 65 integration), covering:

- Category/substrate/universe parsing and validation
- All compatibility rejection rules (including PathType)
- Functor transport with normalization
- Functor equational theory verification (VerifyFunctor)
- Natural transformation and adjunction validation
- Von Neumann theory capture and Rust code generation
- Categorical law pass/fail/inconclusive cases with witness counts
- PathType auto-injection and path algebra proofs
- HoTT equality mode compatibility
- All 8 example files

```bash
cargo test
```

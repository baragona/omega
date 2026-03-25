<p align="center">
  <h1 align="center">Omega</h1>
  <p align="center"><strong>A Unified Stack for Logic, Computation, and Epistemic Reasoning</strong></p>
  <p align="center">
    <a href="#the-stack">The Stack</a> &middot;
    <a href="#omega-core">Omega Core</a> &middot;
    <a href="#apeiron">Apeiron</a> &middot;
    <a href="#hyperion">Hyperion</a> &middot;
    <a href="#metacosm">Metacosm</a> &middot;
    <a href="#getting-started">Getting Started</a>
  </p>
</p>

---

## The Stack

Four layers, each a conservative extension of the one below. Every layer adds a new question without invalidating anything beneath it.

```
┌─────────────────────────────────────────────────────────────┐
│  Metacosm          What survives when we move between       │
│                    logics and engines?                       │
├─────────────────────────────────────────────────────────────┤
│  Hyperion          What computational physics runs           │
│                    the math?                                 │
├─────────────────────────────────────────────────────────────┤
│  Apeiron           Same logic, different engine —            │
│                    interaction nets with proof extraction     │
├─────────────────────────────────────────────────────────────┤
│  Omega             How do we write and verify proofs         │
│                    in a user-defined logic?                  │
└─────────────────────────────────────────────────────────────┘
```

| Layer | System | Question | Kernel |
|:------|:-------|:---------|:-------|
| 0 | **Omega** | How do we write proofs? | ~7,400 LOC Rust, zero deps |
| 1 | **Apeiron** | How do we run them on interaction nets? | ~6,000 LOC Rust, 1 dep (egg) |
| 2 | **Hyperion** | How do we decouple math from physics? | Categories + substrates + compilation passes |
| 3 | **Metacosm** | How do we reason about moving between worlds? | Typed epistemic profiles + tactic prover |

---

## Omega Core

**A logic-agnostic proof framework.** Unlike Coq, Lean, or Agda — which commit to a fixed logic — Omega ships with **no built-in logic at all**. Users define their own sorts, connectives, inference rules, and binding structures, and the kernel verifies derivations against those definitions.

The result is a single runtime that can host propositional logic, first-order logic, ZFC set theory, modal logic, linear logic, dependent type theory, HoTT, classical logic, or any formal system you can specify.

### Key Features

- **Hash-consed, arena-allocated** terms — O(1) equality, exponential sharing in constant space (2^100,001 nodes verified in ~30 µs)
- **Five proof layers**: raw derivation trees → tactics → lemmas → metatheorems → reflection
- **Miller pattern unification** for higher-order matching
- **Substructural contexts** (affine, linear) with per-binder eta/linear/affine checks
- **Dependent types**, algebraic universes, W-types, Sigma types, level polymorphism
- **Definitional equality** via user-defined rewrite rules
- **Parameterized theories** with imports and aliased namespaces
- **Theory-to-Rust compiler** (`omega kompile`) — sorts become enums, rewrite rules become `match` functions

### Architecture

```
omega-cli              Command-line interface
  └─ omega-driver        Batch processor, pipeline, code generation
       ├─ omega-elaborate   Constraint solver, unifier, tactic engine
       │    └─ omega-core
       ├─ omega-syntax      S-expression parser
       │    └─ omega-core
       └─ omega-core          Trusted kernel (~7400 LOC, zero dependencies)
```

The trusted computing base (`omega-core`) has no external dependencies and implements three operations: `register_theory`, `check_derivation`, and `check_metatheorem`.

### Quick Example

```lisp
(theory Peano
  (sort Nat) (sort Prop)
  (constructor zero : Nat)
  (constructor succ : (-> Nat Nat))
  (constructor eq   : (-> Nat Nat Prop))
  (constructor add  : (-> Nat Nat Nat))

  (judgment (proves ?P) :where P : Prop)

  (rewrite add-z  (add zero ?n)      ?n)
  (rewrite add-s  (add (succ ?n) ?m) (succ (add ?n ?m)))

  (rule eq-refl :premises () :conclusion (proves (eq ?n ?n))))

;; 1+1=2, proved by normalization alone:
(proof one-plus-one :theory Peano
  :goal (proves (eq (add (succ zero) (succ zero)) (succ (succ zero))))
  :derivation (eq-refl))
```

### What You Can Build

60+ examples spanning foundations, type theory, substructural logic, algebra, verified compilation, and more:

- **Set theory**: ZFC axioms, CH independence, large cardinals, forcing, ordinal arithmetic
- **Type theory**: STLC, System F, dependent types, HoTT, cubical types, quotients, W-types, induction-recursion
- **Substructural**: linear, affine, relevant, Lambek calculus, separation logic
- **Modal/temporal**: S5, provability logic (GL), LTL, Gödel's incompleteness
- **Verified systems**: TCP state machines, rate limiters, KV stores, calculators — all compilable to Rust
- **Compilation**: HOAS verified compilation to C, string ropes, code generation
- **Self-representation**: a proof checker encoded as rewrite rules that verifies its own proofs

> See the full [examples catalog](#examples) below.

---

## Apeiron

**A logic compiler built on interaction nets.** Same user-defined logics as Omega, but running on a fundamentally different engine — interaction nets with optimal sharing, proof-term extraction, and pluggable binding/checking strategies.

### Three Layers of Truth

```
[System]  — How do I speak?    (syntax, binding, evaluation strategy)
[Theory]  — What do I believe? (rewrite rules — the trusted base)
[Proofs]  — What do I know?    (verified theorems — sealed, read-only)
```

### Pluggable Physics

**Binding modes**: implicit, exposed, contextual, linear-explicit, nominal

**Check modes**: rewriting, beta-reduction, oracle, unification, reversible, equality-saturation, eta — and they compose.

### Key Capabilities

- **Optimal sharing** via interaction nets — non-linear variables duplicated lazily
- **Proof-term extraction** showing exact rewrite chains
- **E-graph equality saturation** with bidirectional laws
- **Existence queries** and **negative assertions** (`assert-refuted`)
- **Goal-stack tactics** (apply, auto, assumption, intro, cut, egraph)
- **AutoMorphisms** — automatic compilation between systems with different binding/checking strategies
- **Nested parameterized theories** with parameter substitution

### Example

```lisp
[System Peano
  [@syntax [sort Nat] [op z] [op s] [op add]]
  [@binding implicit]
  [@check rewriting]]

[Theory PeanoRules :in Peano
  [@rule add-z [add z ?n]      ==> ?n]
  [@rule add-s [add [s ?n] ?m] ==> [s [add ?n ?m]]]]

[Proofs Check :in PeanoRules
  [assert-eq one-plus-one [add [s z] [s z]] [s [s z]]]]
```

29 examples including Omega ports, lambda calculus, type systems, modal logic, morphisms, reversible computing, and more.

---

## Hyperion

**A logical framework framework.** Separates *math* (categories) from *physics* (substrates) and compiles the two together, verifying that your mathematical structure is actually implementable on your chosen computational engine.

### The Core Insight

Not every mathematical structure runs natively on every computational substrate. Lambda calculus needs closures. Modal logic needs scope isolation. Tensor products need parallel composition. When a category and substrate are compatible, Hyperion compiles directly. When they aren't, it inserts *compilation passes* — Girard's bang modality, Reynolds' defunctionalization, Kripke world threading — to bridge the gap.

### Three-Layer Design

- **Category** (the math): CCC, symmetric monoidal, modal, HoTT path algebra, preorder, judgment declarations
- **Substrate** (the physics): interaction graphs, term trees, von Neumann machines; resource modes (optimal sharing, linear, affine, deep copy)
- **Universe**: binds category to substrate after compatibility checking, compiles to Apeiron

### Self-Application

Categories and substrates are just data. Nothing stops you from defining a category whose objects are themselves categories, whose morphisms are functors, and whose paths are natural isomorphisms — then running it on a substrate. The framework frameworks itself, all the way up.

### Example

```lisp
[Category STLC
  :structure CartesianClosed
  :objects [Type]
  :morphisms [Term]
  :composition [app]
  :identity [id]]

[Substrate InteractionNet
  @engine interaction-graph
  @resource optimal-sharing
  @equality topological-hash]

[Universe STLCOnNets
  :category STLC
  :substrate InteractionNet]
```

---

## Metacosm

**A physics engine for mathematical epistemology.** Makes the transitions between different logical worlds formal. Where Hyperion lets you choose your physics, Metacosm lets you reason about what happens when you *change* your physics.

### The Problem

Real-world theorem proving is a pipeline: an e-graph discovers an equivalence → a directed rewriter verifies it → a compiler generates code. Each stage has different strengths. Today these transitions are informal. Metacosm makes them formal.

### Typed Epistemic Profiles

Worlds are characterized across four independent axes:

- **Discovery**: none → heuristic → semi-decidable → complete
- **Verification**: soundness × completeness × termination (independent sub-axes)
- **Canonicalization**: normalization × confluence × unique-normal-forms
- **Compression**: mode (lossless/lossy/codegen) × invertibility

### Topological Taint Tracking

When a theorem moves between worlds, what survives? Transitions compose precisely: `preserves` is an intersection, `breaks` is a union, lossy transport irreparably taints the chain.

### Substrate Physics Auditing

Epistemic claims are audited against the underlying Hyperion substrate. You cannot declare a world has complete discovery if its engine is a von Neumann machine with no equality saturation.

### Cosmological Prover

An LCF-style tactic engine for proving universal metatheorems and impossibility results that hold for *all* admissible worlds, not just registered ones.

### The Epistemic Receipt

The ultimate output: a materialized artifact wrapped in a formal guarantee of what was preserved and what was lost across the pipeline.

```
=== EPISTEMIC RECEIPT ===
  input:  [mult [s [s [s z]]] [s [s [s z]]]]
  output: [s [s [s [s [s [s [s [s [s z]]]]]]]]]    <- 3*3 = 9
  preserved: [Soundness, Completeness]
  lost:      [PathStructure, ResourceSensitivity]
  distance:  24
=========================
```

### Three Coexisting Modes

| Mode | Blocks | Behavior |
|:-----|:-------|:---------|
| **Omega** | `Theory`, `Proofs` | Routes through Hyperion to Apeiron |
| **Hyperion** | `Category`, `Substrate`, `Universe`, `Functor` | Handled by Hyperion |
| **Cosmology** | `World`, `Transition`, `Pipeline`, `Emit`, `Law`, ... | Full epistemic machinery |

120+ tests including adversarial edge cases: ouroboros topologies, Ship of Theseus leaks, categorical contradictions, physics defiers, and round-trip paradoxes.

---

## Getting Started

### Installation

```bash
git clone https://github.com/yourusername/omega
cd omega
cargo build --release
```

### Running Examples

```bash
# Omega — verify ZFC set theory
cargo run --release -- check examples/zfc.omega

# Omega — compile a verified state machine to Rust
cargo run --release -- kompile examples/tcp-state.omega --theory TcpState -o tcp-crate/

# Apeiron — run a logic on interaction nets
cd apeiron && cargo run -- examples/omega-stlc.ap

# Hyperion — compile a category onto a substrate
cd hyperion && cargo run -- examples/stlc-universe.hyp

# Metacosm — full epistemic pipeline
cd metacosm && cargo run -- check examples/golden-path.mcm -v
```

### Testing

```bash
# Everything
cargo test --workspace

# Individual crates
cargo test --test integration                    # Omega integration tests
cd apeiron && cargo test                         # Apeiron (75 tests)
cd hyperion && cargo test                        # Hyperion
cd metacosm && cargo test                        # Metacosm (120+ tests)
```

---

## Examples

### Omega (60+ examples)

<details>
<summary><strong>Foundations</strong></summary>

| Example | Description |
|:--------|:------------|
| `full-demo.omega` | Complete tour — all five proof layers from scratch (17 proofs) |
| `prop-logic.omega` | Natural deduction with conjunction, disjunction, implication |
| `first-order.omega` | Predicates, universal quantification, the Socrates syllogism |
| `classical-logic.omega` | DNE, LEM, Peirce's law, contraposition |
| `zfc.omega` | ZFC set theory — Von Neumann ordinals, pairing, union, AC (39 proofs) |
| `zfc-independence.omega` | Complete CH independence — forcing, Cohen poset, zero black boxes (25 proofs) |
| `zfc-honest.omega` | Honest CH formalization — every rule tagged as axiom/derived/admitted (29 proofs) |
| `large-cardinals.omega` | Inaccessible cardinals, Grothendieck universes, reflection principle |
| `ordinal-arithmetic.omega` | Veblen hierarchy, ε₀, Γ₀ (Feferman-Schütte ordinal) |
| `godel.omega` | Gödel's Second Incompleteness Theorem via provability logic |
| `continuum.omega` | CH independence via Boolean-valued models |
| `forcing.omega` | Gödel's L and Cohen's forcing |

</details>

<details>
<summary><strong>Type Theory</strong></summary>

| Example | Description |
|:--------|:------------|
| `stlc.omega` | Simply-typed lambda calculus with de Bruijn indices |
| `dep-types.omega` | Pi types, identity type, type computation |
| `w-types.omega` | W-types, algebraic universes, Sigma types |
| `hott.omega` | Homotopy Type Theory — J eliminator, full groupoid structure |
| `hits.omega` | Higher inductive types — circle, suspension, truncation |
| `cubical.omega` | Cubical type theory — transport and univalence that compute |
| `system-f.omega` | Impredicative polymorphism, Church encodings |
| `induction-recursion.omega` | Dybjer-Setzer pattern, universe of codes |
| `level-poly.omega` | Universe-polymorphic List, Id, Pi |
| `bidirectional.omega` | Bidirectional type checking (synthesis ⇒, checking ⇐) |

</details>

<details>
<summary><strong>Substructural Logic</strong></summary>

| Example | Description |
|:--------|:------------|
| `linear-logic.omega` | Tensor, lolli, bang modality |
| `affine-logic.omega` | Move semantics as logic |
| `relevant-logic.omega` | System R — no vacuous truths |
| `lambek.omega` | Non-commutative logic for natural language |
| `separation.omega` | Bunched implications — sharing + ownership |
| `separation-logic.omega` | Frame rule, heap reasoning |

</details>

<details>
<summary><strong>Categories, Algebra, Modal/Temporal</strong></summary>

| Example | Description |
|:--------|:------------|
| `category-theory.omega` | Yoneda lemma (16 proofs) |
| `topos.omega` | Boolean vs Heyting subobject classifiers |
| `monoid.omega` | Parameterized theory instantiated for Nat and Bool |
| `modal-logic.omega` | S5 — necessity, possibility |
| `temporal-logic.omega` | LTL — always, eventually, next, until |
| `provability-logic.omega` | Gödel-Löb logic (GL) |
| `game.omega` | Game semantics — proofs as winning strategies |

</details>

<details>
<summary><strong>Computation, Effects, Verified Systems</strong></summary>

| Example | Description |
|:--------|:------------|
| `compile-verified.omega` | HOAS verified compilation to C (19 proofs + 6 emitted functions) |
| `effects.omega` | Algebraic effects — three handlers for one program |
| `tcp-state.omega` | TCP state machine, compiled to Rust |
| `calc.omega` | Verified calculator (34 rewrite rules), compiled to Rust with REPL |
| `rate-limiter.omega` | Token-bucket algorithm with safety invariants |
| `self.omega` | Self-representation — Omega's checker encoded as rewrite rules |
| `zk-circuit.omega` | Zero-knowledge R1CS circuits |
| `streams.omega` | Coinduction and bisimulation over infinite structures |

</details>

### Apeiron (29 examples)

Omega ports, lambda calculus, optimal sharing, type systems, binding modes, morphisms, reversible computing, and more. See [`apeiron/README.md`](apeiron/README.md).

### Metacosm (9 examples)

End-to-end epistemic pipelines, tactic-proved laws, adversarial stress tests, and three-layer integration. See [`metacosm/README.md`](metacosm/README.md).

---

## The "Neutral Tool" Philosophy

Each layer is neutral about the layer below it:

- **Omega** is neutral about logic — define your own
- **Apeiron** is neutral about binding and checking — choose your physics
- **Hyperion** is neutral about the math/physics pairing — compile any combination
- **Metacosm** is neutral about epistemic regimes — formally track what you gain and lose

Want Classical? Add `axiom excluded_middle`.
Want Constructive? Don't.
Want Linear? Use `(binder-behavior tensor :linear)`.
Want HoTT? Add path axioms.
Want to run it on an e-graph? Change the substrate.
Want to know what survives the transition? Read the receipt.

---

## Comparison

| | **Omega** | **Lean 4 / Coq** | **Dedukti / Lambdapi** |
|:---|:---|:---|:---|
| Logic | User-defined | Fixed (CIC) | User-defined (rewrite rules) |
| Kernel language | Rust (zero deps) | Lean / OCaml | OCaml |
| Term representation | Interned DAG (maximal sharing) | Tree-based | Mixed |
| Equality check | O(1) (pointer comparison) | O(n) | O(n) |
| Binding control | Per-binder eta, linear, affine | Fixed | Fixed |
| Multi-engine | Apeiron (interaction nets), Hyperion (substrates) | Single engine | Single engine |
| Epistemic tracking | Metacosm (typed profiles, taint algebra) | — | — |

---

## License

[MIT](LICENSE)

# Apeiron: The Universal Logic Engine

**Apeiron** is a **logic compiler** built on interaction nets. Instead of hardwiring one logical system, it lets you **choose your physics**: configure the binding strategy and checking strategy, then write axioms and prove theorems within that system.

The kernel is ~6,000 lines of Rust. One dependency (egg for e-graphs). 29 examples. 75 tests.

---

## The Three Layers of Truth

Every Apeiron program has three layers, each with a distinct role and trust level:

```
[System]  — How do I speak?    (syntax, binding, evaluation strategy)
[Theory]  — What do I believe? (rewrite rules, definitions — the axioms)
[Proofs]  — What do I know?    (verified theorems — sealed, read-only)
```

The **System** defines the alphabet and physics. The **Theory** defines the rules — this is the trusted base. The **Proofs** block is **sealed**: it inherits the Theory's rules but cannot add new ones. Every assertion is a theorem verified by running the rules.

### Example: Peano Arithmetic

```lisp
;; Layer 1: Syntax & Physics
[System Peano
  [@syntax [sort Nat] [op z] [op s] [op add] [op mul]]
  [@binding implicit]
  [@check rewriting]
]

;; Layer 2: Axioms (the trusted base)
[Theory PeanoRules :in Peano
  [@rule add-z [add z ?n]      ==> ?n]
  [@rule add-s [add [s ?n] ?m] ==> [s [add ?n ?m]]]
  [@rule mul-z [mul z ?n]      ==> z]
  [@rule mul-s [mul [s ?n] ?m] ==> [add ?m [mul ?n ?m]]]
]

;; Layer 3: Theorems (sealed — cannot modify rules)
[Proofs PeanoCheck :in PeanoRules
  [assert-eq one-plus-one [add [s z] [s z]] [s [s z]]]
  [assert-eq two-times-three
    [mul [s [s z]] [s [s [s z]]]]
    [s [s [s [s [s [s z]]]]]]]
]
```

Output:
```
[THEORY] PeanoRules loaded
[ASSERT] one-plus-one passed
[ASSERT] two-times-three passed
[PROOFS] PeanoCheck verified (2 assertions)
```

The separation matters: if you accidentally put `@rule` inside a `[Proofs]` block, Apeiron rejects it with a clear error. Your axioms and your theorems live in different worlds.

---

## Choose Your Physics

### Binding Modes

| Mode | Description | Use case |
|:-----|:------------|:---------|
| `implicit` | High-level names, alpha-equivalence via hashing | Most logics, lambda calculus |
| `exposed` | De Bruijn indices visible in the graph | Compilers, bytecode, VM verification |
| `contextual` | Scoped barriers — terms are opaque until scope activates | Modal logic, secure enclaves |
| `linear-explicit` | Every variable used exactly once (no Dup/Erase) | Linear logic, resource tracking |
| `nominal` | Names are meaningful, no alpha-equivalence | Name-sensitive systems |

### Check Modes

| Mode | Description | Use case |
|:-----|:------------|:---------|
| `rewriting` | User-defined `[@rule lhs ==> rhs]` pattern matching | Term rewriting, computation |
| `beta-reduction` | Native lambda calculus reduction | Lambda calculus, Church encodings |
| `oracle` | Topological hashing — structure IS equality | Mathematics, extensional reasoning |
| `unification` | Pattern matching with meta-variables | Logic programming, type inference |
| `reversible` | Every rule auto-generates its inverse | Reversible computing |
| `confluent-race` | Multiple rules may match; non-deterministic | Concurrent/probabilistic systems |
| `equality-saturation` | Bidirectional `@law` rules via e-graph | Equational reasoning, algebraic simplification |
| `eta` | Eta-contraction: `(lam x (app f x)) = f` | Extensional equality |

Modes compose: `[@check rewriting beta-reduction]` gives you both.

---

## Proof-Term Extraction and Existence Queries

### `extract-proof`: Witness the Rewrite Chain

When the e-graph proves `a ≡ b`, `extract-proof` returns a structured proof term showing exactly which rules fired:

```lisp
[Proofs Check :in MyTheory
  [extract-proof my-proof [comp [comp f g] h] [comp f [comp g h]]]
]
```

Output:
```
[PROOF] my-proof = {"type":"step","rule":"assoc-fwd","from":"(comp (comp f g) h)","to":"(comp f (comp g h))","sub_proofs":[]}
```

Multi-step proofs show the full chain:
```json
{"type":"concat",
  "left":{"type":"step","rule":"unit-l-fwd","from":"(comp id a)","to":"a"},
  "right":{"type":"step","rule":"unit-r-fwd","from":"(comp a id)","to":"a"}}
```

Proof terms are built from five constructors: `Refl` (identity), `Step` (single rule application), `Concat` (transitivity), `Inv` (symmetry), and `Cong` (congruence).

### `assert-exists`: Existential Queries

Check whether a term satisfying a set of equality constraints exists:

```lisp
[Proofs Check :in MyTheory
  [assert-exists filler-exists
    :such-that
    [= [src [comp f g]] a]
    [= [tgt [comp f g]] c]]
]
```

Each constraint is a `[= lhs rhs]` pair. All constraints must be simultaneously satisfiable (via direct normalization or e-graph fallback).

### `assert-distinct-paths`: Proof-Relevant Separation

Verify that two path terms are genuinely distinct — not collapsed by the e-graph:

```lisp
[assert-distinct-paths loop-nontrivial [refl base] loop 2]
```

In proof-relevant mode, if the two terms remain in different e-classes after saturation, they are counted as distinct (non-collapse = success).

### `assert-refuted`: Negative Assertions

Verify that a goal is NOT derivable from the given assumptions and derive rules:

```lisp
[Proofs Check :in MyTheory
  [assert-refuted impossible-goal
    :assumptions [[holds a a]]
    :goal [holds b b]
    :depth 5]
]
```

Succeeds if exhaustive backward-chaining search fails to find a proof. Fails if the goal turns out to be derivable. Supports `:strategy forward` for forward-chaining search.

### `tactic`: Goal-Stack Proof Construction

Build proofs step-by-step using backward reasoning tactics:

```lisp
[Proofs Check :in MyTheory
  [tactic modus-ponens-proof
    :goal [typeof p q]
    :assumptions [[typeof p p]]
    :steps [
      [apply mp]          ;; match rule conclusion against goal, push premises
      [auto 3]            ;; backward-chain search on first subgoal
      [assumption]         ;; discharge from assumptions
    ]]
]
```

Available tactic steps:
- **`[apply rule-name]`** — Match a derive rule's conclusion against the top goal; push premises as subgoals
- **`[auto N]`** — Automatic backward-chaining search up to depth N
- **`[assumption]`** — Discharge the top goal if it matches an assumption
- **`[exact term]`** — Discharge the top goal if the term matches exactly
- **`[cut lemma-name]`** — Add a proved lemma's conclusion to assumptions
- **`[intro]`** — Move the first argument of a judgment goal into assumptions
- **`[egraph]`** — Close the top goal via e-graph equality saturation (requires `equality-saturation` check mode). The goal must be `[head lhs rhs]` where `lhs ≡ rhs` is provable by the e-graph. When `equality-saturation` is active, `[auto N]` also falls back to the e-graph if backward-chaining fails.

### Nested Parameterized Theories

Parameterized theories can now import other parameterized theories. Parameters are substituted through nested imports:

```lisp
[Theory EqDecide :params [[T Sort] [eq_op Op]] :in System
  [@rule eq-refl [eq_op ?x ?x] ==> true]
]

[Theory MonoidTemplate :params [[T Sort] [binop Op] [unit Op]] :in System
  [@rule unit-l [binop unit ?x] ==> ?x]
  [import EqDecide T eq :as Eq]    ;; T is substituted when MonoidTemplate is instantiated
]

[Theory NatMonoid :in System
  [import MonoidTemplate Nat add z :as M]
  ;; M.Eq.eq-refl is now available (nested instantiation)
]
```

---

## The Interaction Net Kernel

Under the hood, Apeiron reduces terms on an **interaction net** — a graph where computation happens by local node-pair rewrites. This gives:

- **Optimal sharing**: Non-linear variables are duplicated via Dup nodes that cascade lazily. `compose(quadruple, quadruple)(3) = 48` computes in 74 interactions, not by copying intermediate results.
- **Stack-safe**: No recursion in the reducer. A 100,000-deep Peano numeral normalizes without stack overflow.
- **Topological equality**: Two terms are equal iff their graph topologies hash identically. Alpha-equivalence is free.

### How rewriting works

When you write `[@rule add-z [add z ?n] ==> ?n]`, Apeiron compiles the LHS into a pattern and the RHS into a template. The physics engine runs beta-reduction and Dup/Erase annihilation to normal form, then the rewrite scanner matches patterns against live nodes and fires rules by graph surgery.

Non-linear patterns (same meta-variable twice) verify structural equality via readback comparison:
```lisp
;; ?A appears twice — both occurrences must be structurally equal
[@rule chk-app-ok [chk-app [ty [arrow ?A ?B]] [ty ?A]] ==> [ty ?B]]
```

---

## AutoMorphisms: The Universal Translator

Apeiron auto-generates **morphisms** (compilers) between systems with different binding or checking strategies.

```lisp
[AutoMorphism Compile HighLevel LowLevel
  [Map plus add]          ;; rename operators
  [@strict true]          ;; reject unmapped ops
]
```

The kernel detects binding mismatches (Implicit → Exposed) and auto-generates de Bruijn indexing. It detects checking mismatches (Compute → Oracle) and enables normalize-before-send.

---

## Omega Ports: Same Logic, Different Engine

Apeiron can host the same logical systems as [Omega](../), our S-expression logical framework. Three ports demonstrate this:

### Peano Arithmetic (`omega-peano.ap`)
Direct 1:1 port. Same 4 rewrite rules, same 6 proofs. Omega uses a hash-consed tree normalizer; Apeiron uses an interaction net. Same results.

### Self-Checking Proof System (`omega-self.ap`)
A propositional logic checker + automated solver encoded entirely as rewrite rules. The checker validates its own proofs; the solver builds proof trees automatically. Includes Ackermann function: `ack(3,3) = 61` in 1,241 interactions. 30 assertions across two logical systems.

### STLC Type Checker (`omega-stlc.ap`)
The most innovative port. Omega's STLC uses explicit judgments and derivation trees. Apeiron encodes the **type checker itself** as rewrite rules: `[typeof ctx term]` normalizes to `[ty T]`. The derivation tree is implicit — the rewrite trace IS the typing proof. 10 typing theorems including the S combinator.

---

## Example Catalog

29 examples spanning logic, computation, and compilation:

| Category | Examples |
|:---------|:---------|
| **Omega Ports** | `omega-peano.ap`, `omega-self.ap`, `omega-stlc.ap` |
| **Lambda Calculus** | `church-numerals.ap`, `church-power.ap`, `alpha-equivalence.ap` |
| **Rewriting** | `arithmetic.ap`, `streams.ap`, `higher-order-rewrite.ap` |
| **Optimal Sharing** | `sharing-demo.ap` (compose(quadruple,quadruple)(3)=48) |
| **Logic** | `logic-programming.ap`, `modal-logic.ap`, `unified-logic.ap` |
| **Type Systems** | `weak-lf.ap`, `inductive-types.ap`, `linear-linter.ap` |
| **Binding Modes** | `linear-types.ap`, `nominal.ap`, `contextual-alpha.ap`, `mixed-binding.ap` |
| **Morphisms** | `automorphism.ap`, `morphism-zoo.ap`, `grand-unification.ap`, `leibniz.ap` |
| **Exotic Modes** | `reversible.ap`, `nondeterministic.ap`, `differential.ap` |
| **Compilation** | `stack-compiler.ap`, `reflection.ap` |

---

## Quick Start

```bash
# Run an example
cargo run -- examples/omega-stlc.ap

# Run all tests (75 tests, ~1s)
cargo test

# Try your own
cargo run -- my-theory.ap
```

## Architecture

```
src/
  arena.rs      — Node arena with recycling and scope management
  builder.rs    — S-expression → interaction net graph compiler
  egraph.rs     — E-graph equality saturation (via egg), proof-term extraction, proof-relevant mode
  hash.rs       — Topological hashing (alpha-equivalence, equality)
  interact.rs   — Interaction rules: beta, dup, erase, barrier, sym
  morphism.rs   — AutoMorphism engine: binding/checking/op translation
  node.rs       — Node types: Lam, App, Dup, Erase, Sym, Barrier, Future
  parser.rs     — S-expression parser with bracket syntax
  physics.rs    — Main reduction loop (physics scheduler)
  readback.rs   — Graph → tree readback for display
  rewrite.rs    — Pattern compiler + graph rewrite engine
  system.rs     — System/Theory/Proofs processing + session management
```

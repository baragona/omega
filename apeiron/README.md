# Apeiron: The Universal Logic Engine

**Apeiron** is a **logic compiler** built on interaction nets. Instead of hardwiring one logical system, it lets you **choose your physics**: configure the binding strategy and checking strategy, then write axioms and prove theorems within that system.

The kernel is ~6,000 lines of Rust. No dependencies. 29 examples. 122 tests.

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

Modes compose: `[@check rewriting beta-reduction]` gives you both.

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

# Run all tests (122 tests, ~1s)
cargo test

# Try your own
cargo run -- my-theory.ap
```

## Architecture

```
src/
  arena.rs      — Node arena with recycling and scope management
  builder.rs    — S-expression → interaction net graph compiler
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

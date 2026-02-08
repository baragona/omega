<p align="center">
  <h1 align="center">Omega</h1>
  <p align="center"><strong>A Reflective Logical Framework</strong></p>
  <p align="center">
    <a href="#getting-started">Getting Started</a> &middot;
    <a href="#features">Features</a> &middot;
    <a href="#architecture">Architecture</a> &middot;
    <a href="#examples">Examples</a>
  </p>
</p>

---

Omega is a **logic-agnostic proof framework** written in Rust. Unlike systems such as Coq, Lean, or Agda — which commit to a fixed logic (CIC, MLTT) — Omega ships with **no built-in logic at all**. Users define their own sorts, connectives, inference rules, and binding structures, and the kernel verifies derivations against those definitions.

The result is a single runtime that can host propositional logic, first-order logic, ZFC set theory, modal logic, linear logic, simply-typed lambda calculus, or any other formal system you can specify.

## Performance

Omega's kernel operates on a **hash-consed, arena-allocated** term representation. Structurally identical sub-terms are stored exactly once, reducing equality checks to pointer comparisons and enabling verification of terms with exponential sharing in constant space.

| System | Term Size | Time | Result |
| :--- | :--- | :--- | :--- |
| Tree-Based Checker | 2^100,001 nodes | — | OOM (would require >10^30 PB) |
| **Omega (Interned)** | 2^100,001 nodes | **~3 µs** | Verified |

> Benchmark: structural verification of a recursively doubled term at depth 100,000. See `examples/torture.omega`.

## Features

### Logic as Configuration

Define sorts, term constructors, operators with custom binding specifications, and inference rules — all in a declarative S-expression syntax.

```lisp
(theory Peano
  (sort Nat)
  (sort Prop)

  (constructor zero : Nat)
  (constructor succ : (-> Nat Nat))
  (constructor eq   : (-> Nat Nat Prop))

  (judgment (proves ?P) :where P : Prop)

  (rule eq-refl
    :premises ()
    :conclusion (proves (eq ?n ?n)))

  (rule eq-symm
    :premises ((proves (eq ?a ?b)))
    :conclusion (proves (eq ?b ?a))))
```

### Constraint-Based Elaboration

Omega's elaborator infers implicit arguments via first-order unification with occurs check, deferred constraints, and transitive fixpoint resolution. Proof terms stay concise.

```lisp
;; eq-trans declares ?b as implicit:
(rule eq-trans
  :premises ((proves (eq ?a ?b)) (proves (eq ?b ?c)))
  :conclusion (proves (eq ?a ?c))
  :implicit (?b))

;; The user writes a derivation without the middle term:
(eq-trans (eq-refl) (eq-refl))

;; The elaborator creates a fresh meta for ?b and solves it
;; via unification against the sub-derivation conclusions.
```

### Proof by Reflection

Prove a metatheorem about a theory's rules via case analysis, then **reflect** it into the kernel as a new inference rule.

```lisp
;; 1. Prove commutativity of 'and' as a metatheorem:
(meta-theorem and-comm-meta
  :theory SimpleLogic
  :forall ((D (proves (and ?A ?B))))
  :exists ((D' (proves (and ?B ?A))))
  :proof (case-analysis D
    (case and-intro (D1 D2)
      (by-rule and-intro D2 D1))))

;; 2. Reflect it as a new rule:
(reflect and-comm-meta :as proves/and-comm :theory SimpleLogic)

;; 3. Use the reflected rule in proofs:
(proof comm-test
  :theory SimpleLogic
  :assumptions ((proves (and p q)))
  :goal (proves (and q p))
  :derivation (proves/and-comm (assumption)))
```

## Comparison

| | **Omega** | **Lean 4 / Coq** | **Dedukti / Lambdapi** |
| :--- | :--- | :--- | :--- |
| Logic | User-defined, reflective | Fixed (CIC) | User-defined (rewrite rules) |
| Kernel language | Rust (zero deps) | Lean / OCaml | OCaml |
| Term representation | Interned DAG (maximal sharing) | Tree-based | Mixed |
| Equality check | O(1) (pointer comparison) | O(n) | O(n) |
| Surface syntax | S-expressions | Algol-style | Algol-style |

## Getting Started

### Installation

```bash
git clone https://github.com/yourusername/omega
cd omega
cargo build --release
```

### Running an Example

```bash
# Verify ZFC set theory
cargo run --release -- check examples/zfc.omega

# Verify the exponential-sharing stress test
cargo run --release -- check examples/torture.omega
```

### Examples

| File | Description |
| :--- | :--- |
| `prop-logic.omega` | Propositional logic |
| `first-order.omega` | First-order predicate logic |
| `zfc.omega` | ZFC set theory fragment |
| `peano.omega` | Peano arithmetic |
| `stlc.omega` | Simply-typed lambda calculus |
| `modal-logic.omega` | S5 modal logic |
| `linear-logic.omega` | Linear logic (tensor, bang, lolli) |
| `implicit-demo.omega` | Implicit argument inference |
| `peano-compute.omega` | Peano arithmetic with definitional equality |
| `reflection-demo.omega` | Proof by reflection |
| `torture.omega` | Exponential term stress test |

## Architecture

Omega is structured as a Rust workspace with a strict dependency hierarchy:

```
omega-cli              Command-line interface
  └─ omega-driver        Batch processor and pipeline
       ├─ omega-elaborate   Constraint solver, unifier, tactic engine
       │    └─ omega-core
       ├─ omega-syntax      S-expression parser, locally nameless encoding
       │    └─ omega-core
       └─ omega-core          Trusted kernel (~5100 LOC, zero dependencies)
```

**`omega-core`** is the trusted computing base. It has no external dependencies and implements four operations: `register_theory`, `check_derivation`, `check_metatheorem`, and `reflect`. Everything above it is untrusted elaboration.

## Roadmap

- [x] User-defined binding specifications
- [x] Hash-consed interned kernel
- [x] Constraint unification and implicit arguments
- [x] Definitional equality (delta reduction)

## License

[MIT](LICENSE)

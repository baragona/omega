<p align="center">
  <h1 align="center">Omega</h1>
  <p align="center"><strong>A Logic-Agnostic Logical Framework</strong></p>
  <p align="center">
    <a href="#getting-started">Getting Started</a> &middot;
    <a href="#features">Features</a> &middot;
    <a href="#architecture">Architecture</a> &middot;
    <a href="#examples">Examples</a>
  </p>
</p>

---

Omega is a **logic-agnostic proof framework** written in Rust. Unlike systems such as Coq, Lean, or Agda — which commit to a fixed logic (CIC, MLTT) — Omega ships with **no built-in logic at all**. Users define their own sorts, connectives, inference rules, and binding structures, and the kernel verifies derivations against those definitions.

The result is a single runtime that can host propositional logic, first-order logic, ZFC set theory, modal logic, linear logic, simply-typed lambda calculus, dependent type theory, classical logic, or any other formal system you can specify.

## Performance

Omega's kernel operates on a **hash-consed, arena-allocated** term representation. Structurally identical sub-terms are stored exactly once, reducing equality checks to pointer comparisons and enabling verification of terms with exponential sharing in constant space.

| System | Term Size | Time | Result |
| :--- | :--- | :--- | :--- |
| Tree-Based Checker | 2^100,001 nodes | — | OOM (would require >10^30 PB) |
| **Omega (Interned)** | 2^100,001 nodes | **~30 µs** | Verified |

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

Prove a metatheorem about a theory's rules via case analysis, then **reflect** it as a new inference rule. The kernel verifies the metatheorem; the driver installs the derived rule.

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
| Logic | User-defined | Fixed (CIC) | User-defined (rewrite rules) |
| Kernel language | Rust (zero deps) | Lean / OCaml | OCaml |
| Term representation | Interned DAG (maximal sharing) | Tree-based | Mixed |
| Equality check | O(1) (pointer comparison) | O(n) | O(n) |
| Binding control | Per-binder eta, linear, affine | Fixed | Fixed |
| Surface syntax | S-expressions | Algol-style | Algol-style |

## The "Neutral Tool" Philosophy

By being neutral, Omega is more powerful than specialized tools in their own domains.

- **Coq** forces you into Constructive Logic. (Hard to do classical math.)
- **Isabelle** forces you into Classical Logic. (Hard to do constructive math.)
- **Rust** forces you into Affine Logic. (Hard to do GC/sharing.)
- **Omega** lets you choose.

Want Classical? `axiom excluded_middle : Or A (Not A)`.
Want Constructive? Don't add it.
Want Linear? Use `(binder-behavior tensor :linear)`.
Want Affine? Use `(context-mode affine)`.
Want HoTT? Add path axioms.

Omega is powerful enough to:

- **Verify a Rust-like type system** (`libs/omega-rust/`)
- **Formalize ZFC set theory** (`examples/zfc.omega`)
- **Formalize Homotopy Type Theory** (`examples/hott.omega`)
- **Compile verified programs to C** (`examples/compile-verified.omega`)
- **Model induction-recursion and HITs** (`examples/induction-recursion.omega`, `examples/hits.omega`)

The kernel is done. The rest is just writing `.omega` files.

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
| `affine-logic.omega` | Substructural (affine) logic with move semantics |
| `number-theory.omega` | Induction proofs with Miller pattern unification |
| `monoid.omega` | Parameterized theories (dual instantiation) |
| `compiler-demo.omega` | Multi-theory imports (Option, Result, Pair) |
| `codegen-demo.omega` | String ropes and C code generation via `emit` |
| `sequent-calc.omega` | Sequent calculus (left/right rules, cut) |
| `hoare-logic.omega` | Hoare triples (assignment, frame, sequence) |
| `classical-logic.omega` | Classical logic (DNE, LEM, Peirce's law) |
| `dep-types.omega` | Dependent types (Pi, identity type) |
| `compile-factorial.omega` | Verify and compile factorial in one file |
| `compile-verified.omega` | HOAS verified compilation (same lambda evaluates AND compiles) |
| `w-types.omega` | Algebraic universes, W-types, Sigma types |
| `hott.omega` | Homotopy Type Theory (path algebra, groupoid laws) |
| `category-theory.omega` | Category theory with Yoneda lemma |
| `induction-recursion.omega` | Induction-recursion (mutual U/El codes) |
| `hits.omega` | Higher inductive types (circle, suspension, truncation) |
| `level-poly.omega` | Level-polymorphic constructors and rules |
| `eta-demo.omega` | Eta-contraction (nested eta, composition rewrites) |
| `linear-demo.omega` | Per-binder linear and affine variable checks |
| `ac-demo.omega` | Associative-commutative normalization |
| `torture.omega` | Exponential term stress test |

#### Standard Library (`libs/`)

| File | Description |
| :--- | :--- |
| `option.omega` | Option(T) — parameterized, none/some, elimination |
| `result.omega` | Result(T,E) — parameterized, ok/err, elimination |
| `pair.omega` | Pair(A,B) — parameterized, affine context mode |
| `string.omega` | StringLib — rope constructors for code generation |

#### OmegaRust (`libs/omega-rust/`)

| File | Description |
| :--- | :--- |
| `rust-types.omega` | Rust type system (lifetimes, Copy, subtyping) |
| `borrow.omega` | Borrow checker via affine contexts |
| `eval.omega` | Operational semantics via rewrite rules |

## Architecture

Omega is structured as a Rust workspace with a strict dependency hierarchy:

```
omega-cli              Command-line interface
  └─ omega-driver        Batch processor and pipeline
       ├─ omega-elaborate   Constraint solver, unifier, tactic engine
       │    └─ omega-core
       ├─ omega-syntax      S-expression parser, locally nameless encoding
       │    └─ omega-core
       └─ omega-core          Trusted kernel (~7400 LOC, zero dependencies)
```

**`omega-core`** is the trusted computing base. It has no external dependencies and implements three operations: `register_theory`, `check_derivation`, and `check_metatheorem`. Reflection is driver-level sugar — the kernel verifies the metatheorem, and the driver installs the derived rule. Everything above the kernel is untrusted elaboration.

## Roadmap

- [x] User-defined binding specifications
- [x] Hash-consed interned kernel
- [x] Constraint unification and implicit arguments
- [x] Definitional equality (delta reduction / rewrite rules)
- [x] Substructural (affine) contexts
- [x] Context extensions and induction
- [x] Miller pattern unification (higher-order)
- [x] Theory imports
- [x] Parameterized theories / modules
- [x] String ropes and code generation (`emit`)
- [x] Dependent types (Pi) and classical logic (DNE)
- [x] HOAS verified compilation
- [x] Algebraic universes, W-types, Sigma types
- [x] Induction-recursion and higher inductive types
- [x] Level-polymorphic declarations
- [x] Per-binder eta-contraction, linear/affine checks
- [x] Reflection moved out of kernel (three-operation trusted core)

## License

[MIT](LICENSE)

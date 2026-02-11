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
- **Prove Gödel's Second Incompleteness Theorem** (`examples/godel.omega`)
- **Derive classical logic from game-theoretic strategies** (`examples/game.omega`)
- **Compare Boolean vs Heyting topoi** (`examples/topos.omega`)

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

#### Foundations

**`examples/prop-logic.omega`** — Propositional Logic (5 proofs)

Natural deduction with conjunction, disjunction, and implication. Context extensions enable hypothetical reasoning — `A → (B → (A ∧ B))` adds assumptions to nested premises. Identity, weakening, commutativity, currying, and or-commutativity.

**`examples/first-order.omega`** — First-Order Logic (5 proofs)

Predicates over terms, universal quantification, and equality substitution. The classic Socrates syllogism: "all humans are mortal, Socrates is human, therefore Socrates is mortal" — formalized as universal instantiation through pattern matching.

**`examples/classical-logic.omega`** — Classical Logic (5 proofs)

Double negation elimination as a single axiom, deriving everything classical: LEM, Peirce's law `((P→Q)→P)→P`, contraposition, and negation of implication. Explicit `(assumption N)` for disambiguating context search in deeply nested proofs.

**`examples/paraconsistent.omega`** — Paraconsistent Logic (10 proofs)

Contradictions without explosion. Russell's paradox `R∈R ∧ R∉R` exists as a dialetheia — a true contradiction — but doesn't infect the entire system. The key: omit the standard explosion rule `⊥ → A`, so contradictions remain locally contained.

**`examples/zfc.omega`** — ZFC Set Theory (15 proofs)

Von Neumann ordinals (0=∅, 1={∅}, 2={∅,{∅}}), pairing, union, powerset, and the Axiom of Choice with a Skolem operator. Deep implicit-argument chains: `eq-trans` infers middle terms across 3-step equality reasoning over set membership.

**`examples/sequent-calc.omega`** — Sequent Calculus (8 proofs)

Left/right introduction rules, explicit structural rules (weakening, exchange, contraction), and the cut rule. Proves the same theorem both directly and via cut-based reasoning — the sequent calculus analog of the "detour" that cut-elimination removes.

#### Type Theory

**`examples/stlc.omega`** — Simply-Typed Lambda Calculus (10 proofs)

De Bruijn indices with explicit typing contexts. All the combinators: I (identity), K (const), S (7-level derivation tree), B (composition), Church pair, flip, and Church numeral 2. Eta-expansion in a non-empty STLC context demonstrates deep variable lookup.

**`examples/dep-types.omega`** — Dependent Types (7 proofs)

Pi types (dependent function spaces), identity type, and type computation via beta-reduction. The key proof: `(λx.λy.refl) z : Π(y:Nat). Eq(Nat, y, y)` — partial application triggers WHNF reduction to solve the type family.

**`examples/w-types.omega`** — W-Types and Universes (10 proofs)

Algebraic universe levels (lzero, lsuc, lmax, imax) with AC normalization. W-type recursion via built-in `wrec` reduction. Sigma types with `fst`/`snd` as rewrite rules. Impredicative Prop: `imax(l, 0) = 0` for CIC-style universe stratification.

**`examples/hott.omega`** — Homotopy Type Theory (12 proofs)

The J eliminator (path induction), transport, ap (action on paths), and the full groupoid structure: left/right unit laws, left/right inverse, involution of path inversion, and transport over concatenation. All proved via J — no axioms beyond reflexivity.

**`examples/hits.omega`** — Higher Inductive Types (12 proofs)

Circle (S¹ with base and loop), suspension (north, south, merid), and propositional truncation. Typing derivations for path composition `concat(loop, loop)`, path inversion, and the squash constructor. Recursor computation rules as definitional equalities.

**`examples/system-f.omega`** — System F (10 proofs)

Impredicative polymorphism where `∀α. α→α` can be instantiated with itself — the self-application that STLC forbids. Church-encoded booleans as polymorphic functions, type abstraction/application, and the identity polymorphism that makes System F the simplest system with type:type flavor.

**`examples/extensional.omega`** — Extensional Type Theory (10 proofs)

The reflection rule turns propositional equality proofs into definitional equality — if you can prove `a = b`, the kernel treats them as identical. Consequences: function extensionality for free and UIP (all identity proofs are equal). The price: type checking becomes undecidable.

**`examples/two-level.omega`** — Two-Level Type Theory (10 proofs)

Inner fibrant level (HoTT-like, no UIP) and outer strict level (with UIP and reflection) running simultaneously, connected by one-way lifting. Staged compilation and parametricity reasoning — the inner level is the "object language" and the outer level is the "meta language."

**`examples/quantitative.omega`** — Quantitative Type Theory (10 proofs)

Dependent types meet resource tracking. Each binder carries a quantity annotation: 0 (erased at runtime), 1 (used exactly once), or ω (unrestricted). Semiring laws govern quantity composition. Per-binder usage checks enforce the annotations — a linear function that discards its argument is a type error.

**`examples/induction-recursion.omega`** — Induction-Recursion (12 proofs)

Mutual definition of a universe of codes U and decoding function El — the Dybjer-Setzer pattern. `El(nat-code) = Nat` and `El(pi-code(a, b)) = Pi(El(a), El(b(a)))` as rewrite rules. Symmetry, transitivity, and Pi congruence over decoded types.

**`examples/level-poly.omega`** — Level Polymorphism (13 proofs)

Universe-polymorphic List, Id, and Pi with `lmax` as ACI-normalized level computation. `nil : List(List(Nat))` requires chaining `t-list(t-nat)` through the level system. Pi types at generic levels with assumptions providing universe witnesses.

#### Substructural Logic

**`examples/linear-logic.omega`** — Linear Logic (6 proofs)

Multiplicative conjunction (tensor), linear implication (lolli), additive connectives (with/oplus), and the bang modality. `!A ⊢ A ⊗ A` shows how bang enables resource duplication — the fundamental difference from affine logic.

**`examples/affine-logic.omega`** — Affine Logic (10 proofs)

Move semantics as logic: each assumption used at most once. Tensor introduction splits resources, tensor elimination decomposes them. Linear implication via context extensions. The crown jewel: `A⊸B, B⊸C ⊢ A⊸C` — linear function composition chaining three rules deep.

**`examples/relevant-logic.omega`** — Relevant Logic (10 proofs)

System R: implications `A → B` require A to actually be used in deriving B, rejecting vacuous truths like `B → (A → A)`. Linear binder constraints forbid both weakening (discarding) and contraction (duplicating), creating a logic strictly between linear and classical.

**`examples/lambek.omega`** — Lambek Calculus (10 proofs)

Non-commutative substructural logic for natural language parsing. Tensor product `A ⊗ B ≠ B ⊗ A` — word order matters. Directed implications: `A\B` (A needed on the left) vs `B/A` (A needed on the right). Type-raising, composition, and parsing transitive verb constructions as logical derivations.

**`examples/separation.omega`** — Space (10 proofs)

Bunched Implications: classical ∧ (sharing) and linear \* (ownership) in one system. The distribution bridge (P∧Q)\*R ⊢ (P\*R)∧(Q\*R). Heap verification: framed writes, pointer swaps, composed operations.

**`examples/separation-logic.omega`** — Separation Logic (10 proofs)

Extends Hoare logic with separating conjunction `P * Q` for disjoint heap regions, enabling local reasoning about heap-manipulating programs. The frame rule: `{P}c{Q}` implies `{P*R}c{Q*R}` — unrelated heap state is preserved automatically. Allocation, deallocation, and pointer operations.

#### Categories and Algebra

**`examples/category-theory.omega`** — Yoneda Lemma (16 proofs)

Categories, functors, and natural transformations with the full Yoneda bijection. Part 1: 10 definitional equalities (all by normalization — composition laws, functor laws, ψ/φ maps). Part 2: 6 multi-step proofs using naturality, double congruence, and right cancellation via sections.

**`examples/category.omega`** — Structure (10 proofs)

Cartesian Closed Categories. Morphisms are proofs, composition is cut, products are conjunction, exponentials are implication. The hypothetical syllogism as a 5-level categorical morphism.

**`examples/monoid.omega`** — Parameterized Monoid Theory (8 proofs)

A single `MonoidTheory` parameterized over carrier, operation, and identity — instantiated for Nat with addition and Bool with conjunction. Triple associativity via `trans(assoc, assoc)`, unit simplification via `cong-l(right-id)`, and double congruence from assumptions.

**`examples/topos.omega`** — The Engine of Truth (10 proofs)

Two subobject classifiers computed side by side:
- Boolean Ω = {⊤,⊥}: ¬¬x = x, LEM holds → Classical
- Heyting Ω = {⊤,u,⊥}: ¬¬u = ⊤ ≠ u, LEM fails → Intuitionistic

Same connectives, same rules. Change Ω, change the logic. Ω defined inside Omega.

#### Modal and Temporal Logic

**`examples/modal-logic.omega`** — S5 Modal Logic (6 proofs)

Box (necessity) and diamond (possibility) with axioms K, T, and 5. Positive introspection, necessitation, and the S5 theorem `◇A → □◇A` — possibility is itself necessary.

**`examples/provability-logic.omega`** — Provability Logic (10 proofs)

Gödel-Löb logic (GL) where `□P` means "P is provable" and Löb's axiom `□(□P→P) → □P` replaces the standard T axiom. Box distribution, provability of tautologies, and the groundwork for Gödel's incompleteness — related to but distinct from the full treatment in `godel.omega`.

**`examples/temporal.omega`** — Time (10 proofs)

State machines as categories. Traffic light cycles and mutex lock protocols. Safety by absence (no Held→Held morphism), liveness by reachability.

**`examples/temporal-logic.omega`** — Linear Temporal Logic (10 proofs)

LTL with first-class temporal operators: always (□), eventually (◇), next (○), and until (U). Proves operator interactions, distribution laws, and eventuality guarantees over infinite execution traces. The fundamental duality: `□A ≡ ¬◇¬A`.

**`examples/godel.omega`** — The Limits of Proof (10 proofs)

Provability logic (GL) with the Gödel sentence G ↔ ¬□G. Highlights:
- Proof 9 (Second Incompleteness): Con → ¬□Con in 5 lines — "if consistent, you can't prove your own consistency." The proof: □Con → □⊥ via Löb, then Con turns □⊥ into ⊥.
- Proofs 7-8: □(G → ¬□G) and □(¬□G → G) — the system knows what G means, it just can't decide it.

#### Computation and Effects

**`examples/monad.omega`** — Effects (10 proofs)

Hoare logic as Kleisli category. Monad laws verified by rewriting. Counter state machine with {n=0} inc;inc;inc {n=3}.

**`examples/game.omega`** — Logic as Interaction (10 proofs)

Game semantics: proofs are winning strategies. Copycat (A→A), fork (combine two responses), case analysis (or-elimination), and Peirce's law — the classical "bluff" strategy where the Prover assumes ¬A, catches the Opponent in a contradiction, and wins.

**`examples/hoare-logic.omega`** — Program Verification (5 proofs)

Hoare triples {P}c{Q} with assignment, frame rule, sequencing, and conditionals. The frame rule enables local reasoning — `{P}c{Q}` implies `{P∧R}c{Q∧R}` when c doesn't touch R. Conditional proof combines two branches into a disjunctive postcondition.

**`examples/pi.omega`** — Pi-Calculus (10 proofs)

Session-typed concurrent processes where protocols are types and processes are proofs. Duality guarantees deadlock freedom: if one side sends, the other receives. Channel creation, parallel composition, and session continuation — the Curry-Howard correspondence for concurrency.

**`examples/cut-elim.omega`** — Cut Elimination (11 proofs)

Proofs as programs with cut elimination as computation. Linear logic proof terms where each cut-reduction step is a rewrite rule and Omega's normalizer is the abstract machine. Tensor projection, additive selection, dereliction, duplication, and composition all reduce to normal forms by `eq-refl`.

#### Verified State Machines

**`examples/kv-store.omega`** — Verified Key-Value Store (10 proofs)

Transactional KV store state machine with put/get/delete operations, begin/commit/rollback transaction logic, and safety invariants: read-your-writes consistency and multi-key isolation. State transitions modeled as rewrite rules.

**`examples/rate-limiter.omega`** — Token-Bucket Rate Limiter (14 proofs)

Token-bucket algorithm with Peano natural number tokens capped at maximum capacity. Safety invariant: accept count never exceeds token count. Burst capacity, token replenishment, and request handling — all verified by normalization.

**`examples/tcp-state.omega`** — TCP State Machine (10 proofs)

Simplified TCP connection lifecycle: Closed → Listen → SynRecvd → Established → FinWait → Closed. Transitions as rewrite rules, safety proofs for valid state sequences, and Rust code generation via string ropes and `emit`.

**`examples/tcp-server.omega`** — TCP Server with Effects (5 proofs)

Pure state machine (`step`, `can-send`, `is-open`) separated from abstract I/O effects (`bind-port`, `send-ack`). The effect dispatch pattern: verify the logic in Omega, implement the I/O boundary in Rust via a generated effects trait.

**`examples/calc.omega`** — Verified Calculator (16 proofs)

5 sorts, 29 constructors, 34 rewrite rules. Peano arithmetic (add, mul, sub, pow, factorial), control flow (if, min, max), expression AST with eval dispatch, and effect-based output. Kompiles to Rust with a REPL frontend for interactive use.

#### Verified Compilation

**`examples/compile-verified.omega`** — HOAS Verified Compilation (19 proofs, 6 emitted C functions)

ONE program definition, TWO uses. A lambda like `λx. x+x` is both an evaluator (apply to 3 → 6) and a compiler (apply to `var("x")` → `"x + x"`). Beta reduction bridges the two worlds. Five HOAS functions (double, square, abs, triple, is-zero) verified by normalization AND compiled to C. Multi-step congruence proofs: from `n=m`, derive `triple(n)=triple(m)` in a 3-step chain.

**`examples/compile-factorial.omega`** — Verified Factorial (6 proofs + C emission)

Factorial verified from 0! through 5! by Peano normalization, then compiled to `int factorial(int n)` via string rope translation. The same rewrite rules that compute `3! = 6` also guide the C code generation.

**`examples/codegen-demo.omega`** — String Ropes and C Generation

Tree-based string construction: fragments built during proof, flattened only at emit time. Generates a complete C program with `#include`, `main()`, `printf`, and `return 0` — all assembled from rope combinators.

#### Metatheory and Reflection

**`examples/reflection-demo.omega`** — Proof by Reflection (2 metatheorems, 10 proofs)

Two metatheorems verified by exhaustive case analysis — `and-comm` and `or-comm` — then reflected as new inference rules. The reflected rules compose: `and-comm(and-comm(x)) = x` (roundtrip), cross-connective reasoning `A∧B ⊢ A∨B` via elimination + introduction, and `A∨B ⊢ (B∨A) ∧ (B∨A)` combining or-comm with contraction.

**`examples/number-theory.omega`** — Induction via Miller Patterns (3 proofs)

Structural induction over naturals with higher-order pattern unification. `(?P ?n)` matched against `(eq (add ?n z) ?n)` automatically solves `?P → λx.(eq (add x z) x)`. Proves `n+0=n`, the successor lemma, and commutativity of addition — all over ALL naturals, not just examples.

**`examples/implicit-demo.omega`** — Implicit Arguments (10 proofs)

The `eq-trans` rule declares `?b` implicit — the middle term is inferred from sub-derivation conclusions. Triple transitivity `a=b, b=c, c=d ⊢ a=d` chains two `eq-trans` calls, each inferring a different implicit. Deep congruence-symmetry composition: `a=b ⊢ succ(succ(b)) = succ(succ(a))`.

#### Logic Translations

**`examples/girard.omega`** — Girard Translation (10 proofs)

Call-by-name translation from classical logic to linear logic. Every classical hypothesis gets wrapped in `!` (of-course), enabling the contraction and weakening that linear logic forbids by default. Classical ∧ becomes ⊗, classical ∨ becomes ⊕, and implication becomes `!A ⊸ B`. Four bridge theorems with identity as the canonical proof.

**`examples/glivenko.omega`** — Glivenko Translation (10 proofs)

Every classical tautology becomes intuitionistic when double-negated: if `⊢_c A` then `⊢_i ¬¬A`. The 1929 theorem that bridges classical and intuitionistic logic. Continuation monad interpretation: `¬¬A` is `(A → ⊥) → ⊥`, so classical proofs are CPS-transformed intuitionistic proofs.

**`examples/collapse.omega`** — Paraconsistent Collapse (12 proofs)

Consistency filter extracting the classically safe fragment from paraconsistent logic. Dialetheias (propositions that are both true and false) are tolerated but don't escape — a "robust truth" predicate distinguishes genuine truths from mere dialetheias, restoring DNE for the safe fragment.

#### Kernel Features

**`examples/peano-compute.omega`** — Definitional Equality (6 proofs)

Arithmetic by normalization alone. Rewrite rules for addition and multiplication fire during proof checking — `1+1=2`, `2×3=6`, and `3×3=9` are all proved by `eq-refl` after the kernel reduces both sides to the same normal form.

**`examples/eta-demo.omega`** — Eta-Contraction (12 proofs)

With `(binder-behavior lambda :substitutive :eta)`, the arena canonicalizes `λx.(f x)` to `f` at intern time. Nested eta works: `λx.λy.(f x y)` contracts to `f` in two steps. Part 2 combines eta with equational reasoning — `f=g ⊢ (λx.(f x)) = g` succeeds because the LHS eta-contracts before matching.

**`examples/linear-demo.omega`** — Per-Binder Usage Checks (5 proofs)

Fine-grained resource tracking at the lambda abstraction level: linear binders require exactly one use, affine binders allow zero or one. Different from context-level substructurality — each binder independently controls its variable's usage.

**`examples/ac-demo.omega`** — AC Normalization (12 proofs)

Symbols declared `:ac` are flattened, sorted by structural hash, and rebuilt right-associative at intern time. `op(a, op(b, c)) = op(c, op(a, b))` by `eq-refl`. ACI adds idempotency: `join(a, join(a, join(b, b))) = join(a, b)`. Part 2 adds equational reasoning with assumptions over AC-normalized terms.

**`examples/torture.omega`** — Exponential Sharing Stress Test

Hash-consing benchmark: a term doubled 100,000 times has 2^100,001 nodes as a tree but ~50 unique interned handles. The interned checker verifies it in ~30μs. The tree checker would need more memory than exists.

**`examples/compiler-demo.omega`** — Multi-Theory Imports (10 proofs)

The "final boss" of the module system: imports Option(Int), Result(Int, String), and Pair(Int, Bool) into a single theory. Each parameterized theory instantiated with different types, all coexisting with aliased namespaces. Cross-module equality reasoning.

#### Standard Library (`libs/`)

**`libs/option.omega`** — Option(T): parameterized theory with none/some constructors and case elimination. Single type parameter.

**`libs/result.omega`** — Result(T, E): parameterized with two type parameters, ok/err constructors, and case elimination.

**`libs/pair.omega`** — Pair(A, B): parameterized with affine context mode — pairs consume their components linearly.

**`libs/string.omega`** — StringLib: rope constructors (empty, cat, newline) with identity rewrites. The backbone of code generation.

#### OmegaRust (`libs/omega-rust/`)

**`libs/omega-rust/rust-types.omega`** — Rust type system formalized: U32, Bool, Box, Ref, MutRef, Pair, Fn, Option, Result. Lifetimes with outlives relation, Copy trait derivation, and reference subtyping via covariance. 10 proofs.

**`libs/omega-rust/borrow.omega`** — Borrow checker as affine logic: Box is move-only (consumed on use), resource splitting across pairs, shared and mutable references with lifetime tracking. 10 proofs.

**`libs/omega-rust/eval.omega`** — Operational semantics via rewrite rules: pair projection, box dereference, option unwrap, conditionals, and Peano arithmetic. All proved by `eq-refl` after normalization. 10 proofs.

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

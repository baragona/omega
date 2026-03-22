# Hyperion

**A Logical Framework Framework Framework.**

A research prototype exploring whether category theory can systematically classify the design space of logical frameworks. It's closer to a "type system for type systems" than a compiler tool.

Hyperion is a meta-system for building logical frameworks. You describe the *mathematics* you want (a category) and the *computational physics* you want it to run on (a substrate), and Hyperion compiles the two together into a working logical system, verifying that your math is actually implementable on your chosen physics.

The core insight: not every mathematical structure can run on every computational substrate. Lambda calculus needs an engine that supports closures. Modal logic needs scope isolation. Tensor products need parallel composition. Hyperion enforces these constraints at compile time, then generates the correct backend configuration automatically.

But Hyperion's real power is *self-application*. Categories and substrates are just data. Nothing stops you from defining a category whose objects are themselves categories, whose morphisms are functors, and whose paths are natural isomorphisms --- and then running it on a substrate. The framework frameworks itself, all the way up.

## Why Hyperion?

Apeiron gave us the ultimate freedom to define our math, but it still made one massive assumption: it forced all of that math to run on one specific type of "computer" --- the Interaction Net. It was a logical framework framework, but it was stuck in a single physical reality.

Hyperion asks: *what if the "physics" of the system could be just as customizable as the math?*

To understand Hyperion, think of it like designing a video game. Apeiron let you invent the rules of the game (the math), but forced you to play it on a specific console (the Interaction Net). Hyperion takes a step further back and says, "Let's abstract the console, too."

Hyperion splits the world into two distinct pieces:

- **The Category (The Math):** This is the pure logic. What are the rules? What do the objects look like? Are we doing standard algebra, or weird modal logic with parallel universes?
- **The Substrate (The Physics):** This is the engine that actually executes the math. Are we running this on a highly parallel graph (like Apeiron's Interaction Nets)? Are we running it sequentially on a standard computer chip? Are resources infinitely copyable, or are they strictly linear (meaning once you use a variable, it's gone forever)?

Hyperion acts as the ultimate matchmaker. You hand it your Math and your Physics, and it checks to see if they are actually compatible. For example, if your math requires infinite parallel processing, but you try to run it on a strictly sequential physics engine, Hyperion will stop you and say, "This math physically cannot exist in this universe."

If they are compatible, Hyperion compiles them together into a working system called a **Universe**.

But here is where it gets truly wild --- the reason we can call it a "logical framework framework framework" (LF^3).

Because everything in Hyperion is just data, nothing stops you from using Hyperion to define a math system whose only job is to talk about *other math systems*. It is frameworks all the way up.

Even better, because you have separated the math from the physics, you can use **Functors** (translators) to bridge entirely different computational realities. You can have a super-advanced, complex physics engine autonomously discover a brilliant mathematical proof, and then transport that exact proof over to a much simpler, basic physics engine just to mechanically verify it. The knowledge survives the jump across different universes.

So, to trace the whole journey:

| System | Question it answers |
|---|---|
| **Lean/Coq** | How do we write proofs on this specific, complex mathematical foundation? |
| **Omega** | How do we build a simpler, generic foundation to write proofs on? |
| **Apeiron** | How do we let users build their own foundations, running on a shared engine? |
| **Hyperion** | How do we let users define the laws of physics that govern the engines that run the foundations? |

It strips everything away until all you are left with is the pure relationship between what is logically true, and what is physically computable.

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

A **Substrate** declares computational physics: what engine executes your terms (interaction graphs, term trees, cellular automata, symmetric monoidal nets, von Neumann machines), how resources are managed (optimal sharing, linear, affine, deep copy), how scoping works (transparent, contextual membranes, cryptographic), and what notion of equality the system uses (rewriting, hashing, unification, homotopy, equality saturation, proof-relevant).

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

## Proof-Term Extraction

When the e-graph proves an equality, `extract-proof` returns a structured proof term showing the chain of named rewrite steps. This is critical for higher-categorical verification where the proof term IS the higher-dimensional cell (e.g., the associator 2-cell in a 2-category).

```
[Proofs MonoidCheck :in MonoidTheory
  ;; Extract the associator as a proof term
  [extract-proof alpha_fgh
    [comp [comp f g] h]
    [comp f [comp g h]]]

  ;; Multi-step proof: unit-l then unit-r
  [extract-proof unit-chain
    [comp id [comp a id]]
    a]
]
```

Output:
```
[PROOF] alpha_fgh = {"type":"step","rule":"assoc-fwd","from":"(comp (comp f g) h)","to":"(comp f (comp g h))"}
[PROOF] unit-chain = {"type":"concat",
  "left":{"type":"step","rule":"unit-l-fwd","from":"(comp id (comp a id))","to":"(comp a id)"},
  "right":{"type":"step","rule":"unit-r-fwd","from":"(comp a id)","to":"a"}}
```

Proof terms are built from five constructors:
- **Refl**: identity (`a ≡ a`)
- **Step**: single rule application with rule name, source, target
- **Concat**: transitivity (chain two proofs)
- **Inv**: symmetry (reverse a proof)
- **Cong**: congruence (same head, proofs for each argument)

When `assert-eq` succeeds via the e-graph, the proof term is now included in the output automatically.

## Existence Queries (`assert-exists`)

Check whether terms satisfying equality constraints exist, without providing explicit witnesses:

```
[Proofs KanCheck :in KanTheory
  ;; Does a composite with the right source and target exist?
  [assert-exists composite_exists
    :such-that
    [= [src [comp f g]] a]
    [= [tgt [comp f g]] c]]

  ;; Does an associator 2-cell exist?
  [assert-exists assoc_exists
    :such-that
    [= [src2 [assoc_cell f g h]] [comp [comp f g] h]]
    [= [tgt2 [assoc_cell f g h]] [comp f [comp g h]]]]
]
```

Each constraint is a `[= lhs rhs]` pair. All constraints must be simultaneously satisfiable via direct normalization or e-graph fallback. This enables verification of Kan conditions: "for every composable pair, there exists a filler."

## Proof-Relevant Equality Mode

A substrate mode where the e-graph tracks labeled edges instead of collapsing identity. Two terms can be connected by multiple distinct paths:

```
[Substrate HoTTSub
  @engine interaction-graph
  @resource-mode optimal-sharing
  @barrier transparent
  @equality proof-relevant        ;; NEW MODE
]
```

Use `assert-distinct-paths` to verify that two path terms are genuinely distinct — not collapsed by the e-graph:

```
[Proofs CircleCheck :in CircleTheory
  ;; refl(base) and loop are both base=base paths, but distinct
  [assert-distinct-paths loop_nontrivial [refl base] loop 2]

  ;; loop and loop∘loop are also distinct
  [assert-distinct-paths winding_distinct loop [concat loop loop] 2]
]
```

In proof-relevant mode, if the two terms remain in different e-classes after saturation, they are counted as distinct (non-collapse = success). This preserves path spaces for HoTT: `refl` and `loop` on S¹ are different paths even though they share endpoints.

## Kernel Cubical Reduction

The `[IntervalSort]` categorical structure enables kernel-level reduction rules for cubical type theory. These fire as directed `@rule` rewrites before e-graph saturation:

```
[Category CubicalCat
  [Object Type]
  [Morphism coe    :domain [Type Type Type] :codomain Type]
  [Morphism hcomp  :domain [Type Type]      :codomain Type]
  [Morphism refl   :domain [Type]           :codomain Type]
  [Morphism concat :domain [Type Type]      :codomain Type]
  [Morphism inv    :domain [Type]           :codomain Type]
  [Morphism ap     :domain [Type Type]      :codomain Type]
  [PathType :refl refl :concat concat :inv inv :ap ap]
  [PartialElement :hcomp hcomp :coe coe]
  [IntervalSort :interval I :endpoints [i0 i1]]   ;; NEW
]
```

When `IntervalSort`, `PathType`, and `PartialElement` are all present, Hyperion auto-injects kernel cubical reduction rules:

| Rule | Reduction |
|------|-----------|
| `coe(refl(A), i, x)` | `x` (from PartialElement) |
| `hcomp(refl(a), base)` | `base` (from PartialElement) |
| `coe(concat(p, q), i, x)` | `coe(q, i, coe(p, i, x))` (composite path decomposition) |
| `coe(inv(p), i, x)` | `coe(p, i, x)` (inverse path unwrapping) |

These are deterministic reductions that fire at the kernel level. Without them, `coe` along complex paths doesn't simplify and verification times out.

```
[Proofs CubicalCheck :in CubicalTT
  [assert-eq coe-refl    [coe [refl A] i0 x] x]
  [assert-eq coe-concat  [coe [concat p q] i0 x] [coe q i0 [coe p i0 x]]]
  [assert-eq coe-inv     [coe [inv p] i0 x] [coe p i0 x]]
  [assert-eq hcomp-refl  [hcomp [refl A] x] x]
]
```

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

### Autonomous Proof Discovery

In an `equality-saturation` substrate, the e-graph doesn't just verify user-stated equalities --- it *discovers* consequences autonomously. Given a set of independent `@law` declarations, the e-graph saturates the equivalence classes and can close non-obvious multi-step proof paths that no human hinted at.

The Eckmann-Hilton argument demonstrates this. Five independent primitive laws (interchange + 4 unit laws) are declared for two binary operations (`concat` for vertical composition, `hcomp` for horizontal). From these alone, the e-graph autonomously discovers:

1. **Coincidence**: `concat(α, β) = hcomp(α, β)` --- the two operations are the same
2. **Commutativity**: `concat(α, β) = concat(β, α)` --- vertical composition commutes

The proof path requires ~7 rewrite steps using laws in *both* directions (inserting units via reverse application, then eliminating via forward). No tautology, no shortcut --- pure equational saturation over independent axioms.

```
[Theory EckmannHilton :in MetaEqUniverse :no-laws
    [const base_obj Cat]
    [const alpha Cat]
    [const beta Cat]

    [@law interchange
        [hcomp [concat_cat ?a ?b] [concat_cat ?c ?d]]
        === [concat_cat [hcomp ?a ?c] [hcomp ?b ?d]]]
    [@law hcomp-left-id  [hcomp [refl_cat base_obj] ?p] === ?p]
    [@law hcomp-right-id [hcomp ?p [refl_cat base_obj]] === ?p]
    [@law concat-left-id  [concat_cat [refl_cat base_obj] ?p] === ?p]
    [@law concat-right-id [concat_cat ?p [refl_cat base_obj]] === ?p]
]

[Proofs EckmannHiltonCheck :in EckmannHilton
    ;; The e-graph discovers both consequences from the 5 primitives
    [assert-eq eckmann-hilton-coincide [concat_cat alpha beta] [hcomp alpha beta]]
    [assert-eq eckmann-hilton-commutes [concat_cat alpha beta] [concat_cat beta alpha]]
]
```

Crucially, the *same* five laws in a `rewrite-equivalence` substrate fail to prove either consequence. Directed normalization can only apply rules forward, and neither `concat(α, β)` nor `hcomp(α, β)` matches any rule head --- they're stuck terms. This proves that the discovery is genuine: it emerges from the e-graph's bidirectional saturation physics, not from the law content alone.

See `eckmann-hilton.hyp` for the full 5-part demonstration.

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

### Epistemic Transport Across Physics Boundaries

Functors enable a powerful pattern: **knowledge generated by advanced physics can be serialized and verified by weaker physics**. An `equality-saturation` substrate can autonomously discover theorems (via bidirectional `@law` reasoning), then a functor transports the computed results to a `rewrite-equivalence` substrate where they are mechanically verified using only forward rules.

```
;; E-graph world discovers theorems via bidirectional @law declarations
[Theory Discovery :in EGraphWorld :no-laws
    [@law interchange [hcomp [concat_cat ?a ?b] [concat_cat ?c ?d]]
                  === [concat_cat [hcomp ?a ?c] [hcomp ?b ?d]]]
    ;; ... 4 more laws ...
    [def compound [hcomp [concat_cat alpha [refl_cat base_obj]]
                         [concat_cat [refl_cat base_obj] beta]]]
]

;; Directed world receives transported normal forms via functor
[Theory Received :in DirectedWorld :no-laws
    [@rule interchange [hcomp [concat_cat ?a ?b] [concat_cat ?c ?d]]
                   ==> [concat_cat [hcomp ?a ?c] [hcomp ?b ?d]]]
    ;; ... 4 more rules ...
    [Import compound-t [InsightTransport compound]]  ;; Transported!
]

[VerifyFunctor InsightTransport :source Discovery :target Received]

[Proofs TransportVerification :in Received
    ;; Transported result verified: compound normalized to hcomp(alpha, beta)
    [assert-eq transport-compound compound-t [hcomp alpha beta]]

    ;; The physics gap: directed world CANNOT discover the coincidence
    [assert-neq gap-coincide [concat_cat alpha beta] [hcomp alpha beta]]
]
```

The `Import` command normalizes the source expression on its native substrate, then ships the normal form across the functor. `VerifyFunctor` confirms that all source laws are preserved in the target. The directed world can then mechanically verify the transported results --- but it *cannot independently discover* the theorems that the e-graph found, because its forward-only rules lack the bidirectional reasoning power.

This proves: discovery flows one way (advanced physics → serialization → verification), and the physics gap is real.

See `transport-discovery.hyp` for the full pipeline.

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

Adding `[Morphism hcomp :domain [Cat Cat] :codomain Cat]` to the meta-category introduces horizontal composition of 2-cells, modeling a **2-category** where objects are categories, 1-morphisms are functors, and 2-morphisms are natural transformations. PathType provides vertical composition (`concat`) and reflexivity (`refl`), while `hcomp` provides horizontal composition. With the interchange law relating the two compositions, this is the structure needed for the Eckmann-Hilton argument at the meta-level.

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
| PartialElement | `hcomp`, `coe` | 2 cubical `@rule`s (coe-refl, hcomp-refl) | Nothing |
| IntervalSort + PathType + PartialElement | `I`, `i0`, `i1` | 2 kernel cubical `@rule`s (coe-concat, coe-inv) | Nothing |
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
| `eckmann-hilton.hyp` | 5-part Eckmann-Hilton: auto-injected PathType interchange, ap-concat distributivity, naturality gap, e-graph autonomous discovery (coincidence + commutativity from 5 laws), physics dependence (same laws fail in directed substrate) |
| `transport-discovery.hyp` | Meta-categorical transport across physics boundaries: e-graph discovers theorems, VerifyFunctor confirms 5 law preservations, Import transports normal forms to directed substrate, mechanical verification + physics gap (assert-neq) |
| `lf3-grand-tour.hyp` | Comprehensive showcase: CCC + PathType + Monoidal + Modal + Preorder + resources + barriers in one file |
| `ouroboros-linear.hyp` | Self-application under linear resource constraints |
| `schrodinger-egraph.hyp` | E-graph meets modal barriers --- scope isolation stress test |
| `verify-functor-resource.hyp` | Resource-aware VerifyFunctor (linear-to-linear transport) |
| `prelude-demo.hyp` | Using prelude categories and substrates |
| `catlab-features.hyp` | Proof-term extraction, assert-exists, proof-relevant mode, kernel cubical reduction |

## Architecture

```
hyperion/
  src/
    category.rs      Category definitions and parsing (CCC, Monoidal, PathType, Preorder, Modal, IntervalSort)
    substrate.rs     Substrate definitions (Engine, ResourceMode, BarrierMode, EqualityMode incl. ProofRelevant)
    universe.rs      Universe binding and naming
    compile.rs       Compatibility checking + Apeiron system generation
    functor.rs       Cross-substrate functor definitions (incl. :verify flag)
    nat_trans.rs     Natural transformation definitions
    adjunction.rs    Adjunction definitions
    laws.rs          Categorical law auto-generation (flat + structural witnesses)
    session.rs       Main session orchestration (VerifyFunctor, PathType/Preorder/Cubical injection)
    codegen/
      mod.rs         Von Neumann kompile entry point
      rust_ast.rs    Lightweight Rust AST types
      analyze.rs     VN theory -> Rust AST (sort analysis, boxing, pattern matching)
      emit.rs        Rust AST -> source files
    main.rs          CLI (check + kompile subcommands)
  prelude.hyp        Standard prelude (categories + substrates)
  examples/          21 example files
  tests/
    integration.rs   107 integration tests
```

Hyperion depends on [Apeiron](../apeiron/) for term rewriting, beta reduction, and oracle checking. The Von Neumann backend is the only path that bypasses Apeiron entirely.

## CLI Reference

```
hyperion check <file.hyp> [-v] [--no-prelude] [--skip-laws] [--json] [--stdin]
hyperion kompile <file.hyp> --theory <name> -o <output_dir/>
```

| Flag | Effect |
|---|---|
| `-v` | Print stats (categories, substrates, universes, Apeiron node counts) |
| `--no-prelude` | Don't load the standard prelude |
| `--skip-laws` | Don't verify categorical laws after theory loading |
| `--json` | Output structured JSON (CatLab schema: status, results, discoveries) |
| `--stdin` | Read input from stdin instead of a file |
| `--theory` | Theory name for kompile |
| `-o` | Output directory for generated Rust crate |

## Tests

160 tests (42 unit + 118 integration), covering:

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
- Autonomous e-graph discovery (Eckmann-Hilton coincidence + commutativity from independent axioms)
- Cross-physics epistemic transport (e-graph → functor → directed verification + physics gap)
- Proof-term extraction via egg's Explanation API (`extract-proof`)
- Existence queries (`assert-exists` with `:such-that` constraints)
- Proof-relevant equality mode (`@equality proof-relevant`, `assert-distinct-paths`)
- Kernel cubical reduction (`IntervalSort` + auto-injected coe-concat, coe-inv rules)
- All example files

```bash
cargo test
```

## Design Philosophy

Hyperion takes four positions:

**Math and physics are orthogonal.** The same lambda calculus can run on interaction graphs, term trees, or von Neumann machines. The same monoidal structure can live on linear or affine substrates. Hyperion enforces the necessary constraints but otherwise stays out of the way.

**Honesty over ceremony.** Law verification tests on witness atoms and reports the count. It doesn't claim universal truth. VerifyFunctor checks rule preservation concretely. Fuel exhaustion is inconclusive, not failure. The system tells you exactly what it checked and what it couldn't.

**Self-application is the test.** If Hyperion can't framework itself --- if you can't define a category of categories and reason about functors between functors --- then the abstraction leaks. The examples prove it doesn't: `wild-linear-meta.hyp` has meta-categories with PathType at the meta-level, and it all just works.

**Physics determines provability.** Identical mathematical content produces different provability depending on the substrate's equality physics. The same five equational laws declared as `@law` in an `equality-saturation` substrate enable the e-graph to autonomously discover that vertical composition is commutative (the Eckmann-Hilton argument). The same five laws in a `rewrite-equivalence` substrate are compiled as forward-only rules, and both consequences are unprovable --- `assert-neq` confirms the gap. The substrate isn't decoration; it's load-bearing. This is why Hyperion exists: to make the relationship between mathematical structure and computational physics explicit, enforced, and exploitable.

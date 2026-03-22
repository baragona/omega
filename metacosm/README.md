# Metacosm

**A Physics Engine for Mathematical Epistemology.**

Most theorem provers give you one world. You write proofs in it. Maybe it's Lean's dependent type theory, Coq's CIC, or a custom logic you built in Omega. But it's always *one* world, with one fixed set of capabilities.

In the real world, theorem proving is a pipeline:

1. An **e-graph** autonomously discovers an equivalence (complete discovery, but produces no certificates).
2. A **directed rewriter** takes that discovery and mechanically verifies it (no discovery, but produces auditable proofs).
3. A **compiler** takes the verified result and generates executable code (fast and confluent, but loses all proof structure).

Today, the transitions between these fundamentally different epistemic regimes are informal. A human decides what to trust.

Metacosm makes them formal. It asks: what if you had many worlds, each with different strengths, and you could formally reason about moving theorems between them?

---

## The Stack

Metacosm is the third layer of a unified architectural stack. Each layer is a conservative extension of the one below it.

| Layer | System | Question |
|-------|--------|----------|
| 0 | **Omega / Apeiron** | How do we write proofs in a user-defined logic? |
| 1 | **Hyperion** | How do we define the *physics* that runs the logic? |
| 2 | **Metacosm** | How do we formally reason about *moving between* logics and physics? |

**Omega** (via Apeiron) is a logic-agnostic proof framework. You define your own sorts, constructors, judgments, and rules, and Omega checks derivations against them. It's a logical framework --- like LF, but with hash-consing, Miller pattern unification, rewrite rules, and substructural contexts.

**Hyperion** separates math from physics. A *Category* declares pure mathematical structure (CCC, monoidal, modal, HoTT). A *Substrate* declares computational physics (interaction graphs, term trees, von Neumann machines). A *Universe* binds the two together after checking compatibility. The same lambda calculus can run on an e-graph or a term rewriter --- Hyperion enforces that the math is actually implementable on the chosen physics.

**Metacosm** takes Hyperion universes and makes them dynamic. Universes become *worlds* with epistemic profiles. You can measure their capabilities, transition between them, compose transitions, and prove that invariants survive.

Each layer is designed as a conservative extension of the one below. Hyperion files work unchanged in Metacosm (all Hyperion block types are routed). Omega's core blocks (`Theory`, `Proofs`) pass through Hyperion to Apeiron unperturbed.

---

## Key Features

### 1. Typed Epistemic Profiles

Worlds aren't just loosely tagged; they are mathematically defined across four independent axes forming an epistemic lattice:

**Discovery** --- can the world find new theorems?
```
none < heuristic < semi-decidable < complete-fragment < complete
```

**Verification** --- can the world check whether a claim is true? A *product* of three independent sub-axes:
```
soundness:    none < heuristic < sound
completeness: none < partial < complete
termination:  unknown < semi-decidable < decidable
```

**Canonicalization** --- can the world produce normal forms? Another product:
```
normalization: none < weak < strong
confluence:    boolean
unique-normal-forms: boolean
```

**Compression** --- can the world reduce representation size?
```
mode: none | lossless | lossy | quotient | abstraction | codegen
lossy: boolean
invertible: boolean
```

Worlds can also declare **theorem-class sensitivity** --- different epistemic profiles for different kinds of reasoning:

```clojure
[World Explorer
    :epistemic [:discover complete :verify sound]
    :class-epistemic [
        [Equational       :discover complete :verify decidable]
        [ResourceSensitive :discover none    :verify heuristic]
    ]
]
```

Explorer is excellent at equational reasoning but can't handle linear logic. This isn't a limitation of the formalism --- it's the reality of e-graphs, and Metacosm makes it explicit.

### 2. Topological Taint Tracking & Composition Algebra

When a theorem moves from an e-graph to a compiler, what survives? Metacosm calculates pipeline integrity using a strict algebra. Transitions compose precisely: **preserves** is an intersection, **breaks** is a union, and lossy transport modes irreparably taint the structural witness of the chain.

```clojure
[Transition ExploreToCertify
    :kind Tunnel
    :from Explorer :to Certifier
    :transport [:mode witness :loss [PathStructure]]
    :preserves [Soundness Completeness]
    :breaks [PathStructure]
]

[Compose ExploreToExecute
    :transitions [ExploreToCertify CertifyToExecute]
]
;; preserves = {Soundness, Completeness} ∩ {Soundness} = {Soundness}
;; breaks    = {PathStructure} ∪ {ResourceSensitivity, PathStructure}
;; transport = max(witness, lossy) = lossy
```

### 3. Substrate Physics Auditing

Metacosm isn't just a metadata linter; it's physically aware. The engine audits your epistemic claims against the underlying Hyperion substrate. If you declare a world has complete discovery, but its underlying engine is a von Neumann abstract machine with no equality saturation, the audit fails. You cannot declare a world is a genius if its brain is a toaster.

```clojure
;; This FAILS the audit:
[Substrate DumbSubstrate
    @engine von-neumann
    @equality topological-hash
]

[World Delusional
    :substrate DumbSubstrate
    :epistemic [:discover complete :verify decidable :canonicalize unique-nf]
]

[CheckWorld Delusional]
;; → AUDIT FAIL: complete discovery requires equality-saturation
;; → AUDIT FAIL: strong normalization requires a term rewriting engine
```

The audit catches 10+ incompatibility rules between category structures, substrate engines, and epistemic claims --- including class-level overrides that try to exceed their substrate's physical limits.

### 4. Cosmological Prover (The "Dangerous Lifts")

Metacosm features an embedded LCF-style tactic engine for proving universal metatheorems (`[Law]`) and categorical contradictions (`[Impossibility]`). These are the *dangerous lifts* --- they hold for **every admissible world**, including ones not yet imagined.

```clojure
;; Prove dominance is transitive --- for ALL worlds, not just registered ones
[Law DominanceTransitive
    :forall [?W1 ?W2 ?W3 :type World]
    :assume [[dominates ?W1 ?W2] [dominates ?W2 ?W3]]
    :show [dominates ?W1 ?W3]
    :method proof
    :proof [
        [unfold dominates]
        [intros-axis ?A]
        [apply lattice-transitivity :on ?A]
        [qed]
    ]
]
```

Available tactics: `unfold`, `intros-axis`, `apply`, `contradiction`, `split`, `assumption`, `qed`. Built-in axioms: `lattice-reflexivity`, `lattice-transitivity`, `pigeonhole-principle`, `dominates-antisymmetry`, `preservation-intersection`.

Three verification methods coexist honestly:
- **`model-check`**: search over registered worlds (honest: reports count, not universality)
- **`structural`**: pattern-matched algebraic proofs (built-in families)
- **`proof`**: tactic scripts over the epistemic lattice (true metatheory)

---

## The Capstone: The Epistemic Receipt

The ultimate output of a Metacosm pipeline isn't just a compiled binary; it's a materialized artifact wrapped in a formal epistemic guarantee.

When you push a term through an `[Emit]` block, Metacosm evaluates it through the Omega normalizer, tracks its journey through the pipeline, and generates an **Epistemic Receipt**:

```clojure
[Emit ThreeTimesThree
    :term [mult [s [s [s z]]] [s [s [s z]]]]
    :theory PeanoArithmetic
    :pipeline GoldenPath
    :format epistemic-receipt
]
```

```
=== EPISTEMIC RECEIPT ===

--- Payload ---
  input:  [mult [s [s [s z]]] [s [s [s z]]]]
  output: [s [s [s [s [s [s [s [s [s z]]]]]]]]]      <- 3*3 = 9
  theory: PeanoArithmetic

--- Journey ---
  step 1: Discover(Explorer)
  step 2: Tunnel(Explorer) -> Certifier [distance=11] [lost: PathStructure]
  step 3: Verify(Certifier)
  step 4: CoarseGrain(Certifier) -> Executor [distance=13] [lost: ResourceSensitivity, PathStructure]
  step 5: Measure(Executor)

--- Invariants ---
  preserved: [Soundness, Completeness]
  lost:      [PathStructure, ResourceSensitivity]
  distance:  24

--- Cost ---
  interactions:    12
  term size (in):  16
  term size (out): 37
=========================
```

You don't have to trust the compiler blindly. You read the receipt. The cosmological laws guarantee the journey.

---

## Quick Syntax Overview

```clojure
;; Define a world and its physical limitations
[World Certifier
    :category STLC
    :substrate Rewrite
    :epistemic [
        :discover heuristic
        :verify [:soundness sound :completeness complete :termination decidable]
        :canonicalize unique-nf
        :compress lossless
    ]
]

;; Define a transition and what survives the jump
[Transition ExploreToCertify
    :kind Tunnel
    :from Explorer :to Certifier
    :functor Extract
    :transport [:mode witness :loss [PathStructure]]
    :preserves [Soundness Completeness]
    :breaks [PathStructure]
]

;; Assert properties and have them formally checked
[Assert [dominates Explorer Executor]]
[Assert [preserves GoldenPipeline Soundness]]
[Assert [faithful ExploreToCertify]]

;; Prove a universal law about your universe
[Law DominanceTransitive
    :forall [?W1 ?W2 ?W3 :type World]
    :assume [[dominates ?W1 ?W2] [dominates ?W2 ?W3]]
    :show [dominates ?W1 ?W3]
    :method proof
    :proof [
        [unfold dominates]
        [intros-axis ?A]
        [apply lattice-transitivity :on ?A]
        [qed]
    ]
]

;; Execute a term through the full pipeline with epistemic tracking
[Emit Result
    :term [mult [s [s [s z]]] [s [s [s z]]]]
    :theory PeanoArithmetic
    :pipeline GoldenPath
    :format epistemic-receipt
]
```

---

## Part 1: Worlds and Epistemic Profiles

A world is a Hyperion universe annotated with a typed epistemic profile describing what it can and can't do.

### Declaring a World

Short syntax sugars into the full product form:

```clojure
;; Short syntax
[World Explorer
    :category CartesianClosed
    :substrate EGraphSubstrate
    :epistemic [
        :discover complete
        :verify sound                 ;; sugar for soundness=sound, rest=default
        :canonicalize weak-nf         ;; sugar for normalization=weak, rest=default
        :compress none
    ]
]

;; Full decomposed syntax
[World Certifier
    :category CartesianClosed
    :substrate RewriteSubstrate
    :epistemic [
        :discover heuristic
        :verify [:soundness sound :completeness complete :termination decidable]
        :canonicalize [:normalization strong :confluence yes :unique-normal-forms yes]
        :compress [:mode lossless :lossy no :invertible yes]
    ]
]
```

---

## Part 2: Transitions

A transition is a formal declaration of what happens when you move a theorem from one world to another.

### Transition Kinds

| Kind | Meaning |
|------|---------|
| `Tunnel` | Transport a theorem with witnesses intact |
| `CoarseGrain` | Lossy compression (drop detail, keep essentials) |
| `ConservativeExtension` | Target extends source without invalidating anything |
| `Split` | One world branches into two |
| `Merge` | Two worlds combine |
| `Collapse` | Lossy projection |
| `Refinement` | Add detail |
| `Quotient` | Identify equivalent things |
| `Transport` | Generic movement |
| `PhaseTransition` | Qualitative change of regime |

### Transport Modes

| Mode | What crosses |
|------|-------------|
| `Witness` | Full proof witnesses |
| `TheoremOnly` | Bare theorem statements |
| `Conservative` | Everything (conservative extension) |
| `Lossy` | Selectively --- some information dropped |

---

## Part 3: World Morphisms and Functors

When a transition carries a Hyperion functor, it becomes a *world morphism* with derived categorical properties:

- **Faithful**: injective on morphisms (no proof collapsing --- distinct proofs stay distinct)
- **Full**: surjective on morphisms (every target proof has a preimage)
- **Structure-preserving**: which categorical structures (exponentials, tensor products, etc.) survive the translation

```clojure
[Functor Extract
    :from EGraph :to Rewrite
    :map-object [Type Type] :map-object [Term Term]
    :map-morphism [app app]
]

[Transition ExploreToCertify
    :kind Tunnel
    :from Explorer :to Certifier
    :functor Extract              ;; Functorial semantics
    :preserves [Soundness]
]
```

---

## Part 4: Epistemic Inference

The engine runs constraint propagation to fixpoint (max 20 iterations), applying rules like:

- **Tunnel preserving Soundness**: if the source has `verify: sound`, the target must too
- **Conservative extension**: target must dominate source on *all* epistemic axes
- **Witness transport from a sound source**: target must be at least as sound

Inference only fills in *default* values. If you explicitly declared something, inference won't overwrite it --- it reports a **conflict** instead.

---

## Part 5: Assertions and Laws

### Assertions

User-declared formal claims, checked against the session state:

```clojure
[Assert [dominates Explorer Executor]]
[Assert [preserves TheoremPipeline Soundness]]
[Assert [preserves-transition ExploreToExecute Soundness]]
[Assert [faithful DiscoverTunnel]]
[Assert [distance Explorer Executor :max 10]]
```

### Cosmological Laws

Universal properties about all admissible worlds:

```clojure
;; Model-checked (honest: "holds for N registered worlds")
[Law DominanceReflexive
    :forall [W]
    :then [dominates W W]
    :method model-check
]

;; Tactic-proved (true metatheory: holds for ALL admissible worlds)
[Law DominanceTransitive
    :forall [?W1 ?W2 ?W3 :type World]
    :assume [[dominates ?W1 ?W2] [dominates ?W2 ?W3]]
    :show [dominates ?W1 ?W3]
    :method proof
    :proof [
        [unfold dominates]
        [intros-axis ?A]
        [apply lattice-transitivity :on ?A]
        [qed]
    ]
]
```

### Impossibility Proofs

Prove that certain epistemic configurations are unreachable:

```clojure
[Refute ExecutorCannotDominateExplorer
    :forall [W]
    :impossible [
        [dominates Executor Explorer]
    ]
    :method model-check
]
```

---

## Part 6: Observables and Measurement

### Semantic Observables (Meta-Theoretic)

Derived from the epistemic profile --- no measurement needed:

```clojure
[Observable DiscoveryPower :kind discovery-strength]
[Measure :observable DiscoveryPower :world Explorer]
;; -> [MEASURE] DiscoveryPower(Explorer) = complete
```

### Empirical Observables (Operational)

Actual runtime measurements. Require explicit `:value`:

```clojure
[Observable SearchTime :kind search-cost]
[Measure :observable SearchTime :world Explorer :value 4200ms]
```

Semantic observables cannot have explicit `:value` overrides --- the system derives the value from the epistemic profile, preventing wishful thinking.

---

## Part 7: Families, Pipelines, and Emit

### Families

A universe family groups worlds that share invariants:

```clojure
[Family GoldenPipeline
    :worlds [Explorer Certifier Executor]
    :invariants [Soundness]
]
```

### Pipelines

A pipeline is a sequence of steps through worlds, validated for epistemic feasibility:

```clojure
[Pipeline GoldenPath
    [Step discover     :action Discover    :world Explorer]
    [Step transport    :action Tunnel      :world Explorer    :target Certifier]
    [Step verify       :action Verify      :world Certifier]
    [Step compile      :action CoarseGrain :world Certifier   :target Executor]
    [Step measure      :action Measure     :world Executor]
]
```

Steps support `:class` for theorem-class-specific epistemic validation.

### Emit (The Epistemic Receipt)

Push an Omega term through a Metacosm pipeline, normalizing it via the theory's rewrite rules and wrapping the result in the pipeline's epistemic journey:

```clojure
[Emit ThreeTimesThree
    :term [mult [s [s [s z]]] [s [s [s z]]]]
    :theory PeanoArithmetic
    :pipeline GoldenPath
    :format epistemic-receipt    ;; or :format term for just the result
]
```

The receipt includes: payload (input/output), journey (each step with distance and loss), invariants (preserved/broken), and cost (interactions, term sizes).

---

## Part 8: Embeddings

Metacosm's three layers embed conservatively:

```
Omega ↪ Hyperion ↪ Metacosm
```

Each embedding is **conservative**, **definable-fragment**, **strict-extension**, and **non-perturbing**. These are registered as built-in embeddings and verified by the metatheory engine.

World-to-world embeddings check epistemic dominance: a conservative embedding from A to B requires that B dominates A.

---

## Part 9: The Three Modes

Metacosm operates in three modes depending on what blocks you use:

| Mode | Blocks | Behavior |
|------|--------|----------|
| **Omega** | `Theory`, `Proofs` | Routes through Hyperion to Apeiron |
| **Hyperion** | `Category`, `Substrate`, `Universe`, `Functor` | Handled by Hyperion |
| **Cosmology** | `World`, `Transition`, `Pipeline`, `Emit`, `Law`, ... | Full epistemic machinery |

These modes coexist in a single file. The routing is automatic.

---

## Robustness & Testing

Metacosm is backed by 120+ tests designed to catch adversarial edge cases:

- **Ouroboros Topologies**: Prevents infinite loops in cyclic conservative extensions
- **Ship of Theseus Leaks**: Catches family invariants broken mid-pipeline and falsely re-asserted
- **Categorical Contradictions**: Rejects unfaithful quotients and backwards embeddings
- **Physics Defiers**: Prevents domain-specific classes from exceeding their substrate's theoretical maximum capabilities
- **Round-Trip Paradoxes**: Detects faithfulness claims on transitions tainted by lossy compression
- **Delusional Auditors**: Substrate physics audits catch impossible epistemic declarations
- **Layer Embedding**: Full three-layer integration tests proving Omega theories survive through Hyperion substrates and Metacosm worlds unperturbed

## The Flagship Example

`examples/golden-path.mcm` ties the entire stack together in a single file:

- An **Omega theory** (Peano arithmetic with rewrite rules, verified by `assert-eq`)
- **Hyperion infrastructure** (STLC category, three substrates, two functors)
- **Three worlds** with full epistemic profiles and theorem-class sensitivity
- **Transitions** with functorial semantics, transport modes, and composition algebra
- **15 observables** (11 semantic, 4 empirical) with **26 measurements**
- **Assertions**, **world audits**, **cross-world lemma transport**
- **Conservative embedding verification** (Omega → Metacosm full stack)
- **Two materialized pipelines** (generic + equational class-specific)
- **Epistemic promotion** (dominance → licensed transition)
- **Tactic-proved laws** (reflexivity, transitivity)
- **Impossibility refutations**
- **Two cosmological emits** including the full Epistemic Receipt for `3 * 3 = 9`

## Building and Testing

```bash
cd metacosm
cargo build
cargo test
cargo run -- check examples/golden-path.mcm -v
```

## Architecture

```
metacosm/
  src/
    session.rs         Three-mode routing, world/transition/assertion processing
    world.rs           WorldDef, FamilyDef, parsing
    epistemic.rs       The four-axis epistemic profile system
    transition.rs      TransitionDef, composition algebra, transport modes
    morphism.rs        WorldMorphism, FunctorRef, morphism properties
    inference.rs       Fixpoint epistemic constraint propagation
    assertion.rs       User-declared formal claims
    pipeline.rs        PipelineDef, step validation
    embedding.rs       Layer and world embeddings
    emit.rs            Cosmological Emit and Epistemic Receipt
    proof_engine.rs    LCF-style tactic engine for cosmological metatheory
    audit.rs           Substrate physics auditing
    law.rs             Cosmological laws (model-check, structural, proof)
    refute.rs          Impossibility proofs
    materialize.rs     Pipeline materialization and epistemic tracing
    metatheory.rs      Proof certificates for system properties
    theorem_class.rs   Named reasoning fragments (Equational, ResourceSensitive, ...)
    knowledge.rs       Semantic vs empirical knowledge species
    error.rs           Structured error types
    main.rs            CLI
  examples/
    golden-path.mcm                The flagship end-to-end demo
    cosmology-demo.mcm             Three-world pipeline demo
    proof-engine-demo.mcm          Tactic-proved laws demo
    stress-tests.mcm               Core adversarial tests (1-6)
    stress-tests-advanced.mcm      Advanced adversarial tests (7-10)
    stress-tests-boss.mcm          Boss-level adversarial tests (11-14)
    stress-tests-embedding.mcm     Three-layer integration test
    omega-mode.mcm                 Pure theory/proofs pass-through
    hyperion-mode.mcm              Category/substrate pass-through
  tests/
    integration.rs     120+ integration tests
```

Metacosm depends on [Hyperion](../hyperion/) for categories, substrates, and functors, which in turn depends on [Apeiron](../apeiron/) for term rewriting and proof checking.

## Design Philosophy

**Epistemic profiles are typed, not adjectives.** "Sound" is not a boolean. Verification is a product of soundness, completeness, and termination --- independent axes that can't be collapsed without losing information.

**Transportability is relational.** Whether a theorem survives a transition depends on the transition, not just the worlds. The same two worlds can be connected by a faithful tunnel or a lossy collapse.

**Inference respects declarations.** The constraint engine fills in defaults and flags contradictions. It never silently overwrites something you said.

**Honesty is non-negotiable.** Model-checking says "holds for N registered worlds." Tactic proofs say "holds for all admissible worlds." These are different claims and Metacosm never conflates them.

**Conservative extension all the way down.** Omega works inside Hyperion works inside Metacosm. No layer invalidates the one below. This isn't just a design goal --- the metatheory engine proves it.

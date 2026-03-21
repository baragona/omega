# From Zero to Infinity: A Tour of Omega, Apeiron, and Hyperion

**Three systems. One question: how deep does the rabbit hole go?**

Most proof assistants ship with a logic. Lean gives you dependent type theory. Coq gives you the Calculus of Inductive Constructions. Isabelle gives you classical higher-order logic. You learn their rules. You play their game.

Omega ships with *nothing*. You bring your own logic. And by the time you reach Hyperion, you'll be bringing your own *physics* too.

---

## Act I: Omega --- Build Your Own Logic

### Your first theory in 30 seconds

In Lean, `Nat` is built in. In Omega, you build it:

```lisp
(theory Peano
  (sort Nat)
  (constructor z : Nat)
  (constructor s : (-> Nat Nat))
  (constructor add : (-> Nat Nat Nat))

  (judgment (eq ?a ?b) :where a : Nat b : Nat)
  (rule eq-refl :premises () :conclusion (eq ?a ?a))

  (rewrite add-z (add z ?n) ?n)
  (rewrite add-s (add (s ?n) ?m) (s (add ?n ?m))))
```

That's it. You've defined natural numbers, addition, and a notion of equality. The `(rewrite ...)` rules tell the kernel how to compute --- `add(0, n)` steps to `n`, `add(S(n), m)` steps to `S(add(n, m))`.

Now prove 1 + 1 = 2:

```lisp
(proof one-plus-one
  :theory Peano
  :goal (eq (add (s z) (s z)) (s (s z)))
  :derivation (eq-refl))
```

The entire proof is `eq-refl`. The kernel normalizes both sides via your rewrite rules, sees they're identical, done. No tactics, no automation --- the computation *is* the proof.

**Compare to Lean:**
```lean
#eval 1 + 1  -- 2 (but Nat, +, and the kernel are all built in)
```

Omega's version is longer, but you *defined the entire number system yourself*. The kernel has zero opinions about what `Nat` is.

### Propositional logic: proofs as trees

Define conjunction, disjunction, implication. Add inference rules. Prove theorems:

```lisp
(theory PropLogic
  (sort Prop)
  (constructor and : (-> Prop Prop Prop))
  (constructor imp : (-> Prop Prop Prop))

  (judgment (proves ?P) :where P : Prop)

  (rule and-intro
    :premises ((proves ?A) (proves ?B))
    :conclusion (proves (and ?A ?B)))

  (rule imp-intro
    :premises ((proves ?B))
    :context ((0 (proves ?A)))        ;; assume A in the premise
    :conclusion (proves (imp ?A ?B))))
```

The `:context` annotation is key. `imp-intro` says: "to prove A implies B, assume A and derive B." Context extensions give you hypothetical reasoning --- the same mechanism Gentzen used in natural deduction, but as data you configure.

Prove A&B implies B&A:

```lisp
(proof and-comm
  :goal (proves (imp (and ?A ?B) (and ?B ?A)))
  :derivation
    (imp-intro
      (and-intro
        (and-elim-r (assumption))
        (and-elim-l (assumption)))))
```

That's a raw derivation tree --- explicit, verbose, trusted by the kernel. Every node names a rule and provides sub-proofs.

### Climbing the ladder: tactics, metatheorems, lemmas

Writing raw trees gets old. Omega has five layers, each making the next easier:

**Tactics** --- backward reasoning:
```lisp
(proof auto-swap
  :goal (proves (and q p))
  :assumptions ((proves p) (proves q))
  :tactics (auto 3))    ;; brute-force search finds it
```

**Metatheorems** --- prove properties *about* derivations:
```lisp
(meta-theorem and-comm-meta
  :forall ((D (proves (and ?A ?B))))
  :exists  ((D' (proves (and ?B ?A))))
  :proof (case-analysis D
    (case and-intro (D1 D2) (by-rule and-intro D2 D1))))
```

This says: "for ANY proof of A&B, I can mechanically transform it into a proof of B&A." The kernel checks exhaustiveness (every introduction rule is covered) and soundness (each case is valid).

**Reflection** --- turn a metatheorem into a one-step rule:
```lisp
(reflect and-comm-meta :as and-comm :theory PropLogic)
```

Now `and-comm` is a native inference rule. The 5-node proof tree becomes 1 node.

**Lemmas** --- the Cut rule:
```lisp
(lemma and-assoc
  :premises ((proves (and (and ?A ?B) ?C)))
  :conclusion (proves (and ?A (and ?B ?C)))
  :derivation ...)  ;; verified once, then and-assoc is a new rule
```

Prove once, discard the tree, keep the signature. This is Cut: the most powerful structural rule in logic, available as a command.

### The zoo: every logic you've heard of (and some you haven't)

Because logic is configuration, Omega hosts *all of them*:

| Logic | Key idea | Omega feature |
|:---|:---|:---|
| **Classical** | Add `axiom DNE : Not(Not(A)) -> A` | One extra rule |
| **Intuitionistic** | Don't add DNE | Default |
| **Linear** | Each hypothesis used exactly once | `(binder-behavior tensor :linear)` |
| **Affine** | Each hypothesis used at most once | `(context-mode affine)` |
| **Modal S5** | Box, diamond, necessity | Context extensions for worlds |
| **HoTT** | Paths, transport, J eliminator | Rewrite rules + dependent types |
| **ZFC** | Sets, membership, axiom of choice | 18 axioms, 39 proofs |

The topos example is the punchline. Define a 2-element Boolean algebra (true/false) and a 3-element Heyting algebra (true/unknown/false). *Same connectives. Same rules.* Run both:

```lisp
;; Boolean: Double negation elimination works
(proof bool-dne-true :goal (eq (b-not (b-not bt)) bt) :derivation (eq-refl))
;; --- classical!

;; Heyting: Double negation maps unknown to true, not back to unknown
(proof heyt-dne-unknown :goal (eq (h-not (h-not hu)) ht) :derivation (eq-refl))
;; --- not-not(u) = true, not u. DNE fails. Intuitionistic!
```

Change the truth values, change the logic. Omega doesn't care which you pick. As the file header says: "Meta-joke: We are defining Omega inside Omega."

### Computation as compilation

`omega kompile` extracts a verified theory into a Rust crate. Sorts become enums, constructors become variants, rewrite rules become `match` functions:

```bash
omega kompile examples/calc.omega --theory CalcTheory -o calc-crate/
```

The generated Rust code preserves the semantics you proved. A TCP state machine verified in Omega compiles to a Rust `step()` function with the same safety guarantees. You can even generate C via string ropes and beta reduction (HOAS: one lambda, two uses --- evaluate it for correctness, compile it for code).

---

## Act II: Apeiron --- Same Math, Different Engine

Omega's kernel is a hash-consed tree normalizer. Fast, correct, boring. What if you wanted a fundamentally different computational substrate?

Apeiron is a **logic compiler built on interaction nets**. Instead of rewriting trees, it reduces terms on a graph where computation happens by local node-pair rewrites. The math stays the same. The physics changes completely.

### The same arithmetic, different engine

Here's Peano arithmetic in Apeiron:

```lisp
[System Peano
  [@syntax [sort Nat] [op z] [op s] [op add] [op mul]]
  [@binding implicit]
  [@check rewriting]
]

[Theory PeanoRules :in Peano
  [@rule add-z [add z ?n]      ==> ?n]
  [@rule add-s [add [s ?n] ?m] ==> [s [add ?n ?m]]]
]

[Proofs Check :in PeanoRules
  [assert-eq one-plus-one [add [s z] [s z]] [s [s z]]]
]
```

Same four rules. Same proofs. Different engine. Omega normalizes via memoized hash-consed reduction. Apeiron normalizes via graph surgery on an interaction net with Dup/Erase nodes and a physics scheduler.

### Three layers of trust

Every Apeiron program separates concerns:

- **System** --- How do I speak? (syntax, binding strategy, evaluation strategy)
- **Theory** --- What do I believe? (axioms --- the trusted base)
- **Proofs** --- What do I know? (verified theorems --- sealed, read-only)

If you try to put `@rule` inside a `[Proofs]` block, Apeiron rejects it. Your axioms and theorems live in different worlds.

### Choose your physics

This is where Apeiron diverges from everything else. You choose your **binding mode** and **checking mode**:

| Binding | What it means | Example |
|:---|:---|:---|
| `implicit` | Alpha-equivalence via hashing | Most logics |
| `exposed` | De Bruijn indices in the graph | Compilers, VMs |
| `linear-explicit` | Every variable used exactly once | Linear logic |
| `nominal` | Names are meaningful, no alpha | Name-sensitive systems |

| Checking | What it means | Example |
|:---|:---|:---|
| `rewriting` | Pattern-match reduction | Term rewriting |
| `beta-reduction` | Native lambda calculus | Church encodings |
| `oracle` | Topological hashing = equality | Mathematics |
| `reversible` | Every rule auto-generates its inverse | Reversible computing |

Modes compose: `[@check rewriting beta-reduction]` gives you both.

**Compare to Dedukti/Lambdapi:** Dedukti also lets you define logics via rewrite rules, but it runs on a fixed OCaml term rewriter. Apeiron gives you a choice of computational substrate with fundamentally different operational characteristics (optimal sharing, stack safety at 100K depth, topological equality).

### AutoMorphisms: the universal translator

Define two systems with different binding/checking strategies and Apeiron auto-generates a compiler between them:

```lisp
[AutoMorphism Compile HighLevel LowLevel
  [Map plus add]
  [@strict true]
]
```

It detects binding mismatches (implicit -> de Bruijn) and auto-generates indexing. It detects checking mismatches (compute -> oracle) and enables normalize-before-send. The morphism IS the compiler.

---

## Act III: Hyperion --- The Laws of Physics Are Customizable Too

Apeiron let you define your own math, but it forced all that math to run on one engine: the interaction net. What if different math needs different physics?

Lambda calculus needs closures. Modal logic needs scope isolation. Tensor products need parallel composition. Not every mathematical structure can run on every computational substrate.

Hyperion makes the physics configurable too.

### Category + Substrate = Universe

```
 Category          Substrate            Universe
 (the math)    +   (the physics)    =   (the system)
```

A **Category** declares pure mathematical structure --- sorts, operations, higher structure (CCC for lambda, monoidal for tensors, PathType for HoTT, modal for necessity).

A **Substrate** declares computational physics --- what engine runs your terms, how resources are managed, how scoping works, what equality means.

A **Universe** binds them. Hyperion checks compatibility and compiles both into a working system.

### Lambda calculus on an interaction net

```
[Category CartesianClosed
  [Object Type]
  [Object Term]
  [Morphism app :domain [Term Term] :codomain Term]
  [Exponential lam :object Term]
  [Evaluator app]
]

[Substrate InteractionNet
  @engine interaction-graph
  @resource-mode optimal-sharing
  @barrier transparent
  @equality topological-hash
]

[Universe WeakLF :category CartesianClosed :substrate InteractionNet]

[Proofs Check :in SimpleLogic
  [assert-eq beta [app [lam x x] z] z]           ;; (lambda x.x) z = z
]
```

Hyperion automatically verifies that a CCC needs a lambda-capable engine (interaction graphs support it), generates the Apeiron system, and passes the theory through.

### What happens when math and physics disagree

Try running a CCC on a von Neumann machine:

```
[Substrate VonNeumann
  @engine von-neumann
  @resource-mode deep-copy
  @barrier transparent
  @equality rewrite-equivalence
]

[Universe BadIdea :category CartesianClosed :substrate VonNeumann]
;; ERROR: Exponential requires lambda-capable engine
;;        (von-neumann does not support lambda)
```

Hyperion refuses. Lambda abstraction needs closures; sequential hardware doesn't have them natively. The incompatibility is caught at compile time, not at runtime when your proofs mysteriously fail.

Or try linear resources with closures:

```
[Substrate LinearEngine
  @engine interaction-graph
  @resource-mode strictly-linear
  ...
]

[Universe AlsoBad :category CartesianClosed :substrate LinearEngine]
;; ERROR: StrictlyLinear + Exponential --- linear resources can't duplicate closures
```

The substrate constrains what math is *physically realizable*.

### HoTT for free: PathType auto-injection

Declare `[PathType]` in your category and Hyperion auto-injects the entire groupoid structure:

```
[Category PathSpace
  [Object Type]
  [Object Term]
  [Morphism app :domain [Term Term] :codomain Term]
  [Exponential lam :object Term]
  [Evaluator app]
  [PathType :refl refl :concat concat :inv inv :ap ap]
]

[Universe HoTTWorld :category PathSpace :substrate HomotopyEngine]

;; Empty theory body --- all path rules auto-injected!
[Theory PathAlgebra :in HoTTWorld]

[Proofs PathCheck :in PathAlgebra
  [assert-eq left-unit  [concat [refl a] p]     p]
  [assert-eq right-unit [concat p [refl a]]     p]
  [assert-eq assoc      [concat [concat p q] r] [concat p [concat q r]]]
  [assert-eq ap-refl    [ap f [refl a]]         [refl [app f a]]]
]
```

Compare to Omega's HoTT file, which manually defines refl, concat, inv, transport, ap, J, and 12 rewrite rules across 200 lines. Hyperion takes the category-theoretic structure seriously: if you declare PathType, the path algebra *comes with it*.

### The Eckmann-Hilton punchline: physics determines provability

This is Hyperion's killer demo. Define five independent algebraic laws for two binary operations (vertical composition `concat` and horizontal composition `hcomp`):

```
[@law interchange [hcomp [concat ?a ?b] [concat ?c ?d]]
              === [concat [hcomp ?a ?c] [hcomp ?b ?d]]]
[@law hcomp-left-id  [hcomp [refl base] ?p] === ?p]
[@law hcomp-right-id [hcomp ?p [refl base]] === ?p]
[@law concat-left-id  [concat [refl base] ?p] === ?p]
[@law concat-right-id [concat ?p [refl base]] === ?p]
```

On an `equality-saturation` substrate (e-graphs), the system **autonomously discovers** that vertical composition is commutative: `concat(a, b) = concat(b, a)`. Nobody told it. Nobody hinted. The e-graph saturated equivalence classes and found a ~7-step proof path using laws in both directions.

Now put the *exact same five laws* on a `rewrite-equivalence` substrate (directed rewriting):

```
[assert-neq gap [concat alpha beta] [hcomp alpha beta]]
;; PASSES --- can't prove coincidence. The terms are stuck.
```

Same laws. Different physics. Different provability. The substrate isn't decoration --- it's *load-bearing*.

### Cross-substrate transport: knowledge survives the jump

A Functor moves terms between substrates. The e-graph world discovers a theorem, normalizes it, then ships the result to a directed world for mechanical verification:

```
[Functor InsightTransport :from EGraphEngine :to DirectedEngine]
[Import result [InsightTransport compound]]
```

The directed world can verify the transported normal form but *cannot independently discover it*. Discovery flows one way: advanced physics -> serialization -> verification.

This is epistemic transport across computational realities.

### The infinite ascent: Meta-categories

Nothing stops you from defining a category whose objects are categories and whose morphisms are functors:

```
[Category MetaCat
  [Object Cat]
  [Morphism functor :domain [Cat Cat] :codomain Cat]
  [PathType :refl refl :concat concat :inv inv :ap ap]
]
```

PathType auto-injects at the meta-level too. Paths between functors are natural isomorphisms. Paths between paths are modifications. The framework frameworks itself, all the way up.

---

## Act IV: Metacosm --- Universes as First-Class Epistemic Objects

Hyperion lets you bind math to physics. But what happens when you have *multiple* worlds, each with different epistemic capabilities, and you need to move theorems between them?

A theorem discovered by an e-graph saturator can't be discovered by a directed rewriter. A proof verified by a decidable checker carries more weight than one checked heuristically. A compiled result loses the proof structure that made it trustworthy.

Metacosm makes these relationships formal.

### Three modes, one system

Metacosm is a strict superset. Omega and Hyperion appear inside it as conservative fragments:

- **Omega mode**: Theory/Proofs blocks pass straight through to Apeiron. Single world, no cosmology. Everything you wrote before still works.
- **Hyperion mode**: Category/Substrate/Universe blocks pass through to Hyperion. Static world families. Same semantics.
- **Cosmology mode**: Worlds with epistemic profiles, transitions between worlds, observables, pipelines. The new layer.

Nothing breaks. Everything extends.

### Worlds with typed epistemics

Every world declares what it can and can't do, not as vague adjectives but as typed products:

```
[World Explorer
    :category CartesianClosed
    :substrate EGraphSubstrate
    :epistemic [
        :discover complete
        :verify sound
        :canonicalize weak-nf
        :compress none
    ]
    :class-epistemic [
        [Equational :discover complete :verify decidable]
        [ResourceSensitive :discover none :verify heuristic]
    ]
    :admits [Split Tunnel]
]
```

Each axis is a product, not a scalar:

| Axis | Structure | Why not a single chain |
|:---|:---|:---|
| **Discovery** | `none < heuristic < semi-decidable < complete` | Clean single chain. No decomposition needed. |
| **Verification** | soundness x completeness x termination | A sound-and-complete semidecision procedure is different from a decidable total checker. |
| **Canonicality** | normalization x confluence x unique-nf | Confluent-but-not-normalizing and normalizing-but-not-confluent are incomparable. |
| **Compression** | mode + lossy + invertible | Codegen and lossless are qualitatively different, not ranked. |

The `:class-epistemic` block makes it precise: Explorer is excellent at equational discovery but can't do resource-sensitive reasoning at all. Same world, different epistemic profiles per theorem family.

Short syntax desugars into the full product:

```
:verify decidable
;; expands to: [:soundness sound :completeness complete :termination decidable]

:canonicalize confluent
;; expands to: [:normalization none :confluence yes :unique-normal-forms no]

:compress codegen
;; expands to: [:mode codegen :lossy yes :invertible no]
```

### Transitions: the algebra of world-crossing

When a theorem moves from one world to another, information changes. Metacosm tracks this with typed transitions:

```
[Transition DiscoverTunnel
    :kind Tunnel
    :from Explorer
    :to Certifier
    :transport [:mode witness :loss [PathStructure]]
    :preserves [Soundness]
    :breaks [PathStructure]
]
```

A **Tunnel** moves a theorem to a world where it wasn't discoverable but is verifiable. The e-graph world found it; the rewriting world checks it. Transport mode says *how much* survives: witness (the full proof), theorem-only (just the statement), lossy (some structure lost), conservative (everything transfers).

Metacosm validates transitions against epistemic profiles:
- Tunnel targets must be able to verify (soundness > none)
- Conservative extensions require the target to dominate the source
- Invariant conflicts (claiming to both preserve and break Soundness) are rejected

Transitions compose. If A→B preserves [Soundness, Normalization] and B→C preserves [Soundness], the composed A→C preserves only [Soundness] (intersection). Breaks accumulate (union). Loss accumulates. Transport modes compose (witness + lossy = lossy):

```
[Compose ExplorerToExecutor :transitions [DiscoverTunnel CertifyToExecute]]
;; => Explorer → Executor, preserves=[Soundness], transport=lossy
```

### Derived observables: inference, not annotation

Metacosm infers epistemic properties from substrate and category structure where possible:

- `equality-saturation` substrate → `confluence = true`
- `interaction-graph` engine → `discover >= semi-decidable`
- `abstract-machine` engine → `compress = codegen`
- `term-tree` + `rewrite-equivalence` → `normalization >= weak`

User-declared values are never overridden. The system fills in defaults. Use `:derive no` to suppress inference entirely.

### Semantic vs empirical: two species of knowledge

Not all knowledge is the same kind:

```
[Observable VerifySoundness :kind verification-soundness]   ;; semantic (meta-theoretic)
[Observable SearchTime :kind search-cost]                   ;; empirical (operational)
```

Semantic observables (soundness, confluence, invertibility) are extracted from the epistemic profile. They follow from the structure of the logic.

Empirical observables (proof size, search cost, runtime) must be supplied explicitly:

```
[Measure :observable SearchTime :world Explorer :value 3200ms]
```

You can't derive runtime from a type signature. Metacosm knows the difference.

### Conservative embedding: architecture as metatheory

The layering isn't just an implementation story. Metacosm formalizes and checks it:

```
[Embedding DemoLayerCheck
    :from Omega
    :to Hyperion
    :properties [conservative definable-fragment strict-extension non-perturbing]
]
```

Four properties, each mechanically checked:

- **Conservative**: source blocks pass through with unchanged semantics
- **Definable fragment**: every source block type is accepted by the target
- **Strict extension**: the target has block types the source doesn't
- **Non-perturbing**: adding target features doesn't alter source behavior

Three builtin embeddings are auto-registered: Omega→Hyperion, Hyperion→Metacosm, Omega→Metacosm. World-to-world embeddings check epistemic dominance.

### The flagship demo: theorem cosmology pipeline

Three worlds. Two transitions. One pipeline. The full story:

```
Explorer  ──Tunnel──>  Certifier  ──CoarseGrain──>  Executor
(e-graph)              (rewriting)                   (compiler)
discover=complete      verify=decidable              compress=codegen
```

Explorer discovers theorems by e-graph saturation. Certifier verifies them by decidable checking. Executor compiles them to operational code. Each step trades one kind of epistemic power for another.

Measure the pipeline:

```
[MEASURE] DiscoveryPower(Explorer) = complete
[MEASURE] DiscoveryPower(Explorer) = none [class=ResourceSensitive]
[MEASURE] VerificationClarity(Certifier) = sound+complete+decidable
[MEASURE] Confluence(Explorer) = true     ;; derived from equality-saturation
[MEASURE] SearchTime(Explorer) = 3200ms   ;; empirical
[MEASURE] EpistemicDistance(Explorer → Executor) = 7
[COMPOSE] ExplorerToExecutor = Explorer → Executor (signature=[lossy](dist=7))
```

The distance is provisional. The composition is algebraic. The derivation is sound. The separation between semantic and empirical is clean. And the whole thing runs on top of Hyperion and Omega without perturbing either.

### What's next: metatheorems

The names are in place. The next frontier is turning them into actual theorems:

- **Embedding conservativity**: prove, not just check, that Omega-mode blocks produce identical results in Metacosm
- **Composition laws**: associativity, identity, preservation-intersection as formal properties
- **Dominance monotonicity**: class-sensitive profiles respect the partial order
- **Derivation soundness**: each inference rule is justified by substrate semantics
- **Pipeline preservation**: if every transition preserves an invariant, the pipeline preserves it

That is where the system moves from "mathematically respectable" to "research-grade."

---

## The View from the Top

| System | What you configure | What's fixed |
|:---|:---|:---|
| **Lean/Coq** | Nothing | Logic, kernel, computation |
| **Omega** | Logic, rules, binding | The kernel (hash-consed tree normalizer) |
| **Apeiron** | Logic, rules, binding, checking strategy | The engine (interaction nets) |
| **Hyperion** | Logic, rules, binding, checking, engine, resources, scoping, equality | Nothing |

Omega asks: *what if logic were configuration?* Apeiron asks: *what if the evaluation strategy were too?* Hyperion asks: *what if the laws of physics governing computation were themselves data?*

The answer, at every level, is the same: you get more power by committing to less. The neutral tool beats the specialized one --- not by being generic, but by making the assumptions explicit and exchangeable.

The kernel is done. The rest is just writing files.

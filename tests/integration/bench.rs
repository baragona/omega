/// Benchmark: interned derivation checker performance.
use std::time::Instant;

use omega_core::derivation::{Context, Derivation};
use omega_core::expr::Expr;
use omega_core::interned_check::InternedTheory;
use omega_core::judgment::{ConstructorDecl, JudgmentForm, Rule, SortDecl};
use omega_core::theory::Theory;
use omega_driver::batch;
use omega_driver::session::Session;

// ---------------------------------------------------------------------------
// ZFC benchmark (moderate-size proofs, tests amortized caching)
// ---------------------------------------------------------------------------

#[test]
fn bench_zfc() {
    let path = "examples/zfc.omega";
    let iterations = 20;

    // Warm up
    let mut session = Session::new();
    let _ = batch::process_file_path(&mut session, path).unwrap();

    let start = Instant::now();
    for _ in 0..iterations {
        let mut session = Session::new();
        batch::process_file_path(&mut session, path).unwrap();
    }
    let total = start.elapsed();
    let avg = total / iterations;

    eprintln!("\n--- ZFC Benchmark ({} iterations) ---", iterations);
    eprintln!("  Interned checker (cached): {:?} avg", avg);
    eprintln!("---");
}

// ---------------------------------------------------------------------------
// Torture test: exponential sharing via doubling
// ---------------------------------------------------------------------------

fn make_torture_theory() -> Theory {
    let mut tb = Theory::builder("TortureArith");
    tb.add_sort(SortDecl::new("Nat"));
    tb.add_constructor(ConstructorDecl::new("z", Expr::sym("Nat")));
    tb.add_constructor(ConstructorDecl::new(
        "s",
        Expr::app(vec![Expr::sym("->"), Expr::sym("Nat"), Expr::sym("Nat")]),
    ));
    tb.add_constructor(ConstructorDecl::new(
        "add",
        Expr::app(vec![
            Expr::sym("->"),
            Expr::sym("Nat"),
            Expr::sym("Nat"),
            Expr::sym("Nat"),
        ]),
    ));
    tb.add_judgment(JudgmentForm::new(
        "eq",
        Expr::app(vec![Expr::sym("eq"), Expr::meta("a"), Expr::meta("b")]),
        vec![],
    ));
    tb.push_rule(Rule::new(
        "eq-refl",
        vec![],
        Expr::app(vec![Expr::sym("eq"), Expr::meta("a"), Expr::meta("a")]),
    ));
    tb.build().unwrap()
}

/// Compute tree node count from depth (avoids O(2^k) traversal).
fn tree_node_count(depth: u32) -> u64 {
    // nodes(0) = 1 (z), nodes(k) = 1(App) + 1(Sym "add") + 2*nodes(k-1) = 2 + 2*nodes(k-1)
    // Closed form: nodes(k) = 3 * 2^k - 2
    3 * (1u64 << depth) - 2
}

fn format_count(n: u64) -> String {
    if n >= 1_000_000_000_000 {
        format!("{:.1}T", n as f64 / 1e12)
    } else if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1e3)
    } else {
        format!("{}", n)
    }
}

#[test]
fn torture_interned() {
    let theory = make_torture_theory();
    let eq_refl = Derivation::RuleApp {
        rule_name: "eq-refl".into(),
        premises: vec![],
    };

    eprintln!("\n=== Torture Test: Exponential Sharing via Doubling ===\n");
    eprintln!(
        "{:<8} {:<14} {:<14} {:<16}",
        "Depth", "Tree nodes*", "Arena nodes", "Check time"
    );

    let mut cached = InternedTheory::new(&theory);

    for &depth in &[25u32, 50, 100, 500, 1_000, 10_000, 100_000] {
        let arena = cached.arena_mut();
        let z = arena.sym("z");
        let add = arena.sym("add");
        let eq_sym = arena.sym("eq");

        // Build doubled term: O(depth) work, O(depth) arena nodes
        let mut term = z;
        for _ in 0..depth {
            term = arena.app(vec![add, term, term]);
        }
        let h_goal = arena.app(vec![eq_sym, term, term]);
        let arena_size = arena.len();

        // Check eq-refl: O(1) — both sides of (eq X X) are the same handle
        let start = Instant::now();
        cached.check_h(h_goal, &eq_refl, &[]).unwrap();
        let time = start.elapsed();

        let tree_would_be = if depth <= 63 {
            format_count(tree_node_count(depth))
        } else {
            format!("2^{}", depth + 1)
        };

        eprintln!(
            "{:<8} {:<14} {:<14} {:<16?}",
            depth, tree_would_be, arena_size, time
        );
    }
    eprintln!("(* tree nodes = size if term were fully expanded as a tree)\n");
    eprintln!("=== End Torture Test ===");
}

// ---------------------------------------------------------------------------
// Reduction benchmark: prove n + n = 2n by reflexivity via rewrite rules
// ---------------------------------------------------------------------------

fn make_peano_compute_theory() -> Theory {
    let mut tb = Theory::builder("PeanoCompute");
    tb.add_sort(SortDecl::new("Nat"));
    tb.add_constructor(ConstructorDecl::new("z", Expr::sym("Nat")));
    tb.add_constructor(ConstructorDecl::new(
        "s",
        Expr::app(vec![Expr::sym("->"), Expr::sym("Nat"), Expr::sym("Nat")]),
    ));
    tb.add_constructor(ConstructorDecl::new(
        "add",
        Expr::app(vec![Expr::sym("->"), Expr::sym("Nat"), Expr::sym("Nat"), Expr::sym("Nat")]),
    ));
    tb.add_judgment(JudgmentForm::new(
        "eq",
        Expr::app(vec![Expr::sym("eq"), Expr::meta("a"), Expr::meta("b")]),
        vec![],
    ));
    tb.push_rule(Rule::new(
        "eq-refl",
        vec![],
        Expr::app(vec![Expr::sym("eq"), Expr::meta("a"), Expr::meta("a")]),
    ));
    tb.add_rewrite(omega_core::judgment::RewriteRule::new(
        "add-z",
        Expr::app(vec![Expr::sym("add"), Expr::sym("z"), Expr::meta("n")]),
        Expr::meta("n"),
    ));
    tb.add_rewrite(omega_core::judgment::RewriteRule::new(
        "add-s",
        Expr::app(vec![Expr::sym("add"), Expr::app(vec![Expr::sym("s"), Expr::meta("n")]), Expr::meta("m")]),
        Expr::app(vec![Expr::sym("s"), Expr::app(vec![Expr::sym("add"), Expr::meta("n"), Expr::meta("m")])]),
    ));
    tb.build().unwrap()
}

/// Build the Peano numeral for n: z, (s z), (s (s z)), ...
fn peano_num(n: usize) -> Expr {
    let mut term = Expr::sym("z");
    for _ in 0..n {
        term = Expr::app(vec![Expr::sym("s"), term]);
    }
    term
}

#[test]
fn bench_reduction() {
    let theory = make_peano_compute_theory();
    let eq_refl = Derivation::RuleApp {
        rule_name: "eq-refl".into(),
        premises: vec![],
    };
    let empty_ctx = Context::new();

    eprintln!("\n=== Reduction Benchmark: n + n = 2n via eq-refl ===\n");
    eprintln!("{:<8} {:<16}", "n", "Interned check");

    for &n in &[5, 10, 20, 50, 100] {
        let lhs = Expr::app(vec![Expr::sym("add"), peano_num(n), peano_num(n)]);
        let rhs = peano_num(n * 2);
        let goal = Expr::app(vec![Expr::sym("eq"), lhs, rhs]);

        let mut cached = InternedTheory::new(&theory);
        let start = Instant::now();
        cached.check(&goal, &eq_refl, &empty_ctx).unwrap();
        let interned_time = start.elapsed();

        eprintln!("{:<8} {:<16?}", n, interned_time);
    }
    eprintln!("\n=== End Reduction Benchmark ===");
}

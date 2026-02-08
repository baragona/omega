/// Benchmark comparison: tree-based vs interned derivation checker.
///
/// Runs both checkers on the full ZFC example and compares timings.
/// This is a #[test] that prints results, not a criterion benchmark,
/// so it always passes but shows relative performance.
use std::time::Instant;

use omega_driver::batch;
use omega_driver::session::Session;

#[test]
fn bench_zfc_interned_vs_tree() {
    let path = "examples/zfc.omega";
    let iterations = 20;

    // Warm up
    let mut session = Session::new();
    let _ = batch::process_file_path(&mut session, path).unwrap();

    // Run with interned checker (cached per-theory)
    let start = Instant::now();
    for _ in 0..iterations {
        let mut session = Session::new();
        session.kernel.use_interned = true;
        batch::process_file_path(&mut session, path).unwrap();
    }
    let interned_total = start.elapsed();

    // Run with tree-based checker
    let start = Instant::now();
    for _ in 0..iterations {
        let mut session = Session::new();
        session.kernel.use_interned = false;
        batch::process_file_path(&mut session, path).unwrap();
    }
    let tree_total = start.elapsed();

    let interned_avg = interned_total / iterations;
    let tree_avg = tree_total / iterations;

    eprintln!("\n--- ZFC Benchmark ({} iterations) ---", iterations);
    eprintln!("  Interned checker (cached): {:?} avg", interned_avg);
    eprintln!("  Tree checker:              {:?} avg", tree_avg);
    if tree_avg > interned_avg {
        let speedup = tree_avg.as_nanos() as f64 / interned_avg.as_nanos() as f64;
        eprintln!("  Speedup: {:.2}x faster with interning", speedup);
    } else {
        let ratio = interned_avg.as_nanos() as f64 / tree_avg.as_nanos() as f64;
        eprintln!("  Ratio:   {:.2}x (tree faster — expected for small proofs)", ratio);
    }
    eprintln!("---");
}

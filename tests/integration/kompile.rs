/// Integration tests for `omega kompile` — theory-to-Rust compiler.
use omega_driver::batch;
use omega_driver::codegen;
use omega_driver::session::Session;

fn kompile_theory(file: &str, theory: &str) -> std::collections::HashMap<String, String> {
    let mut session = Session::new();
    batch::process_file_path(&mut session, file).unwrap();

    let krate = codegen::analyze::analyze(session.kernel.get_theory(theory).unwrap());
    codegen::emit::emit_crate(&krate)
}

#[test]
fn kompile_tcp_state() {
    let files = kompile_theory("examples/tcp-state.omega", "TcpState");

    // Expected files
    assert!(files.contains_key("Cargo.toml"));
    assert!(files.contains_key("src/lib.rs"));
    assert!(files.contains_key("src/omega_generated.rs"));

    let gen = &files["src/omega_generated.rs"];
    assert!(gen.contains("pub enum State"));
    assert!(gen.contains("Closed"));
    assert!(gen.contains("Listen"));
    assert!(gen.contains("SynRecvd"));
    assert!(gen.contains("Established"));
    assert!(gen.contains("FinWait"));
    assert!(gen.contains("pub enum Event"));
    assert!(gen.contains("EvListen"));
    assert!(gen.contains("EvSyn"));
    assert!(gen.contains("pub enum Bool"));

    assert!(gen.contains("pub fn step("));
    assert!(gen.contains("pub fn can_send("));
    assert!(gen.contains("pub fn is_open("));
    assert!(gen.contains("State::Closed, Event::EvListen) => State::Listen"));
    assert!(gen.contains("State::Established => Bool::True"));

    // No box_patterns needed (no recursive types)
    let lib = &files["src/lib.rs"];
    assert!(!lib.contains("box_patterns"));
    // Re-export
    assert!(lib.contains("pub use omega_generated::*;"));
}

#[test]
fn kompile_peano_compute() {
    let files = kompile_theory("examples/peano-compute.omega", "PeanoCompute");

    let gen = &files["src/omega_generated.rs"];
    assert!(gen.contains("pub enum Nat"));
    assert!(gen.contains("Z,"));
    assert!(gen.contains("S(Box<Nat>)"));

    assert!(gen.contains("pub fn add("));
    assert!(gen.contains("pub fn mul("));
    // box patterns for recursive Nat
    assert!(gen.contains("Nat::S(box n)"));
    // Box::new in RHS
    assert!(gen.contains("Box::new(add(n, m))"));

    // box_patterns feature flag present
    let lib = &files["src/lib.rs"];
    assert!(lib.contains("box_patterns"));
}

#[test]
fn kompile_rate_limiter() {
    let files = kompile_theory("examples/rate-limiter.omega", "RateLimiter");

    let gen = &files["src/omega_generated.rs"];
    assert!(gen.contains("pub enum Nat"));
    assert!(gen.contains("pub enum Bucket"));
    assert!(gen.contains("pub enum Decision"));
    assert!(gen.contains("Accept"));
    assert!(gen.contains("Reject"));

    assert!(gen.contains("pub fn request("));
    assert!(gen.contains("pub fn refill("));
    assert!(gen.contains("pub fn tokens("));
    assert!(gen.contains("Decision::Reject"));
    assert!(gen.contains("Decision::Accept"));
}

#[test]
fn kompile_tcp_server_effects() {
    let files = kompile_theory("examples/tcp-server.omega", "TcpServer");

    // Expected files
    assert!(files.contains_key("src/omega_generated.rs"));

    let gen = &files["src/omega_generated.rs"];
    // Pure enums present
    assert!(gen.contains("pub enum State"));
    assert!(gen.contains("pub enum Event"));
    assert!(gen.contains("pub enum Bool"));
    // No Effect enum — it became a trait
    assert!(!gen.contains("pub enum Effect"));
    // Trait with &mut self and derived param names
    assert!(gen.contains("pub trait TcpServerEffects"));
    assert!(gen.contains("fn eff_bind_port(&mut self)"));
    assert!(gen.contains("fn eff_send_data(&mut self, state: State)"));
    assert!(gen.contains("fn eff_log(&mut self, state: State, event: Event, state1: State)"));

    // Pure functions still work
    assert!(gen.contains("pub fn step("));
    assert!(gen.contains("pub fn can_send("));
    assert!(gen.contains("pub fn is_open("));
    // Effectful function: takes effects param, no return type
    assert!(gen.contains("pub fn on_event(effects: &mut impl TcpServerEffects"));
    // Method calls in match arms
    assert!(gen.contains("effects.eff_bind_port()"));
    assert!(gen.contains("effects.eff_send_syn_ack()"));
    assert!(gen.contains("effects.eff_send_data(State::Established)"));
    assert!(gen.contains("effects.eff_log(State::SynRecvd, Event::EvAck, State::Established)"));
    assert!(gen.contains("effects.eff_log(State::FinWait, Event::EvAck, State::Closed)"));
    // No -> Effect return type on effectful function
    assert!(!gen.contains("-> Effect"));
}

#[test]
fn kompile_skips_prop_sort() {
    let files = kompile_theory("examples/tcp-state.omega", "TcpState");
    let gen = &files["src/omega_generated.rs"];
    // Prop is a verification-only sort — should not appear as an enum
    assert!(!gen.contains("pub enum Prop"));
}

#[test]
fn kompile_disk_write() {
    let mut session = Session::new();
    batch::process_file_path(&mut session, "examples/tcp-state.omega").unwrap();

    let dir = std::env::temp_dir().join("omega_kompile_test");
    let _ = std::fs::remove_dir_all(&dir);

    let n = codegen::kompile(&session, "TcpState", dir.to_str().unwrap()).unwrap();
    assert!(n >= 3); // Cargo.toml, lib.rs, omega_generated.rs

    assert!(dir.join("Cargo.toml").exists());
    assert!(dir.join("src/lib.rs").exists());
    assert!(dir.join("src/omega_generated.rs").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn kompile_calc() {
    let files = kompile_theory("examples/calc.omega", "Calc");

    let gen = &files["src/omega_generated.rs"];
    // Enums
    assert!(gen.contains("pub enum Nat"));
    assert!(gen.contains("S(Box<Nat>)"));
    assert!(gen.contains("pub enum Bool"));
    assert!(gen.contains("pub enum Expr"));
    assert!(gen.contains("Lit(Nat)"));
    assert!(gen.contains("AddExpr(Box<Expr>, Box<Expr>)"));
    assert!(gen.contains("FactExpr(Box<Expr>)"));
    assert!(gen.contains("IfExpr(Box<Expr>, Box<Expr>, Box<Expr>)"));
    // No Prop or Effect enums
    assert!(!gen.contains("pub enum Prop"));
    assert!(!gen.contains("pub enum Effect"));
    // Trait
    assert!(gen.contains("pub trait CalcEffects"));
    assert!(gen.contains("fn eff_print(&mut self, nat: Nat)"));
    // Functions
    assert!(gen.contains("pub fn add("));
    assert!(gen.contains("pub fn mul("));
    assert!(gen.contains("pub fn sub("));
    assert!(gen.contains("pub fn pow("));
    assert!(gen.contains("pub fn fact("));
    assert!(gen.contains("pub fn eval("));
    assert!(gen.contains("pub fn run("));
    assert!(gen.contains("pub fn lt("));
    assert!(gen.contains("pub fn min("));
    assert!(gen.contains("pub fn max("));
    // Effectful run function
    assert!(gen.contains("pub fn run(effects: &mut impl CalcEffects"));
    // box_patterns needed
    let lib = &files["src/lib.rs"];
    assert!(lib.contains("box_patterns"));
}

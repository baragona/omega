/// TCP Server with Effects — Proof of Life
///
/// This program demonstrates the full Omega → Rust pipeline:
///   1. State machine defined as rewrite rules in Omega    (tcp-server.omega)
///   2. Safety properties verified with proofs              (omega check)
///   3. Compiled to Rust automatically                      (omega kompile)
///   4. USER implements the TcpServerEffects trait           (this file!)
///
/// The step() and on_event() functions were never written by a human.
/// They were generated from a verified specification.
///
/// The TcpServerEffects trait is the boundary between verified logic
/// and real I/O. Swap in tokio::net, std::fs, or any runtime you want.

use tcp_server_runner::*;

/// A simple logger that implements the generated Effect trait.
/// In a real app, this would wrap tokio::net::TcpListener, std::fs, etc.
struct LoggingServer {
    log: Vec<String>,
}

impl LoggingServer {
    fn new() -> Self {
        Self { log: Vec::new() }
    }
}

impl TcpServerEffects for LoggingServer {
    fn eff_bind_port(&mut self) {
        let msg = "  [EFFECT] Binding to port 8080...".to_string();
        println!("{}", msg);
        self.log.push(msg);
    }

    fn eff_send_syn_ack(&mut self) {
        let msg = "  [EFFECT] Sending SYN-ACK...".to_string();
        println!("{}", msg);
        self.log.push(msg);
    }

    fn eff_send_data(&mut self, state: State) {
        let msg = format!("  [EFFECT] Sending data (state: {:?})...", state);
        println!("{}", msg);
        self.log.push(msg);
    }

    fn eff_send_fin(&mut self) {
        let msg = "  [EFFECT] Sending FIN...".to_string();
        println!("{}", msg);
        self.log.push(msg);
    }

    fn eff_send_rst(&mut self) {
        let msg = "  [EFFECT] Sending RST...".to_string();
        println!("{}", msg);
        self.log.push(msg);
    }

    fn eff_log(&mut self, from: State, event: Event, to: State) {
        let msg = format!("  [EFFECT] Log: {:?} --{:?}--> {:?}", from, event, to);
        println!("{}", msg);
        self.log.push(msg);
    }
}

fn main() {
    println!("=== Omega TCP Server — Effects Demo ===\n");

    // ── Scenario 1: Full lifecycle with effect dispatch ──
    println!("--- Scenario 1: Handshake + data + close (with effects) ---");
    let mut server = LoggingServer::new();
    let mut state = State::Closed;

    let events = [
        ("passive open",  Event::EvListen),
        ("SYN received",  Event::EvSyn),
        ("ACK received",  Event::EvAck),
        ("data transfer", Event::EvData),
        ("data transfer", Event::EvData),
        ("FIN received",  Event::EvFin),
        ("ACK (close)",   Event::EvAck),
    ];

    for (label, event) in &events {
        print!("  {:?}", state);
        // Dispatch the effect for this transition
        on_event(&mut server, state.clone(), event.clone());
        // Apply the pure state transition
        state = step(state, event.clone());
        println!("  --> {:?}  ({})", state, label);
    }
    println!("  Final: {:?}\n", state);
    assert_eq!(state, State::Closed);

    // ── Scenario 2: RST with effects ──
    println!("--- Scenario 2: RST kills connection (with effects) ---");
    state = State::Closed;
    on_event(&mut server, state.clone(), Event::EvListen);
    state = step(state, Event::EvListen);
    on_event(&mut server, state.clone(), Event::EvSyn);
    state = step(state, Event::EvSyn);
    on_event(&mut server, state.clone(), Event::EvAck);
    state = step(state, Event::EvAck);
    println!("  Established: {:?}", state);
    assert_eq!(state, State::Established);

    on_event(&mut server, state.clone(), Event::EvRst);
    state = step(state, Event::EvRst);
    println!("  After RST:   {:?}\n", state);
    assert_eq!(state, State::Closed);

    // ── Summary ──
    println!("--- Effect log ({} entries) ---", server.log.len());
    for entry in &server.log {
        println!("{}", entry);
    }

    println!("\n=== All assertions passed. ===");
    println!("=== Pure logic: generated from verified Omega theory. ===");
    println!("=== Effects: user-implemented trait — plug in any runtime. ===");
}

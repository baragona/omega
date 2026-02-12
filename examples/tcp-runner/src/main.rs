/// TCP State Machine Runner
///
/// This program executes logic that was:
///   1. Defined as rewrite rules in Omega          (tcp-state.omega)
///   2. Verified with 10 safety proofs              (omega check)
///   3. Compiled to Rust automatically              (omega kompile)
///
/// The step() function below was never written by a human.
/// It was generated from a verified specification.

use tcp_runner::*;

fn main() {
    println!("=== Omega TCP State Machine — Proof of Life ===\n");

    // ── Scenario 1: Full TCP handshake + data + graceful close ──
    println!("--- Scenario 1: Normal lifecycle ---");
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
        let can = can_send(state.clone());
        let open = is_open(state.clone());
        print!("  {:?}", state);
        print!("  [can_send={:?}, is_open={:?}]", can, open);
        state = step(state, event.clone());
        println!("  --{}--> {:?}", label, state);
    }
    println!("  Final: {:?} (should be Closed)\n", state);
    assert_eq!(state, State::Closed);

    // ── Scenario 2: RST kills an established connection ──
    println!("--- Scenario 2: RST from Established ---");
    let mut state = State::Closed;
    state = step(state, Event::EvListen);
    state = step(state, Event::EvSyn);
    state = step(state, Event::EvAck);
    println!("  Established: {:?}", state);
    assert_eq!(state, State::Established);

    state = step(state, Event::EvRst);
    println!("  After RST:   {:?} (should be Closed)", state);
    assert_eq!(state, State::Closed);

    // ── Scenario 3: Guard predicates ──
    println!("\n--- Scenario 3: Guard checks ---");
    let states = [
        State::Closed,
        State::Listen,
        State::SynRecvd,
        State::Established,
        State::FinWait,
    ];
    for s in &states {
        let can = can_send(s.clone());
        let open = is_open(s.clone());
        println!("  {:?}: can_send={:?}, is_open={:?}", s, can, open);
    }
    assert_eq!(can_send(State::Established), Bool::True);
    assert_eq!(can_send(State::Closed), Bool::False);
    assert_eq!(is_open(State::Established), Bool::True);
    assert_eq!(is_open(State::Closed), Bool::False);

    println!("\n=== All assertions passed. ===");
    println!("=== This logic was defined in Omega, verified with proofs, ===");
    println!("=== and compiled to Rust. No human wrote step(). ===");
}

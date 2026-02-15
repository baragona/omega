use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: apeiron <file.ap>");
        process::exit(1);
    }

    let filename = &args[1];
    let source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", filename, e);
            process::exit(1);
        }
    };

    let sexps = match apeiron::parser::parse(&source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    let mut session = apeiron::system::Session::new();
    let mut had_errors = false;

    for sexp in &sexps {
        match session.process(sexp) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error: {}", e);
                had_errors = true;
            }
        }
    }

    // Print all output
    for line in &session.output {
        println!("{}", line);
    }

    if had_errors {
        process::exit(1);
    }

    println!("\n--- Arena Stats ---");
    println!("Nodes spawned: {}", session.arena.stats.nodes_spawned);
    println!("Nodes freed:   {}", session.arena.stats.nodes_freed);
    println!("Interactions:  {}", session.arena.stats.interactions);
    println!("Live nodes:    {}", session.arena.live_count());
}

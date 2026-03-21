use std::env;
use std::fs;
use std::process;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  metacosm check <file.mcm> [-v] [--no-prelude] [--json]");
        process::exit(1);
    }

    let subcommand = &args[1];
    match subcommand.as_str() {
        "check" => cmd_check(&args[2..]),
        _ => {
            eprintln!("Unknown subcommand: {}", subcommand);
            eprintln!("Usage:");
            eprintln!("  metacosm check <file.mcm> [-v] [--no-prelude] [--json]");
            process::exit(1);
        }
    }
}

fn cmd_check(args: &[String]) {
    let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");
    let no_prelude = args.iter().any(|a| a == "--no-prelude");
    let json_mode = args.iter().any(|a| a == "--json");

    let start = Instant::now();

    let filename = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .unwrap_or_else(|| {
            eprintln!("Usage: metacosm check <file.mcm> [-v] [--no-prelude] [--json]");
            process::exit(1);
        });

    let source = fs::read_to_string(filename).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {}", filename, e);
        process::exit(1);
    });

    let sexps = apeiron::parser::parse(&source).unwrap_or_else(|e| {
        eprintln!("Parse error: {}", e);
        process::exit(1);
    });

    let mut session = if no_prelude {
        metacosm::session::MetacosmSession::new()
    } else {
        match metacosm::session::MetacosmSession::with_prelude() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: prelude loading failed: {}", e);
                metacosm::session::MetacosmSession::new()
            }
        }
    };

    let mut had_errors = false;

    for sexp in &sexps {
        match session.process(sexp) {
            Ok(()) => {}
            Err(e) => {
                if !json_mode {
                    eprintln!("Error: {}", e);
                }
                had_errors = true;
            }
        }
    }

    if json_mode {
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let output = session.json_output(had_errors, elapsed);
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        for line in &session.output {
            println!("{}", line);
        }

        if verbose {
            println!("\n--- Metacosm Stats ---");
            println!("Worlds:       {}", session.worlds.len());
            println!("Transitions:  {}", session.transitions.len());
            println!("Observables:  {}", session.observables.len());
            println!("Families:     {}", session.families.len());
            println!("Pipelines:    {}", session.pipelines.len());
            println!("Measurements: {}", session.measurements.len());
            println!("\n--- Hyperion Stats ---");
            println!("Categories:   {}", session.hyperion.categories.len());
            println!("Substrates:   {}", session.hyperion.substrates.len());
            println!("Universes:    {}", session.hyperion.universes.len());
            println!("Functors:     {}", session.hyperion.functors.len());
        }
    }

    if had_errors {
        process::exit(1);
    }
}

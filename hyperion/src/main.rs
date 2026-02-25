use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  hyperion check <file.hyp> [-v] [--no-prelude] [--skip-laws]");
        eprintln!("  hyperion kompile <file.hyp> --theory <name> -o <output_dir/>");
        process::exit(1);
    }

    let subcommand = &args[1];
    match subcommand.as_str() {
        "check" => cmd_check(&args[2..]),
        "kompile" => cmd_kompile(&args[2..]),
        _ => {
            eprintln!("Unknown subcommand: {}", subcommand);
            eprintln!("Usage:");
            eprintln!("  hyperion check <file.hyp> [-v] [--no-prelude] [--skip-laws]");
            eprintln!("  hyperion kompile <file.hyp> --theory <name> -o <output_dir/>");
            process::exit(1);
        }
    }
}

fn cmd_check(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: hyperion check <file.hyp> [-v] [--no-prelude] [--skip-laws]");
        process::exit(1);
    }

    let filename = &args[0];
    let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");
    let no_prelude = args.iter().any(|a| a == "--no-prelude");
    let skip_laws = args.iter().any(|a| a == "--skip-laws");

    let source = fs::read_to_string(filename).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {}", filename, e);
        process::exit(1);
    });

    let sexps = apeiron::parser::parse(&source).unwrap_or_else(|e| {
        eprintln!("Parse error: {}", e);
        process::exit(1);
    });

    let mut session = if no_prelude {
        hyperion::session::HyperionSession::new()
    } else {
        match hyperion::session::HyperionSession::with_prelude() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: prelude loading failed: {}", e);
                hyperion::session::HyperionSession::new()
            }
        }
    };

    if skip_laws {
        session.skip_laws = true;
    }

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

    // Print output
    for line in &session.output {
        println!("{}", line);
    }

    if verbose {
        println!("\n--- Hyperion Stats ---");
        println!("Categories:   {}", session.categories.len());
        println!("Substrates:   {}", session.substrates.len());
        println!("Universes:    {}", session.universes.len());
        println!("Functors:     {}", session.functors.len());
        println!("NatTrans:     {}", session.nat_trans.len());
        println!("Adjunctions:  {}", session.adjunctions.len());
        println!("VN Theories:  {}", session.vn_theories.len());
        println!("\n--- Apeiron Stats ---");
        println!(
            "Nodes spawned: {}",
            session.apeiron.arena.stats.nodes_spawned
        );
        println!("Nodes freed:   {}", session.apeiron.arena.stats.nodes_freed);
        println!(
            "Interactions:  {}",
            session.apeiron.arena.stats.interactions
        );
        println!("Live nodes:    {}", session.apeiron.arena.live_count());
    }

    if had_errors {
        process::exit(1);
    }
}

fn cmd_kompile(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: hyperion kompile <file.hyp> --theory <name> -o <output_dir/>");
        process::exit(1);
    }

    let filename = &args[0];
    let mut theory_name: Option<&str> = None;
    let mut output_dir: Option<&str> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--theory" => {
                i += 1;
                theory_name = args.get(i).map(|s| s.as_str());
            }
            "-o" => {
                i += 1;
                output_dir = args.get(i).map(|s| s.as_str());
            }
            _ => {}
        }
        i += 1;
    }

    let theory_name = theory_name.unwrap_or_else(|| {
        eprintln!("Error: --theory <name> is required");
        process::exit(1);
    });

    let output_dir = output_dir.unwrap_or_else(|| {
        eprintln!("Error: -o <output_dir/> is required");
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

    let mut session = hyperion::session::HyperionSession::new();

    for sexp in &sexps {
        match session.process(sexp) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
    }

    // Print processing output
    for line in &session.output {
        println!("{}", line);
    }

    match session.kompile(theory_name, output_dir) {
        Ok(count) => {
            println!("[KOMPILE] Generated {} files in {}/", count, output_dir);
        }
        Err(e) => {
            eprintln!("Kompile error: {}", e);
            process::exit(1);
        }
    }
}

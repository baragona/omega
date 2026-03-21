use clap::Parser;
use omega_driver::batch;
use omega_driver::codegen;
use omega_driver::repl;
use omega_driver::session::Session;

#[derive(Parser)]
#[command(name = "omega")]
#[command(about = "Omega: A Logical Framework with Reflection")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose output.
    #[arg(long, global = true)]
    verbose: bool,

    /// Output results as JSON.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Check an .omega source file.
    Check {
        /// Path to the .omega file (ignored if --stdin is used).
        file: Option<String>,
        /// Read source from stdin instead of a file.
        #[arg(long)]
        stdin: bool,
    },
    /// Compile a verified theory to a Rust crate.
    Kompile {
        /// Path to the .omega file.
        file: String,
        /// Theory to compile (if omitted, compiles the last registered theory).
        #[arg(long)]
        theory: Option<String>,
        /// Output directory for the generated Rust crate.
        #[arg(short, long, default_value = "out")]
        output: String,
    },
    /// Start an interactive REPL.
    Repl,
}

fn main() {
    let cli = Cli::parse();
    let mut session = Session::new().with_verbose(cli.verbose);

    match cli.command {
        Some(Commands::Check { file, stdin }) => {
            let (source, filename) = if stdin {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                    .expect("failed to read stdin");
                (buf, "<stdin>".to_string())
            } else {
                let f = file.expect("file path required when not using --stdin");
                let s = std::fs::read_to_string(&f).unwrap_or_else(|e| {
                    eprintln!("Error: cannot read {}: {}", f, e);
                    std::process::exit(1);
                });
                (s, f)
            };

            if cli.json {
                let output = batch::process_file_json(&mut session, &source, &filename);
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
                if output.status != "success" {
                    std::process::exit(1);
                }
            } else {
                match batch::process_file(&mut session, &source, &filename) {
                    Ok(results) => {
                        for r in results {
                            println!("{}", r);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Some(Commands::Kompile {
            file,
            theory,
            output,
        }) => {
            // First, process the file to register theories
            match batch::process_file_path(&mut session, &file) {
                Ok(results) => {
                    if cli.verbose {
                        for r in &results {
                            eprintln!("{}", r);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error processing {}: {}", file, e);
                    std::process::exit(1);
                }
            }

            // Determine which theory to compile
            let theory_name = theory.unwrap_or_else(|| {
                session
                    .kernel
                    .theory_names()
                    .last()
                    .unwrap_or(&"")
                    .to_string()
            });

            match codegen::kompile(&session, &theory_name, &output) {
                Ok(n) => {
                    println!(
                        "Compiled theory {} → {} ({} files)",
                        theory_name, output, n
                    );
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Repl) | None => {
            // Try to use rustyline, fall back to stdin
            match run_repl_rustyline(&mut session) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("REPL error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn run_repl_rustyline(session: &mut Session) -> Result<(), String> {
    let mut rl = rustyline::DefaultEditor::new()
        .map_err(|e| format!("failed to initialize readline: {}", e))?;

    struct RustylineReader<'a> {
        rl: &'a mut rustyline::DefaultEditor,
    }

    impl<'a> repl::LineReader for RustylineReader<'a> {
        fn read_line(&mut self, prompt: &str) -> Option<String> {
            match self.rl.readline(prompt) {
                Ok(line) => {
                    let _ = self.rl.add_history_entry(&line);
                    Some(line)
                }
                Err(rustyline::error::ReadlineError::Interrupted) => {
                    println!("^C");
                    Some(String::new())
                }
                Err(rustyline::error::ReadlineError::Eof) => None,
                Err(e) => {
                    eprintln!("Read error: {}", e);
                    None
                }
            }
        }
    }

    let mut reader = RustylineReader { rl: &mut rl };
    repl::run_repl(session, &mut reader)
}

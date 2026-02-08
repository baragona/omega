use clap::Parser;
use omega_driver::batch;
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
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Check an .omega source file.
    Check {
        /// Path to the .omega file.
        file: String,
    },
    /// Start an interactive REPL.
    Repl,
}

fn main() {
    let cli = Cli::parse();
    let mut session = Session::new().with_verbose(cli.verbose);

    match cli.command {
        Some(Commands::Check { file }) => {
            match batch::process_file_path(&mut session, &file) {
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

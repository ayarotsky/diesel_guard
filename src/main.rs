use clap::{Parser, Subcommand};
use diesel_guard::output::OutputFormatter;
use diesel_guard::SafetyChecker;
use std::path::PathBuf;
use std::process::exit;

#[derive(Parser)]
#[command(name = "diesel_guard")]
#[command(version, about = "Catch unsafe PostgreSQL migrations in Diesel before they take down production", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check migrations for unsafe operations
    Check {
        /// Path to migration file or directory
        path: PathBuf,

        /// Output format (text or json)
        #[arg(long, default_value = "text")]
        format: String,

        /// Allow unsafe operations (exit with 0 even if violations found)
        #[arg(long)]
        allow_unsafe: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check {
            path,
            format,
            allow_unsafe,
        } => {
            let checker = SafetyChecker::new();

            let results = match checker.check_path(&path) {
                Ok(results) => results,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    exit(1);
                }
            };

            if results.is_empty() {
                OutputFormatter::print_summary(0);
                exit(0);
            }

            let total_violations: usize = results.iter().map(|(_, v)| v.len()).sum();

            match format.as_str() {
                "json" => {
                    println!("{}", OutputFormatter::format_json(&results));
                }
                _ => {
                    // text format
                    for (file_path, violations) in &results {
                        print!("{}", OutputFormatter::format_text(file_path, violations));
                    }
                    OutputFormatter::print_summary(total_violations);
                }
            }

            if !allow_unsafe && total_violations > 0 {
                exit(1);
            }
        }
    }
}

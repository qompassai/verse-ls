#![forbid(unsafe_code)]
//! verse-ls: a language server for Epic's Verse language.
//!
//! verse-ls is advisory tooling for Neovim-first development in Diver.
//! It never claims that Verse code compiles — Build Verse Code in UEFN
//! is the only compile gate. See the repository README for the rung
//! ladder and TARGET.md for the recorded target pair.
//!
//! Modes: `--stdio` (also the default with no arguments) serves LSP on
//! stdin/stdout; `--version` prints exactly one line — the line is
//! golden-tested because the estate's theodolite bridge matches
//! `expect_version` against a probe's complete output; `--help` prints
//! usage.

mod document_store;
mod server;

use std::process::ExitCode;

/// The usage line printed for `--help` and for unknown arguments.
const USAGE: &str = "usage: verse-ls [--stdio | --version | --help]";

/// Which mode the process runs in, decided by argument parsing.
enum Mode {
    /// Serve LSP on stdin/stdout.
    Serve,
    /// Print the one-line version and exit.
    Version,
    /// Print usage to stdout and exit successfully.
    Help,
    /// Print usage to stderr and exit with a failure code.
    UsageError,
}

/// Classify the command-line arguments (excluding the program name).
fn classify(args: &[String]) -> Mode {
    match args {
        [] => Mode::Serve,
        [flag] if flag == "--stdio" => Mode::Serve,
        [flag] if flag == "--version" => Mode::Version,
        [flag] if flag == "--help" => Mode::Help,
        _ => Mode::UsageError,
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match classify(&args) {
        Mode::Serve => {
            server::run_stdio().await;
            ExitCode::SUCCESS
        }
        Mode::Version => {
            println!("verse-ls {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Mode::Help => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Mode::UsageError => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

//! `wordcloud` command-line entry point.

#![forbid(unsafe_code)]

use clap::Parser;

fn main() -> std::process::ExitCode {
    let args = wordcloud::cli::Args::parse();
    match wordcloud::cli::run(&args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wordcloud: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

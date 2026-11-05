//! `wordcloud` command-line entry point.

#![forbid(unsafe_code)]

use clap::Parser;

fn main() -> std::process::ExitCode {
    let args = wordcloud_rs::cli::Args::parse();
    match wordcloud_rs::cli::run(&args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wordcloud: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

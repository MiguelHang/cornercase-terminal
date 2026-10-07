use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use cornercase::cli::{self, Cli};

fn main() -> Result<ExitCode> {
    Ok(if cli::run(Cli::parse())? { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

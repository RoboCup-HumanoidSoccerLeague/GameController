//! This crate defines the main program to analyze GameController log files.

use std::{fs::File, path::PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use game_controller_core::log::TimestampedLogEntry;

use game_controller_logs::{statistics, team_communication};

/// This struct defines the parser for the command line arguments.
#[derive(Parser)]
#[command(about, author, version)]
struct Args {
    /// The kind of thing that should be done with the log files.
    #[command(subcommand)]
    pub command: Commands,
}

/// This struct defines the command line subcommands.
#[derive(Subcommand)]
enum Commands {
    /// Extract statistics about general game events.
    Statistics {
        /// Print a CSV header line before the statistics.
        #[arg(long)]
        header: bool,
        /// The paths of the log files to analyze.
        #[arg(required_unless_present = "header")]
        paths: Vec<PathBuf>,
    },
    /// Extract statistics about the bandwidth usage of team communication.
    TeamCommunication {
        /// The paths of the log files to analyze.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
}

/// The type of a function that evaluates the entries of one log file.
type Evaluate = fn(Vec<TimestampedLogEntry>) -> Result<()>;

/// This function applies an evaluation function to one log file.
fn process_file(f: File, evaluate: Evaluate) -> Result<()> {
    let entries: Vec<TimestampedLogEntry> =
        serde_yaml::from_reader(f).context("could not parse log file")?;
    evaluate(entries)
}

fn main() -> Result<()> {
    let args = Args::parse();

    let (paths, evaluate): (_, Evaluate) = match &args.command {
        Commands::Statistics { header, paths } => {
            if *header {
                statistics::header();
            }
            (paths, |entries| {
                statistics::evaluate(entries).context("could not create statistics from log file")
            })
        }
        Commands::TeamCommunication { paths } => (paths, |entries| {
            team_communication::evaluate(entries).context("could not evaluate team communication")
        }),
    };

    // A file that can't be processed does not stop the others from being processed.
    let mut num_failed = 0;
    for path in paths {
        if let Err(error) = File::open(path)
            .context("could not open log file")
            .and_then(|f| process_file(f, evaluate))
        {
            eprintln!("{}: {error:?}", path.display());
            num_failed += 1;
        }
    }
    if num_failed > 0 {
        bail!(
            "{num_failed} of {} log files could not be processed",
            paths.len()
        );
    }

    Ok(())
}

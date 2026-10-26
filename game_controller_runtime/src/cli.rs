//! This module defines the command line interface.

use std::{env::args_os, ffi::OsString, path::PathBuf};

use anyhow::Error;
use clap::Parser;

use crate::paths::{default_args_file, read_args_file};

/// This struct defines the parser for the command line arguments.
#[derive(Parser)]
#[command(about, author, version, args_override_self = true)]
pub struct Args {
    /// Set the competition type.
    #[arg(long, short)]
    pub competition: Option<String>,
    /// Set the home team (name or number).
    #[arg(long)]
    pub home_team: Option<String>,
    /// Set the away team (name or number).
    #[arg(long)]
    pub away_team: Option<String>,
    /// Set the no-delay test flag.
    #[arg(long)]
    pub no_delay: bool,
    /// Set the penalty shoot-out test flag.
    #[arg(long)]
    pub penalty_shootout: bool,
    /// Open the main window in fullscreen mode.
    #[arg(long, short)]
    pub fullscreen: bool,
    /// Set the network interface to listen/send on/to.
    #[arg(long, short)]
    pub interface: Option<String>,
    /// Send control messages to 255.255.255.255.
    #[arg(long, short)]
    pub broadcast: bool,
    /// Join multicast groups for simulated team communication.
    #[arg(long, short)]
    pub multicast: bool,
    /// Sync the log file to the storage device after each entry.
    #[arg(long)]
    pub sync: bool,
    /// Specify the path to a log file to replay.
    #[arg(long)]
    pub replay: Option<PathBuf>,
    /// Use this configuration directory exclusively (also settable via GAMECONTROLLER_CONFIG).
    #[arg(long)]
    pub config_dir: Option<PathBuf>,
    /// Read persistent options from this file instead of the one in the user's config directory.
    #[arg(long)]
    pub args_file: Option<PathBuf>,
    /// Ignore the file with persistent options.
    #[arg(long)]
    pub no_args_file: bool,
    /// Print the locations of configuration, options file and logs and exit.
    #[arg(long)]
    pub show_paths: bool,
    /// Copy the installed configuration to the user's config directory for editing and exit.
    #[arg(long)]
    pub install_config: bool,
}

/// This function finds the value of `--args-file` and whether `--no-args-file` is present in raw
/// arguments, so that the options file can be read before the real parsing.
fn scan_options_file_flags(args: &[OsString]) -> (Option<PathBuf>, bool) {
    let mut file = None;
    let mut disabled = false;
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        let Some(arg) = arg.to_str() else { continue };
        if arg == "--" {
            break;
        } else if arg == "--no-args-file" {
            disabled = true;
        } else if arg == "--args-file" {
            file = iter.next().map(PathBuf::from);
        } else if let Some(value) = arg.strip_prefix("--args-file=") {
            file = Some(PathBuf::from(value));
        }
    }
    (file, disabled)
}

/// The result of [parse_args].
pub struct ParsedArgs {
    /// The combined arguments.
    pub args: Args,
    /// The options file and the arguments that were taken from it, if there was one.
    pub options_file: Option<(PathBuf, Vec<String>)>,
}

/// This function parses the command line arguments. Persistent options from the options file are
/// inserted before the real arguments, so that the latter take precedence. This also handles the
/// version and help commands and wrong arguments by exiting the process.
pub fn parse_args() -> Result<ParsedArgs, Error> {
    let raw: Vec<OsString> = args_os().collect();
    let (explicit_file, disabled) = scan_options_file_flags(&raw);
    let file = explicit_file
        .or_else(default_args_file)
        .filter(|_| !disabled);
    let from_file = match &file {
        Some(file) => read_args_file(file)?.map(|args| (file.clone(), args)),
        None => None,
    };
    let mut combined = raw.clone();
    if let Some((_, args)) = &from_file {
        combined.splice(1..1, args.iter().map(OsString::from));
    }
    Ok(ParsedArgs {
        args: Args::parse_from(combined),
        options_file: from_file,
    })
}

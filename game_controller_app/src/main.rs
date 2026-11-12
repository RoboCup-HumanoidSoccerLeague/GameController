//! This crate defines the main program of the GameController application.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::{
    path::Path,
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::Context;
use eframe::egui;

use game_controller_app::{
    app::{executable_path, App, LAUNCHER_SIZE},
    style,
};
use game_controller_runtime::{
    cli::{parse_args, Args},
    paths::{
        config_candidates, default_args_file, install_user_config, user_config_directory,
        user_directory, user_log_directory,
    },
};

/// This function prints where the GameController looks for its files.
fn show_paths(args: &Args, exe: &Path) {
    let user = user_config_directory();
    println!("Executable: {}", exe.display());
    for (origin, directory) in config_candidates(args.config_dir.as_deref(), user.as_deref(), exe) {
        let state = if directory.join("teams.yaml").is_file() {
            "found"
        } else {
            "not found"
        };
        println!("Config ({origin}): {} [{state}]", directory.display());
    }
    match default_args_file() {
        Some(file) => println!("Options file: {}", file.display()),
        None => println!("Options file: unknown"),
    }
    match user_log_directory() {
        Some(directory) => println!("Logs: {}", directory.display()),
        None => println!("Logs: unknown"),
    }
}

/// This function runs the app. It first parses command line arguments and displays a launcher in
/// which the user can configure the settings for the game. When the user is done with that, the
/// main window and network services are started and shut down when the app is quit.
fn main() {
    // Parse the command line arguments first. This includes handling the version and help commands
    // and wrong arguments.
    let parsed = parse_args().unwrap_or_else(|error| {
        eprintln!("{error:?}");
        exit(2);
    });
    let args = parsed.args;
    if let Some((file, options)) = &parsed.options_file {
        if !options.is_empty() {
            eprintln!(
                "Using options from {}: {}",
                file.display(),
                options.join(" ")
            );
        }
    }

    // These commands don't need the app.
    if args.show_paths || args.install_config {
        let result = executable_path().and_then(|exe| {
            if args.show_paths {
                show_paths(&args, &exe);
                Ok(())
            } else {
                let user = user_directory().context("could not determine the user directory")?;
                install_user_config(&user, &exe)
            }
        });
        if let Err(error) = result {
            eprintln!("{error:?}");
            exit(1);
        }
        return;
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GameController")
            // The window class (X11) / app ID (Wayland) matches StartupWMClass in the desktop
            // entry of the Linux distribution, which lets desktops associate the two.
            .with_app_id("GameController")
            .with_inner_size(LAUNCHER_SIZE)
            .with_resizable(false)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../icons/icon.png"))
                    .expect("the icon must be a valid PNG"),
            ),
        centered: true,
        ..Default::default()
    };
    let failed = Arc::new(AtomicBool::new(false));
    let app_failed = failed.clone();
    let result = eframe::run_native(
        "GameController",
        options,
        Box::new(move |cc| {
            style::setup(&cc.egui_ctx);
            Ok(Box::new(App::new(args, runtime, app_failed)))
        }),
    );
    if let Err(error) = result {
        eprintln!("{error}");
        exit(1);
    }
    // A fatal error (shown in the window and on stderr) is a failure of the program.
    if failed.load(Ordering::Relaxed) {
        exit(1);
    }
}

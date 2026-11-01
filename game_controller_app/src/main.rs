//! This crate defines the main program of the GameController application.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::{
    env::current_exe,
    path::{Path, PathBuf},
    process::exit,
};

use anyhow::{Context, Error, Result};
use tauri::{
    async_runtime, generate_context, AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use game_controller_runtime::{
    cli::{parse_args, Args},
    launch::make_launch_data,
    paths::{
        config_candidates, default_args_file, install_user_config, resolve_config_directory,
        stale_config_warning, user_config_directory, user_directory, user_log_directory,
    },
    shutdown_runtime, RuntimeState,
};

mod handlers;

use handlers::get_invoke_handler;

/// This struct is used as state to tell the handlers which configuration directory is in use.
pub struct ConfigDirectory(pub PathBuf);

/// This function returns the real path of the executable, which is the base for finding the
/// installed configuration (symbolic links must not confuse the search).
fn executable_path() -> Result<PathBuf> {
    let exe = current_exe().context("could not determine the path of the executable")?;
    Ok(exe.canonicalize().unwrap_or(exe))
}

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

/// This function shows an error to the user and exits the app when the user dismisses it. The
/// error is also printed to stderr for users who started the app from a terminal.
pub fn show_fatal_error(app: &AppHandle, error: &Error) {
    eprintln!("{error:?}");
    let app = app.clone();
    app.dialog()
        .message(format!("{error:#}"))
        .title("GameController")
        .kind(MessageDialogKind::Error)
        .show(move |_| app.exit(1));
}

/// This function runs the tauri app. It first parses command line arguments and displays a
/// launcher in which the user can configure the settings for the game. When the user is done with
/// that, the main window and network services are started and shut down when the app is quit.
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

    // We want to manage an external tokio runtime, mainly to keep dependencies to tauri minimal,
    // but also because I don't know how to do the shutdown correctly otherwise.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    async_runtime::set(runtime.handle().clone());

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let resolved = match executable_path().and_then(|exe| {
                resolve_config_directory(
                    args.config_dir.as_deref(),
                    user_config_directory().as_deref(),
                    &exe,
                )
            }) {
                Ok(resolved) => resolved,
                Err(error) => {
                    show_fatal_error(app.handle(), &error);
                    return Ok(());
                }
            };
            eprintln!("Using config directory {resolved}");
            if let Some(warning) = stale_config_warning(&resolved) {
                eprintln!("Warning: {warning}");
            }
            match make_launch_data(&resolved.directory, args) {
                Ok(launch_data) => {
                    app.manage(launch_data);
                    app.manage(ConfigDirectory(resolved.directory));
                }
                Err(error) => {
                    // The launcher window is not created because it needs the launch data.
                    show_fatal_error(
                        app.handle(),
                        &error.context(format!("invalid configuration in {resolved}")),
                    );
                    return Ok(());
                }
            };

            let _window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .center()
                    .inner_size(640.0, 560.0)
                    .resizable(false)
                    .title("GameController")
                    .build()?;
            Ok(())
        })
        .invoke_handler(get_invoke_handler())
        .build(generate_context!())
        .expect("error while running tauri application");

    app.run(move |handle, event| {
        if let RunEvent::Exit = event {
            if let Some(runtime_state) = handle.try_state::<RuntimeState>() {
                runtime.block_on(shutdown_runtime(&runtime_state));
            }
        }
    });
}

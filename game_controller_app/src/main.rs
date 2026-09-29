//! This crate defines the main program of the GameController application.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::env::current_exe;

use anyhow::Error;
use clap::Parser;
use tauri::{
    async_runtime, generate_context, AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use game_controller_runtime::{
    cli::Args, launch::make_launch_data, shutdown_runtime, RuntimeState,
};

mod handlers;

use handlers::get_invoke_handler;

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
    let args = Args::parse();

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
            let config_directory = current_exe()?
                .parent()
                .unwrap()
                .join("..")
                .join("..")
                .join("config");
            match make_launch_data(&config_directory, args) {
                Ok(launch_data) => {
                    app.manage(launch_data);
                }
                Err(error) => {
                    // The launcher window is not created because it needs the launch data.
                    show_fatal_error(app.handle(), &error);
                    return Ok(());
                }
            };

            let _window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .center()
                    .inner_size(640.0, 480.0)
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

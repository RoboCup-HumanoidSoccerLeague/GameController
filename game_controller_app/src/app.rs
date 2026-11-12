//! This module defines the application: the launcher, and the main window from which the game is
//! operated once the runtime has been started.

use std::{
    env::current_exe,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use anyhow::{Context, Error, Result};
use eframe::egui::{self, Rect, Vec2, ViewportCommand};

use game_controller_core::types::Side;
use game_controller_runtime::{
    cli::Args,
    launch::{make_launch_data, LaunchData, LaunchSettings},
    paths::{
        resolve_config_directory, stale_config_warning, user_config_directory, user_log_directory,
    },
    shutdown_runtime, start_runtime, RuntimeState, UiState,
};

use crate::{actions::declared_actions, launcher::Launcher, main_view::MainView, style::tw};

/// The minimum size of the main window.
const MAIN_WINDOW_SIZE: Vec2 = Vec2::new(1040.0, 848.0);

/// The size of the launcher window.
pub const LAUNCHER_SIZE: Vec2 = Vec2::new(640.0, 560.0);

/// This function returns the real path of the executable, which is the base for finding the
/// installed configuration (symbolic links must not confuse the search).
pub fn executable_path() -> Result<PathBuf> {
    let exe = current_exe().context("could not determine the path of the executable")?;
    Ok(exe.canonicalize().unwrap_or(exe))
}

/// The running game: the runtime services and the latest state they sent.
struct Running {
    runtime_state: RuntimeState,
    /// The slot into which the runtime puts a new state. It is emptied each frame.
    new_ui_state: Arc<Mutex<Option<UiState>>>,
    /// The latest state that was taken out of the slot.
    ui_state: Option<UiState>,
    view: MainView,
}

enum Screen {
    Launcher(Box<Launcher>),
    Running(Box<Running>),
    /// A fatal error. The app exits when the user dismisses it.
    Error(String),
}

/// The application: the launcher, and the main window once the game has been started.
pub struct App {
    runtime: tokio::runtime::Runtime,
    config_directory: PathBuf,
    screen: Screen,
    /// This is set when the app exits from a fatal error, so that the main program can exit
    /// with a failure status.
    failed: Arc<AtomicBool>,
}

impl App {
    /// This function creates the app, which shows the launcher (or an error if the configuration
    /// can't be found or is invalid). `failed` is set if the app exits from a fatal error.
    pub fn new(args: Args, runtime: tokio::runtime::Runtime, failed: Arc<AtomicBool>) -> Self {
        let mut config_directory = PathBuf::new();
        let screen = match executable_path().and_then(|exe| {
            resolve_config_directory(
                args.config_dir.as_deref(),
                user_config_directory().as_deref(),
                &exe,
            )
        }) {
            Err(error) => {
                eprintln!("{error:?}");
                Screen::Error(format!("{error:#}"))
            }
            Ok(resolved) => {
                eprintln!("Using config directory {resolved}");
                if let Some(warning) = stale_config_warning(&resolved) {
                    eprintln!("Warning: {warning}");
                }
                config_directory = resolved.directory.clone();
                match make_launch_data(&resolved.directory, args) {
                    Ok(data) => Screen::Launcher(Box::new(Launcher::new(data))),
                    Err(error) => {
                        let error = error.context(format!("invalid configuration in {resolved}"));
                        eprintln!("{error:?}");
                        Screen::Error(format!("{error:#}"))
                    }
                }
            }
        };
        Self {
            runtime,
            config_directory,
            screen,
            failed,
        }
    }

    /// This function is called when the user finishes the launcher. It starts the runtime (game
    /// state, network services, logging) and switches to the main window.
    fn launch(&mut self, ctx: &egui::Context, data: &LaunchData, settings: LaunchSettings) {
        let new_ui_state = Arc::new(Mutex::new(None));
        let send_ui_state = {
            let new_ui_state = new_ui_state.clone();
            let ctx = ctx.clone();
            move |state: UiState| {
                *new_ui_state.lock().unwrap() = Some(state);
                ctx.request_repaint();
                Ok(())
            }
        };
        let result = self.runtime.block_on(async {
            // The logs are kept in a per-user directory, so that they survive removing or
            // replacing the installation, and all installations write to the same place.
            let log_directory =
                user_log_directory().context("could not determine the directory for logs")?;
            start_runtime(
                &self.config_directory,
                &log_directory,
                &settings,
                &data.teams,
                &data.network_interfaces,
                Box::new(send_ui_state),
            )
            .await
            .context("could not start runtime")
        });
        let runtime_state = match result {
            Ok(runtime_state) => runtime_state,
            Err(error) => {
                self.fatal(&error);
                return;
            }
        };

        // Declare the actions before the runtime starts sending states, so that already the
        // first state contains their legality.
        let _ = runtime_state
            .subscribed_actions_sender
            .send(declared_actions());
        runtime_state.ui_notify.notify_one();

        let team_names = [Side::Home, Side::Away].map(|side| {
            let number = runtime_state.params.game.teams[side].number;
            data.teams
                .iter()
                .find(|team| team.number == number)
                .map_or_else(|| number.to_string(), |team| team.name.clone())
        });
        let view = MainView::new(runtime_state.params.clone(), team_names);

        ctx.send_viewport_cmd(ViewportCommand::Resizable(true));
        ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(MAIN_WINDOW_SIZE));
        if settings.window.fullscreen {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
        } else {
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(MAIN_WINDOW_SIZE));
            // The window grows around its center, so that it stays on the monitor that the
            // launcher is on (which is centered there unless the user moved it). The monitor's
            // origin is not known, so the window can't be centered on the monitor itself.
            let rects = ctx.input(|i| i.viewport().outer_rect.zip(i.viewport().inner_rect));
            if let Some((outer, inner)) = rects {
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(
                    main_window_rect(outer, inner).min,
                ));
            }
        }

        self.screen = Screen::Running(Box::new(Running {
            runtime_state,
            new_ui_state,
            ui_state: None,
            view,
        }));
    }

    fn fatal(&mut self, error: &Error) {
        eprintln!("{error:?}");
        self.screen = Screen::Error(format!("{error:#}"));
    }
}

/// This function returns the outer rectangle of the main window, given the outer and inner
/// rectangles of the launcher window: it keeps the center and the decorations.
fn main_window_rect(outer: Rect, inner: Rect) -> Rect {
    let decorations = (outer.size() - inner.size()).max(Vec2::ZERO);
    Rect::from_center_size(outer.center(), MAIN_WINDOW_SIZE + decorations)
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        match &mut self.screen {
            Screen::Launcher(launcher) => {
                if let Some(settings) = launcher.ui(ui) {
                    let data = launcher.data().clone();
                    self.launch(&ctx, &data, settings);
                }
            }
            Screen::Running(running) => {
                // The lock is only held to move a new state out of the slot.
                let new_ui_state = running.new_ui_state.lock().unwrap().take();
                if new_ui_state.is_some() {
                    running.ui_state = new_ui_state;
                }
                if let Some(state) = &running.ui_state {
                    for action in running.view.ui(ui, state) {
                        let _ = running.runtime_state.action_sender.send(action);
                    }
                } else {
                    ui.painter().rect_filled(ui.max_rect(), 0.0, tw::WHITE);
                }
            }
            Screen::Error(message) => {
                let message = message.clone();
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 3.0);
                        ui.heading("GameController");
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(message).color(tw::RED_600));
                        ui.add_space(16.0);
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                    });
                });
            }
        }
    }

    fn on_exit(&mut self) {
        match &self.screen {
            Screen::Running(running) => {
                self.runtime
                    .block_on(shutdown_runtime(&running.runtime_state));
            }
            Screen::Error(_) => self.failed.store(true, Ordering::Relaxed),
            Screen::Launcher(_) => {}
        }
    }
}

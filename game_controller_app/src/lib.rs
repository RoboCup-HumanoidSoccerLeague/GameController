//! This crate implements the GameController user interface with egui: a launcher in which the game
//! is configured, and the main window from which the game is operated. The views only depend on
//! [game_controller_runtime::UiState] and return the actions to apply, so they can be rendered
//! without a running runtime.

pub mod actions;
pub mod app;
pub mod launcher;
pub mod main_view;
pub mod style;

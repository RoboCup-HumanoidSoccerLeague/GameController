use std::array;

use egui_kittest::Harness;
use enum_map::enum_map;
use game_controller_core::{action::ActionContext, log::NullLogger, GameController};

use crate::{
    actions::declared_actions, app::MAIN_WINDOW_SIZE, style::setup, tests::create_test_params,
};

use super::*;

#[test]
/// Tests the rendering of the initial game view with two dummy teams.
fn initial_game_view() {
    let params = create_test_params();
    let game_controller = GameController::new(params.clone(), Box::new(NullLogger));
    let mut game = game_controller.get_game(false).clone();
    let action_context = ActionContext::new(&mut game, &params, None, None);
    let state = &UiState {
        connection_status: enum_map! {
            Side::Home =>  array::repeat(ConnectionStatus::Good),
            Side::Away =>  array::repeat(ConnectionStatus::Good),
        },
        game: game_controller.get_game(false).clone(),
        legal_actions: declared_actions()
            .into_iter()
            .map(|a| a.is_legal(&action_context))
            .collect(),
        undo_actions: game_controller.get_undo_actions(5),
    };

    let mut view = MainView::new(
        params.clone(),
        ["Invisibles".to_string(), "Incredible Invisible".to_string()],
    );

    let app = |ui: &mut egui::Ui| {
        setup(ui);
        view.ui(ui, state);
    };

    let mut harness = Harness::new_ui(app);
    harness.set_size(MAIN_WINDOW_SIZE);

    harness.run();

    harness.snapshot("initial_game_view");
}

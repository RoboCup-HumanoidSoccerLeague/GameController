use enum_map::enum_map;
use game_controller_core::types::{
    Color, GameParams, Params, Side, SideMapping, TeamParams, TestParams,
};

/// Create params that could be set by the launcher for use in tests.
/// This creates "Middle - Advanced" competition parameters.
pub(crate) fn create_test_params() -> Params {
    let home = TeamParams {
        number: 0,
        field_player_color: Color::Blue,
        goalkeeper_color: Color::Purple,
    };

    let away = TeamParams {
        number: 0,
        field_player_color: Color::Green,
        goalkeeper_color: Color::White,
    };

    let teams = enum_map! {
        Side::Home => home.clone(),
        Side::Away => away.clone(),
    };
    let game = GameParams {
        teams,
        kick_off_side: Side::Away,
        side_mapping: SideMapping::HomeDefendsLeftGoal,
        test: TestParams::default(),
    };
    let competition =
        serde_yaml::from_str(include_str!("../../config/middle_advanced/params.yaml")).unwrap();

    Params { competition, game }
}

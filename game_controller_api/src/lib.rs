use std::time::Duration;

use bytes::Bytes;

use game_controller_core::action::VAction;
use game_controller_core::actions::{
    AddAdditionalTime, FinishHalf, FinishPenaltyShot, FinishSetPlay, FreePenaltyShot, FreeSetPlay,
    GlobalGameStuck, Goal, Penalize, SelectGoalkeeper, SelectPenaltyShotPlayer, StartExtraTime,
    StartPenaltyShootout, StartSetPlay, StopPlay, Substitute, SwitchHalf, TeamMessage, Timeout,
    Undo, Unpenalize, WaitForPenaltyShot, WaitForSetPlay,
};
use game_controller_core::log::NullLogger;
use game_controller_core::types::{
    ActionSource, GameParams, Params, PenaltyCall, PlayerNumber, SetPlay, Side, SideMapping,
    TeamParams, TestParams,
};
use game_controller_core::GameController;
use game_controller_msgs::ControlMessage;

/// # Safety
///
/// `yaml` must point to `len` readable bytes and `team_params` must point to two consecutive
/// [TeamParams] (home, then away).
#[no_mangle]
pub unsafe extern "C" fn gc_params_new(
    yaml: *const u8,
    len: usize,
    team_params: *const TeamParams,
    kick_off_side: Side,
    side_mapping: SideMapping,
    test_params: TestParams,
) -> *mut Params {
    let yaml_str = std::str::from_utf8(unsafe { std::slice::from_raw_parts(yaml, len) })
        .expect("could not parse utf-8");
    let competition_params =
        serde_yaml::from_str(yaml_str).expect("could not parse competition params");
    let params = Params {
        competition: competition_params,
        game: GameParams {
            teams: unsafe {
                enum_map::enum_map! { Side::Home => (*team_params).clone(), Side::Away => (*team_params.wrapping_add(1)).clone() }
            },
            kick_off_side,
            side_mapping,
            test: test_params,
        },
    };
    Box::into_raw(Box::new(params))
}

/// # Safety
///
/// `params` must have been returned by [gc_params_new] and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn gc_params_destroy(params: *mut Params) {
    drop(unsafe { Box::from_raw(params) });
}

#[no_mangle]
pub extern "C" fn gc_new(params: &Params) -> *mut GameController {
    Box::into_raw(Box::new(GameController::new(
        params.clone(),
        Box::new(NullLogger),
    )))
}

#[no_mangle]
pub extern "C" fn gc_seek(game_controller: &mut GameController, duration: u64) {
    game_controller.seek(Duration::from_millis(duration));
}

/// # Safety
///
/// `action` must have been returned by one of the `gc_action_*` functions. Ownership is taken, so
/// it must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn gc_apply(
    game_controller: &mut GameController,
    action: *mut VAction,
    source: ActionSource,
) -> bool {
    let a = unsafe { Box::from_raw(action) };
    game_controller.apply(*a, source)
}

/// # Safety
///
/// `game_controller` must have been returned by [gc_new] and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn gc_destroy(game_controller: *mut GameController) {
    drop(unsafe { Box::from_raw(game_controller) });
}

#[no_mangle]
pub extern "C" fn gc_action_add_additional_time() -> *mut VAction {
    Box::into_raw(Box::new(VAction::AddAdditionalTime(AddAdditionalTime)))
}

#[no_mangle]
pub extern "C" fn gc_action_finish_half() -> *mut VAction {
    Box::into_raw(Box::new(VAction::FinishHalf(FinishHalf)))
}

#[no_mangle]
pub extern "C" fn gc_action_finish_penalty_shot() -> *mut VAction {
    Box::into_raw(Box::new(VAction::FinishPenaltyShot(FinishPenaltyShot)))
}

#[no_mangle]
pub extern "C" fn gc_action_finish_set_play() -> *mut VAction {
    Box::into_raw(Box::new(VAction::FinishSetPlay(FinishSetPlay)))
}

#[no_mangle]
pub extern "C" fn gc_action_free_penalty_shot() -> *mut VAction {
    Box::into_raw(Box::new(VAction::FreePenaltyShot(FreePenaltyShot)))
}

#[no_mangle]
pub extern "C" fn gc_action_free_set_play() -> *mut VAction {
    Box::into_raw(Box::new(VAction::FreeSetPlay(FreeSetPlay)))
}

#[no_mangle]
pub extern "C" fn gc_action_global_game_stuck() -> *mut VAction {
    Box::into_raw(Box::new(VAction::GlobalGameStuck(GlobalGameStuck)))
}

#[no_mangle]
pub extern "C" fn gc_action_goal(side: Side) -> *mut VAction {
    Box::into_raw(Box::new(VAction::Goal(Goal { side })))
}

#[no_mangle]
pub extern "C" fn gc_action_penalize(side: Side, player: u8, call: PenaltyCall) -> *mut VAction {
    Box::into_raw(Box::new(VAction::Penalize(Penalize {
        side,
        player: PlayerNumber::new(player),
        call,
    })))
}

#[no_mangle]
pub extern "C" fn gc_action_select_goalkeeper(side: Side, player: u8) -> *mut VAction {
    Box::into_raw(Box::new(VAction::SelectGoalkeeper(SelectGoalkeeper {
        side,
        player: PlayerNumber::new(player),
    })))
}

#[no_mangle]
pub extern "C" fn gc_action_select_penalty_shot_player(
    side: Side,
    player: u8,
    goalkeeper: bool,
) -> *mut VAction {
    Box::into_raw(Box::new(VAction::SelectPenaltyShotPlayer(
        SelectPenaltyShotPlayer {
            side,
            player: PlayerNumber::new(player),
            goalkeeper,
        },
    )))
}

#[no_mangle]
pub extern "C" fn gc_action_start_extra_time() -> *mut VAction {
    Box::into_raw(Box::new(VAction::StartExtraTime(StartExtraTime)))
}

#[no_mangle]
pub extern "C" fn gc_action_start_penalty_shootout(sides: SideMapping) -> *mut VAction {
    Box::into_raw(Box::new(VAction::StartPenaltyShootout(
        StartPenaltyShootout { sides },
    )))
}

/// # Safety
///
/// `side` must either be null or point to a valid [Side].
#[no_mangle]
pub unsafe extern "C" fn gc_action_start_set_play(
    side: *const Side,
    set_play: SetPlay,
) -> *mut VAction {
    Box::into_raw(Box::new(VAction::StartSetPlay(StartSetPlay {
        side: if side.is_null() {
            None
        } else {
            unsafe { Some(*side) }
        },
        set_play,
    })))
}

#[no_mangle]
pub extern "C" fn gc_action_stop_play(resume: bool) -> *mut VAction {
    Box::into_raw(Box::new(VAction::StopPlay(StopPlay { resume })))
}

#[no_mangle]
pub extern "C" fn gc_action_substitute(side: Side, player_out: u8, player_in: u8) -> *mut VAction {
    Box::into_raw(Box::new(VAction::Substitute(Substitute {
        side,
        player_out: PlayerNumber::new(player_out),
        player_in: PlayerNumber::new(player_in),
    })))
}

#[no_mangle]
pub extern "C" fn gc_action_switch_half() -> *mut VAction {
    Box::into_raw(Box::new(VAction::SwitchHalf(SwitchHalf)))
}

#[no_mangle]
pub extern "C" fn gc_action_team_message(side: Side, illegal: bool) -> *mut VAction {
    Box::into_raw(Box::new(VAction::TeamMessage(TeamMessage {
        side,
        illegal,
    })))
}

/// # Safety
///
/// `side` must either be null or point to a valid [Side].
#[no_mangle]
pub unsafe extern "C" fn gc_action_timeout(side: *const Side) -> *mut VAction {
    Box::into_raw(Box::new(VAction::Timeout(Timeout {
        side: if side.is_null() {
            None
        } else {
            unsafe { Some(*side) }
        },
    })))
}

#[no_mangle]
pub extern "C" fn gc_action_undo(states: u32) -> *mut VAction {
    Box::into_raw(Box::new(VAction::Undo(Undo { states })))
}

#[no_mangle]
pub extern "C" fn gc_action_unpenalize(side: Side, player: u8, force: bool) -> *mut VAction {
    Box::into_raw(Box::new(VAction::Unpenalize(Unpenalize {
        side,
        player: PlayerNumber::new(player),
        force,
    })))
}

#[no_mangle]
pub extern "C" fn gc_action_wait_for_penalty_shot() -> *mut VAction {
    Box::into_raw(Box::new(VAction::WaitForPenaltyShot(WaitForPenaltyShot)))
}

#[no_mangle]
pub extern "C" fn gc_action_wait_for_set_play() -> *mut VAction {
    Box::into_raw(Box::new(VAction::WaitForSetPlay(WaitForSetPlay)))
}

/// # Safety
///
/// `data` must point to a writable buffer of at least `CONTROL_MESSAGE_SIZE` (158) bytes.
#[no_mangle]
pub unsafe extern "C" fn gc_read(
    game_controller: &mut GameController,
    packet_number: u8,
    true_data: bool,
    data: *mut u8,
) {
    let bytes: Bytes = ControlMessage::new(
        game_controller.get_game(!true_data),
        &game_controller.params,
        packet_number,
        true_data,
    )
    .into();
    unsafe {
        std::ptr::copy(bytes.as_ptr(), data, bytes.len());
    }
}

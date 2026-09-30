//! This crate exposes the GameController (and an automatic referee) to C++ programs such as
//! simulators.
//!
//! The header `headers/GameController.h` is generated at build time. It declares everything in the
//! namespace `RoboCup`.
//!
//! # Usage
//!
//! Create parameters with [gc_params_new] and a GameController with [gc_new]. In each simulation
//! step, advance the time with [gc_seek], optionally run [autoreferee::gc_autoreferee_update], and
//! obtain the state in the format of a control message with [gc_read].
//!
//! Actions are created by the `gc_action_*` functions. [gc_apply] takes ownership of an action
//! (so `gc_apply(gc, gc_action_goal(side), source)` does not leak), while [gc_is_legal] does not.
//! Actions that are never applied must be released with [gc_action_destroy].
//!
//! # Errors
//!
//! Functions that fail return `NULL` or `false` (or do nothing if they have no result). The
//! reason can be obtained with [error::gc_last_error]. In particular, `NULL` handles and invalid
//! player numbers are reported this way. `NULL` is also accepted by the `*_destroy` functions.

use std::{ptr::null_mut, time::Duration};

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
    ActionSource, GameParams, Params, PenaltyCall, SetPlay, Side, SideMapping, TeamParams,
    TestParams,
};
use game_controller_core::GameController;
use game_controller_msgs::{ControlMessage, CONTROL_MESSAGE_SIZE};

pub mod autoreferee;
pub mod error;

use error::{player_number, set_last_error};

/// This function returns the given value, or stores an error if it is [None] (i.e. a NULL pointer
/// was passed from C).
pub(crate) fn non_null<T>(value: Option<T>, name: &str) -> Option<T> {
    if value.is_none() {
        set_last_error(format!("{name} must not be NULL"));
    }
    value
}

/// This function moves an action to the heap so that it can be passed to C.
fn new_action(action: VAction) -> *mut VAction {
    Box::into_raw(Box::new(action))
}

/// This function creates GameController parameters.
///
/// The competition parameters are given as UTF-8 encoded YAML data (the content of a
/// `params.yaml` file), the parameters of the game as separate arguments. The caller takes
/// ownership of the result and must release it with [gc_params_destroy]. Returns `NULL` on error.
///
/// # Safety
///
/// `yaml` must be `NULL` or point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn gc_params_new(
    yaml: *const u8,
    len: usize,
    home: Option<&TeamParams>,
    away: Option<&TeamParams>,
    kick_off_side: Side,
    side_mapping: SideMapping,
    test_params: TestParams,
) -> *mut Params {
    let (Some(home), Some(away)) = (non_null(home, "home"), non_null(away, "away")) else {
        return null_mut();
    };
    if yaml.is_null() {
        set_last_error("yaml must not be NULL");
        return null_mut();
    }
    let yaml = match std::str::from_utf8(unsafe { std::slice::from_raw_parts(yaml, len) }) {
        Ok(yaml) => yaml,
        Err(error) => {
            set_last_error(format!("competition params are not valid UTF-8: {error}"));
            return null_mut();
        }
    };
    let competition = match serde_yaml::from_str(yaml) {
        Ok(competition) => competition,
        Err(error) => {
            set_last_error(format!("could not parse competition params: {error}"));
            return null_mut();
        }
    };
    Box::into_raw(Box::new(Params {
        competition,
        game: GameParams {
            teams: enum_map::enum_map! {
                Side::Home => home.clone(),
                Side::Away => away.clone(),
            },
            kick_off_side,
            side_mapping,
            test: test_params,
        },
    }))
}

/// This function destroys GameController parameters.
///
/// # Safety
///
/// `params` must be `NULL` or have been returned by [gc_params_new]. It must not be used
/// afterwards.
#[no_mangle]
pub unsafe extern "C" fn gc_params_destroy(params: *mut Params) {
    if !params.is_null() {
        drop(unsafe { Box::from_raw(params) });
    }
}

/// This function creates a GameController.
///
/// The parameters are copied, so they may be destroyed afterwards. The caller takes ownership of
/// the result and must release it with [gc_destroy]. Returns `NULL` on error.
#[no_mangle]
pub extern "C" fn gc_new(params: Option<&Params>) -> *mut GameController {
    let Some(params) = non_null(params, "params") else {
        return null_mut();
    };
    Box::into_raw(Box::new(GameController::new(
        params.clone(),
        Box::new(NullLogger),
    )))
}

/// This function destroys a GameController.
///
/// # Safety
///
/// `game_controller` must be `NULL` or have been returned by [gc_new]. It must not be used
/// afterwards.
#[no_mangle]
pub unsafe extern "C" fn gc_destroy(game_controller: *mut GameController) {
    if !game_controller.is_null() {
        drop(unsafe { Box::from_raw(game_controller) });
    }
}

/// This function lets `duration` milliseconds pass.
///
/// Timers are updated, and the actions of timers that expire in the meantime are applied.
#[no_mangle]
pub extern "C" fn gc_seek(game_controller: Option<&mut GameController>, duration: u64) {
    if let Some(game_controller) = non_null(game_controller, "game_controller") {
        game_controller.seek(Duration::from_millis(duration));
    }
}

/// This function applies an action if it is legal.
///
/// It takes ownership of the action, even if the action is not applied, so the result of a
/// `gc_action_*` function can be passed directly. Returns whether the action was applied.
///
/// # Safety
///
/// `action` must be `NULL` or have been returned by one of the `gc_action_*` functions. It must
/// not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn gc_apply(
    game_controller: Option<&mut GameController>,
    action: *mut VAction,
    source: ActionSource,
) -> bool {
    if action.is_null() {
        set_last_error("action must not be NULL");
        return false;
    }
    let action = unsafe { Box::from_raw(action) };
    let Some(game_controller) = non_null(game_controller, "game_controller") else {
        return false;
    };
    game_controller.apply(*action, source)
}

/// This function checks whether an action is currently legal.
///
/// In contrast to [gc_apply], it does not take ownership of the action.
#[no_mangle]
pub extern "C" fn gc_is_legal(
    game_controller: Option<&mut GameController>,
    action: Option<&VAction>,
) -> bool {
    let (Some(game_controller), Some(action)) = (
        non_null(game_controller, "game_controller"),
        non_null(action, "action"),
    ) else {
        return false;
    };
    action.is_legal(&game_controller.get_context(false))
}

/// This function destroys an action that has not been passed to [gc_apply].
///
/// # Safety
///
/// `action` must be `NULL` or have been returned by one of the `gc_action_*` functions. It must
/// not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn gc_action_destroy(action: *mut VAction) {
    if !action.is_null() {
        drop(unsafe { Box::from_raw(action) });
    }
}

/// This function creates an [AddAdditionalTime] action.
#[no_mangle]
pub extern "C" fn gc_action_add_additional_time() -> *mut VAction {
    new_action(VAction::AddAdditionalTime(AddAdditionalTime))
}

/// This function creates a [FinishHalf] action.
#[no_mangle]
pub extern "C" fn gc_action_finish_half() -> *mut VAction {
    new_action(VAction::FinishHalf(FinishHalf))
}

/// This function creates a [FinishPenaltyShot] action.
#[no_mangle]
pub extern "C" fn gc_action_finish_penalty_shot() -> *mut VAction {
    new_action(VAction::FinishPenaltyShot(FinishPenaltyShot))
}

/// This function creates a [FinishSetPlay] action.
#[no_mangle]
pub extern "C" fn gc_action_finish_set_play() -> *mut VAction {
    new_action(VAction::FinishSetPlay(FinishSetPlay))
}

/// This function creates a [FreePenaltyShot] action.
#[no_mangle]
pub extern "C" fn gc_action_free_penalty_shot() -> *mut VAction {
    new_action(VAction::FreePenaltyShot(FreePenaltyShot))
}

/// This function creates a [FreeSetPlay] action.
#[no_mangle]
pub extern "C" fn gc_action_free_set_play() -> *mut VAction {
    new_action(VAction::FreeSetPlay(FreeSetPlay))
}

/// This function creates a [GlobalGameStuck] action.
#[no_mangle]
pub extern "C" fn gc_action_global_game_stuck() -> *mut VAction {
    new_action(VAction::GlobalGameStuck(GlobalGameStuck))
}

/// This function creates a [Goal] action.
#[no_mangle]
pub extern "C" fn gc_action_goal(side: Side) -> *mut VAction {
    new_action(VAction::Goal(Goal { side }))
}

/// This function creates a [Penalize] action.
///
/// Returns `NULL` if `player` is not a valid player number.
#[no_mangle]
pub extern "C" fn gc_action_penalize(side: Side, player: u8, call: PenaltyCall) -> *mut VAction {
    player_number(player).map_or(null_mut(), |player| {
        new_action(VAction::Penalize(Penalize { side, player, call }))
    })
}

/// This function creates a [SelectGoalkeeper] action.
///
/// Returns `NULL` if `player` is not a valid player number.
#[no_mangle]
pub extern "C" fn gc_action_select_goalkeeper(side: Side, player: u8) -> *mut VAction {
    player_number(player).map_or(null_mut(), |player| {
        new_action(VAction::SelectGoalkeeper(SelectGoalkeeper { side, player }))
    })
}

/// This function creates a [SelectPenaltyShotPlayer] action.
///
/// Returns `NULL` if `player` is not a valid player number.
#[no_mangle]
pub extern "C" fn gc_action_select_penalty_shot_player(
    side: Side,
    player: u8,
    goalkeeper: bool,
) -> *mut VAction {
    player_number(player).map_or(null_mut(), |player| {
        new_action(VAction::SelectPenaltyShotPlayer(SelectPenaltyShotPlayer {
            side,
            player,
            goalkeeper,
        }))
    })
}

/// This function creates a [StartExtraTime] action.
#[no_mangle]
pub extern "C" fn gc_action_start_extra_time() -> *mut VAction {
    new_action(VAction::StartExtraTime(StartExtraTime))
}

/// This function creates a [StartPenaltyShootout] action.
#[no_mangle]
pub extern "C" fn gc_action_start_penalty_shootout(sides: SideMapping) -> *mut VAction {
    new_action(VAction::StartPenaltyShootout(StartPenaltyShootout {
        sides,
    }))
}

/// This function creates a [StartSetPlay] action.
///
/// `side` may be `NULL` for set plays that are not awarded to a specific team.
#[no_mangle]
pub extern "C" fn gc_action_start_set_play(side: Option<&Side>, set_play: SetPlay) -> *mut VAction {
    new_action(VAction::StartSetPlay(StartSetPlay {
        side: side.copied(),
        set_play,
    }))
}

/// This function creates a [StopPlay] action.
#[no_mangle]
pub extern "C" fn gc_action_stop_play(resume: bool) -> *mut VAction {
    new_action(VAction::StopPlay(StopPlay { resume }))
}

/// This function creates a [Substitute] action.
///
/// Returns `NULL` if `player_out` or `player_in` is not a valid player number.
#[no_mangle]
pub extern "C" fn gc_action_substitute(side: Side, player_out: u8, player_in: u8) -> *mut VAction {
    match (player_number(player_out), player_number(player_in)) {
        (Some(player_out), Some(player_in)) => new_action(VAction::Substitute(Substitute {
            side,
            player_out,
            player_in,
        })),
        _ => null_mut(),
    }
}

/// This function creates a [SwitchHalf] action.
#[no_mangle]
pub extern "C" fn gc_action_switch_half() -> *mut VAction {
    new_action(VAction::SwitchHalf(SwitchHalf))
}

/// This function creates a [TeamMessage] action.
#[no_mangle]
pub extern "C" fn gc_action_team_message(side: Side, illegal: bool) -> *mut VAction {
    new_action(VAction::TeamMessage(TeamMessage { side, illegal }))
}

/// This function creates a [Timeout] action.
///
/// `side` is `NULL` for a referee timeout.
#[no_mangle]
pub extern "C" fn gc_action_timeout(side: Option<&Side>) -> *mut VAction {
    new_action(VAction::Timeout(Timeout {
        side: side.copied(),
    }))
}

/// This function creates an [Undo] action.
#[no_mangle]
pub extern "C" fn gc_action_undo(states: u32) -> *mut VAction {
    new_action(VAction::Undo(Undo { states }))
}

/// This function creates an [Unpenalize] action.
///
/// Returns `NULL` if `player` is not a valid player number.
#[no_mangle]
pub extern "C" fn gc_action_unpenalize(side: Side, player: u8, force: bool) -> *mut VAction {
    player_number(player).map_or(null_mut(), |player| {
        new_action(VAction::Unpenalize(Unpenalize {
            side,
            player,
            force,
        }))
    })
}

/// This function creates a [WaitForPenaltyShot] action.
#[no_mangle]
pub extern "C" fn gc_action_wait_for_penalty_shot() -> *mut VAction {
    new_action(VAction::WaitForPenaltyShot(WaitForPenaltyShot))
}

/// This function creates a [WaitForSetPlay] action.
#[no_mangle]
pub extern "C" fn gc_action_wait_for_set_play() -> *mut VAction {
    new_action(VAction::WaitForSetPlay(WaitForSetPlay))
}

/// This function writes the game state as a control message into a buffer.
///
/// The buffer receives a `RoboCupGameControlData` struct (see `RoboCupGameControlData.h`), so
/// `len` must be at least `sizeof(RoboCupGameControlData)`. If `true_data` is set, the true game
/// state is written (as sent to monitors), otherwise the possibly delayed state that is sent to
/// the players. Returns `false` if the buffer is too small or an argument is `NULL`.
///
/// # Safety
///
/// `data` must be `NULL` or point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn gc_read(
    game_controller: Option<&mut GameController>,
    packet_number: u8,
    true_data: bool,
    data: *mut u8,
    len: usize,
) -> bool {
    let Some(game_controller) = non_null(game_controller, "game_controller") else {
        return false;
    };
    if data.is_null() || len < CONTROL_MESSAGE_SIZE {
        set_last_error(format!(
            "data must point to at least {CONTROL_MESSAGE_SIZE} bytes"
        ));
        return false;
    }
    let bytes: Bytes = ControlMessage::new(
        game_controller.get_game(!true_data),
        &game_controller.params,
        packet_number,
        true_data,
    )
    .into();
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
    }
    true
}

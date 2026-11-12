//! This module defines the list of actions whose legality the UI subscribes to, and typed accessors
//! into the legality mask that the runtime returns for that list.
//!
//! The UI never evaluates rules itself. It declares a fixed, ordered list of every button's action
//! once ([declared_actions]), and each [game_controller_runtime::UiState] contains one bit per
//! declared action in the same order. The index functions in this module must therefore match the
//! order in which [declared_actions] pushes the actions (a unit test checks this).

use game_controller_core::{
    action::VAction,
    actions::{
        AddAdditionalTime, FinishHalf, FinishPenaltyShot, FinishSetPlay, FreePenaltyShot,
        FreeSetPlay, GlobalGameStuck, Goal, Penalize, StartExtraTime, StartPenaltyShootout,
        StartSetPlay, StopPlay, SwitchHalf, Timeout, Undo, Unpenalize, WaitForPenaltyShot,
        WaitForSetPlay,
    },
    types::{PenaltyCall, PlayerNumber, SetPlay, Side, SideMapping},
};

/// The penalty calls that can be selected in the center panel, with their button labels.
pub const PENALTIES: [(&str, PenaltyCall); 12] = [
    ("Pushing", PenaltyCall::Pushing),
    ("Incapable Robot", PenaltyCall::IncapableRobot),
    ("Leaving the Field", PenaltyCall::LeavingTheField),
    ("Illegal Position", PenaltyCall::IllegalPosition),
    ("Motion in Set", PenaltyCall::MotionInSet),
    ("Motion in Stop", PenaltyCall::MotionInStop),
    ("Ball Holding", PenaltyCall::BallHolding),
    ("Local Game Stuck", PenaltyCall::LocalGameStuck),
    ("Pick-up", PenaltyCall::RequestForPickUp),
    ("Arms / Hands", PenaltyCall::PlayingWithArmsHands),
    ("Yellow Card", PenaltyCall::Caution),
    ("Red Card", PenaltyCall::SendOff),
];

/// The number of player slots per team.
pub const NUM_OF_PLAYERS: usize = (PlayerNumber::MAX - PlayerNumber::MIN + 1) as usize;

/// The number of undo buttons.
pub const NUM_OF_UNDO_ACTIONS: usize = 5;

const SIDES: [Side; 2] = [Side::Home, Side::Away];

/// The buttons of a team panel that apply an action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeamAction {
    Timeout,
    Goal,
    DirectFreeKick,
    IndirectFreeKick,
    PenaltyKick,
    ThrowIn,
    GoalKick,
    CornerKick,
}

impl TeamAction {
    const ALL: [TeamAction; 8] = [
        TeamAction::Timeout,
        TeamAction::Goal,
        TeamAction::DirectFreeKick,
        TeamAction::IndirectFreeKick,
        TeamAction::PenaltyKick,
        TeamAction::ThrowIn,
        TeamAction::GoalKick,
        TeamAction::CornerKick,
    ];

    /// This function returns the set play started by this button, if it is a set play button.
    pub fn set_play(self) -> Option<SetPlay> {
        match self {
            TeamAction::Timeout | TeamAction::Goal => None,
            TeamAction::DirectFreeKick => Some(SetPlay::DirectFreeKick),
            TeamAction::IndirectFreeKick => Some(SetPlay::IndirectFreeKick),
            TeamAction::PenaltyKick => Some(SetPlay::PenaltyKick),
            TeamAction::ThrowIn => Some(SetPlay::ThrowIn),
            TeamAction::GoalKick => Some(SetPlay::GoalKick),
            TeamAction::CornerKick => Some(SetPlay::CornerKick),
        }
    }

    /// This function returns the action that this button applies for a given side.
    pub fn action(self, side: Side) -> VAction {
        match self {
            TeamAction::Timeout => VAction::Timeout(Timeout { side: Some(side) }),
            TeamAction::Goal => VAction::Goal(Goal { side }),
            _ => VAction::StartSetPlay(StartSetPlay {
                side: Some(side),
                set_play: self.set_play().unwrap(),
            }),
        }
    }
}

/// The buttons of the center panel that apply an action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameAction {
    SwitchHalf,
    StartExtraTime,
    StartPenaltyShootoutLeft,
    StartPenaltyShootoutRight,
    WaitForPenaltyShot,
    WaitForSetPlay,
    FreePenaltyShot,
    FinishSetPlay,
    FreeSetPlay,
    FinishPenaltyShot,
    FinishHalf,
    StartKickOffNone,
    StartKickOffHome,
    StartKickOffAway,
    AddAdditionalTime,
    RefereeTimeout,
    GlobalGameStuck,
    StopPlay,
    ResumePlay,
}

impl GameAction {
    const ALL: [GameAction; 19] = [
        GameAction::SwitchHalf,
        GameAction::StartExtraTime,
        GameAction::StartPenaltyShootoutLeft,
        GameAction::StartPenaltyShootoutRight,
        GameAction::WaitForPenaltyShot,
        GameAction::WaitForSetPlay,
        GameAction::FreePenaltyShot,
        GameAction::FinishSetPlay,
        GameAction::FreeSetPlay,
        GameAction::FinishPenaltyShot,
        GameAction::FinishHalf,
        GameAction::StartKickOffNone,
        GameAction::StartKickOffHome,
        GameAction::StartKickOffAway,
        GameAction::AddAdditionalTime,
        GameAction::RefereeTimeout,
        GameAction::GlobalGameStuck,
        GameAction::StopPlay,
        GameAction::ResumePlay,
    ];

    /// This function returns the kick-off action for a given kicking side.
    pub fn kick_off(side: Option<Side>) -> Self {
        match side {
            None => GameAction::StartKickOffNone,
            Some(Side::Home) => GameAction::StartKickOffHome,
            Some(Side::Away) => GameAction::StartKickOffAway,
        }
    }

    /// This function returns the action that this button applies.
    pub fn action(self) -> VAction {
        match self {
            GameAction::SwitchHalf => VAction::SwitchHalf(SwitchHalf),
            GameAction::StartExtraTime => VAction::StartExtraTime(StartExtraTime),
            GameAction::StartPenaltyShootoutLeft => {
                VAction::StartPenaltyShootout(StartPenaltyShootout {
                    sides: SideMapping::HomeDefendsLeftGoal,
                })
            }
            GameAction::StartPenaltyShootoutRight => {
                VAction::StartPenaltyShootout(StartPenaltyShootout {
                    sides: SideMapping::HomeDefendsRightGoal,
                })
            }
            GameAction::WaitForPenaltyShot => VAction::WaitForPenaltyShot(WaitForPenaltyShot),
            GameAction::WaitForSetPlay => VAction::WaitForSetPlay(WaitForSetPlay),
            GameAction::FreePenaltyShot => VAction::FreePenaltyShot(FreePenaltyShot),
            GameAction::FinishSetPlay => VAction::FinishSetPlay(FinishSetPlay),
            GameAction::FreeSetPlay => VAction::FreeSetPlay(FreeSetPlay),
            GameAction::FinishPenaltyShot => VAction::FinishPenaltyShot(FinishPenaltyShot),
            GameAction::FinishHalf => VAction::FinishHalf(FinishHalf),
            GameAction::StartKickOffNone => kick_off(None),
            GameAction::StartKickOffHome => kick_off(Some(Side::Home)),
            GameAction::StartKickOffAway => kick_off(Some(Side::Away)),
            GameAction::AddAdditionalTime => VAction::AddAdditionalTime(AddAdditionalTime),
            GameAction::RefereeTimeout => VAction::Timeout(Timeout { side: None }),
            GameAction::GlobalGameStuck => VAction::GlobalGameStuck(GlobalGameStuck),
            GameAction::StopPlay => VAction::StopPlay(StopPlay { resume: false }),
            GameAction::ResumePlay => VAction::StopPlay(StopPlay { resume: true }),
        }
    }
}

fn kick_off(side: Option<Side>) -> VAction {
    VAction::StartSetPlay(StartSetPlay {
        side,
        set_play: SetPlay::KickOff,
    })
}

const TEAM_ACTION_BASE: usize = 0;
const NUM_OF_TEAM_ACTIONS: usize = TeamAction::ALL.len();
const GAME_ACTION_BASE: usize = TEAM_ACTION_BASE + SIDES.len() * NUM_OF_TEAM_ACTIONS;
const NUM_OF_GAME_ACTIONS: usize = GameAction::ALL.len();
const PENALTY_ACTION_BASE: usize = GAME_ACTION_BASE + NUM_OF_GAME_ACTIONS;
// The + 2 are the unpenalize actions (without and with force).
const NUM_OF_PENALTY_ACTIONS: usize = SIDES.len() * NUM_OF_PLAYERS * (PENALTIES.len() + 2);
const UNDO_ACTION_BASE: usize = PENALTY_ACTION_BASE + NUM_OF_PENALTY_ACTIONS;

/// The total number of declared actions.
pub const NUM_OF_ACTIONS: usize = UNDO_ACTION_BASE + NUM_OF_UNDO_ACTIONS;

fn side_index(side: Side) -> usize {
    match side {
        Side::Home => 0,
        Side::Away => 1,
    }
}

fn team_action_index(side: Side, action: TeamAction) -> usize {
    TEAM_ACTION_BASE
        + side_index(side) * NUM_OF_TEAM_ACTIONS
        + TeamAction::ALL.iter().position(|a| *a == action).unwrap()
}

fn game_action_index(action: GameAction) -> usize {
    GAME_ACTION_BASE + GameAction::ALL.iter().position(|a| *a == action).unwrap()
}

/// `call` is an index into [PENALTIES], or [PENALTIES.len()] + force for unpenalize.
fn penalty_action_index(call: usize, side: Side, player: u8) -> usize {
    PENALTY_ACTION_BASE
        + call * SIDES.len() * NUM_OF_PLAYERS
        + side_index(side) * NUM_OF_PLAYERS
        + (player - PlayerNumber::MIN) as usize
}

fn undo_action_index(states: usize) -> usize {
    UNDO_ACTION_BASE + states - 1
}

/// This function returns the list of actions that the UI declares to the runtime.
pub fn declared_actions() -> Vec<VAction> {
    let mut actions = Vec::with_capacity(NUM_OF_ACTIONS);
    for side in SIDES {
        actions.extend(TeamAction::ALL.iter().map(|a| a.action(side)));
    }
    actions.extend(GameAction::ALL.iter().map(|a| a.action()));
    for (_, call) in PENALTIES {
        for side in SIDES {
            for player in PlayerNumber::all() {
                actions.push(VAction::Penalize(Penalize { side, player, call }));
            }
        }
    }
    for force in [false, true] {
        for side in SIDES {
            for player in PlayerNumber::all() {
                actions.push(VAction::Unpenalize(Unpenalize {
                    side,
                    player,
                    force,
                }));
            }
        }
    }
    for states in 1..=NUM_OF_UNDO_ACTIONS {
        actions.push(VAction::Undo(Undo {
            states: states as u32,
        }));
    }
    actions
}

/// A view into the legality mask of the declared actions.
#[derive(Clone, Copy)]
pub struct Legal<'a>(&'a [bool]);

impl<'a> Legal<'a> {
    /// This function wraps a legality mask, if it has the length of the declared action list.
    pub fn new(mask: &'a [bool]) -> Option<Self> {
        (mask.len() == NUM_OF_ACTIONS).then_some(Self(mask))
    }

    pub fn team(&self, side: Side, action: TeamAction) -> bool {
        self.0[team_action_index(side, action)]
    }

    pub fn game(&self, action: GameAction) -> bool {
        self.0[game_action_index(action)]
    }

    /// This function returns whether a penalty call is legal for any player.
    pub fn penalty_call(&self, call: usize) -> bool {
        let start = penalty_action_index(call, Side::Home, PlayerNumber::MIN);
        self.0[start..start + SIDES.len() * NUM_OF_PLAYERS]
            .iter()
            .any(|legal| *legal)
    }

    /// This function returns whether clicking a player is legal, given the selected penalty call
    /// (or unpenalizing if there is none).
    pub fn player(&self, side: Side, player: u8, call: Option<usize>, force: bool) -> bool {
        let call = call.unwrap_or(PENALTIES.len() + usize::from(force));
        self.0[penalty_action_index(call, side, player)]
    }

    /// This function returns whether undoing `states` (1-based) actions is legal.
    pub fn undo(&self, states: usize) -> bool {
        self.0[undo_action_index(states)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_match_declared_actions() {
        let actions = declared_actions();
        assert_eq!(actions.len(), NUM_OF_ACTIONS);
        for side in SIDES {
            for action in TeamAction::ALL {
                assert_eq!(
                    actions[team_action_index(side, action)],
                    action.action(side)
                );
            }
        }
        for action in GameAction::ALL {
            assert_eq!(actions[game_action_index(action)], action.action());
        }
        for (call_index, (_, call)) in PENALTIES.iter().enumerate() {
            for side in SIDES {
                for player in PlayerNumber::all() {
                    assert_eq!(
                        actions[penalty_action_index(call_index, side, player.into())],
                        VAction::Penalize(Penalize {
                            side,
                            player,
                            call: *call
                        })
                    );
                }
            }
        }
        for (force_index, force) in [false, true].into_iter().enumerate() {
            for side in SIDES {
                for player in PlayerNumber::all() {
                    assert_eq!(
                        actions[penalty_action_index(
                            PENALTIES.len() + force_index,
                            side,
                            player.into()
                        )],
                        VAction::Unpenalize(Unpenalize {
                            side,
                            player,
                            force
                        })
                    );
                }
            }
        }
        for states in 1..=NUM_OF_UNDO_ACTIONS {
            assert_eq!(
                actions[undo_action_index(states)],
                VAction::Undo(Undo {
                    states: states as u32
                })
            );
        }
    }
}

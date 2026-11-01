//! This module defines one Python class per action. All of them are subclasses of [Action], which
//! is what [crate::GameController::apply] and [crate::GameController::is_legal] take.

use pyo3::{prelude::*, PyClass};

use game_controller_core::action::VAction;
use game_controller_core::actions as core;

use crate::{player_number, PenaltyCall, SetPlay, Side, SideMapping};

/// This class is the base class of all actions. It can't be instantiated directly.
#[pyclass(frozen, module = "game_controller", subclass)]
pub struct Action {
    pub(crate) action: VAction,
}

#[pymethods]
impl Action {
    fn __repr__(&self) -> String {
        format!("{:?}", self.action)
    }
}

/// This function creates the initializer of an action class.
fn action<T: PyClass<BaseType = Action>>(subclass: T, action: VAction) -> PyClassInitializer<T> {
    PyClassInitializer::from(Action { action }).add_subclass(subclass)
}

/// This macro defines an action class whose constructor has no arguments.
macro_rules! unit_action {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[pyclass(extends = Action, frozen, module = "game_controller")]
        pub struct $name;

        #[pymethods]
        impl $name {
            #[new]
            fn new() -> PyClassInitializer<Self> {
                action(Self, VAction::$name(core::$name))
            }
        }
    };
}

unit_action!(
    /// This action adds a minute of additional time.
    AddAdditionalTime
);
unit_action!(
    /// This action corresponds to the referee call "Finish".
    FinishHalf
);
unit_action!(
    /// This action corresponds to the referee call "Finish" in a penalty shoot-out.
    FinishPenaltyShot
);
unit_action!(
    /// This action corresponds to the referee call "Ball Free" (the end of a set play).
    FinishSetPlay
);
unit_action!(
    /// This action corresponds to the referee call "Playing" in a penalty shoot-out.
    FreePenaltyShot
);
unit_action!(
    /// This action corresponds to the referee call "Playing".
    FreeSetPlay
);
unit_action!(
    /// This action corresponds to the referee call "Global Game Stuck".
    GlobalGameStuck
);
unit_action!(
    /// This action starts extra time.
    StartExtraTime
);
unit_action!(
    /// This action switches from the end of the first half to the beginning of the second half,
    /// including the switch of sides.
    SwitchHalf
);
unit_action!(
    /// This action corresponds to the referee call "Set" in a penalty shoot-out.
    WaitForPenaltyShot
);
unit_action!(
    /// This action corresponds to the referee call "Set".
    WaitForSetPlay
);

/// This action is for when a goal has been scored by a team.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct Goal;

#[pymethods]
impl Goal {
    #[new]
    fn new(side: Side) -> PyClassInitializer<Self> {
        action(Self, VAction::Goal(core::Goal { side: side.into() }))
    }
}

/// This action penalizes a player. Raises a ValueError if the player number is invalid.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct Penalize;

#[pymethods]
impl Penalize {
    #[new]
    fn new(side: Side, player: u8, call: PenaltyCall) -> PyResult<PyClassInitializer<Self>> {
        Ok(action(
            Self,
            VAction::Penalize(core::Penalize {
                side: side.into(),
                player: player_number(player)?,
                call: call.into(),
            }),
        ))
    }
}

/// This action selects the goalkeeper of a team. Raises a ValueError if the player number is
/// invalid.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct SelectGoalkeeper;

#[pymethods]
impl SelectGoalkeeper {
    #[new]
    fn new(side: Side, player: u8) -> PyResult<PyClassInitializer<Self>> {
        Ok(action(
            Self,
            VAction::SelectGoalkeeper(core::SelectGoalkeeper {
                side: side.into(),
                player: player_number(player)?,
            }),
        ))
    }
}

/// This action selects the player of a team (kicker or goalkeeper) for the next penalty shot.
/// Raises a ValueError if the player number is invalid.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct SelectPenaltyShotPlayer;

#[pymethods]
impl SelectPenaltyShotPlayer {
    #[new]
    fn new(side: Side, player: u8, goalkeeper: bool) -> PyResult<PyClassInitializer<Self>> {
        Ok(action(
            Self,
            VAction::SelectPenaltyShotPlayer(core::SelectPenaltyShotPlayer {
                side: side.into(),
                player: player_number(player)?,
                goalkeeper,
            }),
        ))
    }
}

/// This action starts a penalty shoot-out with the given side mapping.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct StartPenaltyShootout;

#[pymethods]
impl StartPenaltyShootout {
    #[new]
    fn new(sides: SideMapping) -> PyClassInitializer<Self> {
        action(
            Self,
            VAction::StartPenaltyShootout(core::StartPenaltyShootout {
                sides: sides.into(),
            }),
        )
    }
}

/// This action starts a set play. `side` is None for set plays that are not awarded to a specific
/// team.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct StartSetPlay;

#[pymethods]
impl StartSetPlay {
    #[new]
    #[pyo3(signature = (set_play, side = None))]
    fn new(set_play: SetPlay, side: Option<Side>) -> PyClassInitializer<Self> {
        action(
            Self,
            VAction::StartSetPlay(core::StartSetPlay {
                side: side.map(Into::into),
                set_play: set_play.into(),
            }),
        )
    }
}

/// This action stops play (or resumes it if `resume` is set), i.e. an (emergency) stop.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct StopPlay;

#[pymethods]
impl StopPlay {
    #[new]
    #[pyo3(signature = (resume = false))]
    fn new(resume: bool) -> PyClassInitializer<Self> {
        action(Self, VAction::StopPlay(core::StopPlay { resume }))
    }
}

/// This action substitutes a player by another one. Raises a ValueError if a player number is
/// invalid.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct Substitute;

#[pymethods]
impl Substitute {
    #[new]
    fn new(side: Side, player_out: u8, player_in: u8) -> PyResult<PyClassInitializer<Self>> {
        Ok(action(
            Self,
            VAction::Substitute(core::Substitute {
                side: side.into(),
                player_out: player_number(player_out)?,
                player_in: player_number(player_in)?,
            }),
        ))
    }
}

/// This action is triggered when a team message is received.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct TeamMessage;

#[pymethods]
impl TeamMessage {
    #[new]
    fn new(side: Side, illegal: bool) -> PyClassInitializer<Self> {
        action(
            Self,
            VAction::TeamMessage(core::TeamMessage {
                side: side.into(),
                illegal,
            }),
        )
    }
}

/// This action is for when a team or the referee takes a timeout. `side` is None for a referee
/// timeout.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct Timeout;

#[pymethods]
impl Timeout {
    #[new]
    #[pyo3(signature = (side = None))]
    fn new(side: Option<Side>) -> PyClassInitializer<Self> {
        action(
            Self,
            VAction::Timeout(core::Timeout {
                side: side.map(Into::into),
            }),
        )
    }
}

/// This action reverts the game to the state before the last `states` user actions.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct Undo;

#[pymethods]
impl Undo {
    #[new]
    fn new(states: u32) -> PyClassInitializer<Self> {
        action(Self, VAction::Undo(core::Undo { states }))
    }
}

/// This action unpenalizes a player. With `force`, this happens even if the penalty timer hasn't
/// expired. Raises a ValueError if the player number is invalid.
#[pyclass(extends = Action, frozen, module = "game_controller")]
pub struct Unpenalize;

#[pymethods]
impl Unpenalize {
    #[new]
    #[pyo3(signature = (side, player, force = false))]
    fn new(side: Side, player: u8, force: bool) -> PyResult<PyClassInitializer<Self>> {
        Ok(action(
            Self,
            VAction::Unpenalize(core::Unpenalize {
                side: side.into(),
                player: player_number(player)?,
                force,
            }),
        ))
    }
}

/// This function adds the action classes to the module.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Action>()?;
    module.add_class::<AddAdditionalTime>()?;
    module.add_class::<FinishHalf>()?;
    module.add_class::<FinishPenaltyShot>()?;
    module.add_class::<FinishSetPlay>()?;
    module.add_class::<FreePenaltyShot>()?;
    module.add_class::<FreeSetPlay>()?;
    module.add_class::<GlobalGameStuck>()?;
    module.add_class::<Goal>()?;
    module.add_class::<Penalize>()?;
    module.add_class::<SelectGoalkeeper>()?;
    module.add_class::<SelectPenaltyShotPlayer>()?;
    module.add_class::<StartExtraTime>()?;
    module.add_class::<StartPenaltyShootout>()?;
    module.add_class::<StartSetPlay>()?;
    module.add_class::<StopPlay>()?;
    module.add_class::<Substitute>()?;
    module.add_class::<SwitchHalf>()?;
    module.add_class::<TeamMessage>()?;
    module.add_class::<Timeout>()?;
    module.add_class::<Undo>()?;
    module.add_class::<Unpenalize>()?;
    module.add_class::<WaitForPenaltyShot>()?;
    module.add_class::<WaitForSetPlay>()?;
    Ok(())
}

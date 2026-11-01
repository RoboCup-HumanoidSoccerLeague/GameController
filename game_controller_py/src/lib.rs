//! This crate exposes the GameController (and an automatic referee) to Python.
//!
//! It is built with maturin as the module `game_controller`. Create [CompetitionParams] (once),
//! [Params] and a [GameController]. In each simulation step, advance the time with
//! [GameController::seek], optionally run [autoreferee::Autoreferee::update], and obtain the state
//! in the format of a control message with [GameController::control_message]. Actions are objects
//! of the classes in [actions] and are applied with [GameController::apply].

use std::time::Duration;

use bytes::Bytes;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyBytes};

use game_controller_core::log::NullLogger;
use game_controller_core::types::{self, GameParams, PlayerNumber};
use game_controller_msgs::ControlMessage;

mod actions;
mod autoreferee;

use actions::Action;

/// This macro defines a Python enum that mirrors an enum of the core (whose variants have the same
/// names), and conversions between both.
macro_rules! mirror_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident),* $(,)? }) => {
        $(#[$meta])*
        #[pyclass(
            eq,
            eq_int,
            frozen,
            from_py_object,
            hash,
            module = "game_controller",
            rename_all = "SCREAMING_SNAKE_CASE"
        )]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum $name {
            $($variant),*
        }

        impl From<$name> for types::$name {
            fn from(value: $name) -> Self {
                match value {
                    $($name::$variant => Self::$variant),*
                }
            }
        }

        impl From<types::$name> for $name {
            fn from(value: types::$name) -> Self {
                match value {
                    $(types::$name::$variant => Self::$variant),*
                }
            }
        }
    };
}

mirror_enum!(
    /// This enumerates the two opposing teams of the game (in the order of the schedule, not the
    /// sides of the field).
    Side { Home, Away }
);

mirror_enum!(
    /// This enumerates the possible assignments of teams to sides of the field of play.
    SideMapping {
        HomeDefendsLeftGoal,
        HomeDefendsRightGoal,
    }
);

mirror_enum!(
    /// This enumerates the set plays.
    SetPlay {
        NoSetPlay,
        KickOff,
        DirectFreeKick,
        IndirectFreeKick,
        PenaltyKick,
        ThrowIn,
        GoalKick,
        CornerKick,
    }
);

mirror_enum!(
    /// This enumerates the possible referee calls for penalties.
    PenaltyCall {
        IllegalPosition,
        MotionInSet,
        MotionInStop,
        LocalGameStuck,
        IncapableRobot,
        RequestForPickUp,
        BallHolding,
        LeavingTheField,
        PlayingWithArmsHands,
        Pushing,
        Caution,
        SendOff,
    }
);

mirror_enum!(
    /// This enumerates the jersey colors.
    Color {
        Blue,
        Red,
        Yellow,
        Black,
        White,
        Green,
        Orange,
        Purple,
        Brown,
        Gray,
    }
);

mirror_enum!(
    /// This enumerates the possible sources that can trigger actions.
    ActionSource {
        Network,
        Timer,
        User,
        Autoreferee,
    }
);

/// This function converts a player number from Python, or raises a ValueError if it is out of
/// range.
pub(crate) fn player_number(player: u8) -> PyResult<PlayerNumber> {
    if (PlayerNumber::MIN..=PlayerNumber::MAX).contains(&player) {
        Ok(PlayerNumber::new(player))
    } else {
        Err(PyValueError::new_err(format!(
            "invalid player number {player} (must be {}-{})",
            PlayerNumber::MIN,
            PlayerNumber::MAX
        )))
    }
}

/// This function converts a duration in seconds from Python, or raises a ValueError if it is
/// negative or not finite.
pub(crate) fn duration(seconds: f64) -> PyResult<Duration> {
    Duration::try_from_secs_f64(seconds)
        .map_err(|error| PyValueError::new_err(format!("invalid duration {seconds}: {error}")))
}

/// This class contains constant parameters for one team.
#[pyclass(frozen, get_all, module = "game_controller")]
pub struct TeamParams {
    /// The number which globally identifies the team within the league.
    number: u8,
    /// The jersey color of the field players.
    field_player_color: Color,
    /// The jersey color of the goalkeeper.
    goalkeeper_color: Color,
}

#[pymethods]
impl TeamParams {
    #[new]
    fn new(number: u8, field_player_color: Color, goalkeeper_color: Color) -> Self {
        Self {
            number,
            field_player_color,
            goalkeeper_color,
        }
    }
}

impl From<&TeamParams> for types::TeamParams {
    fn from(value: &TeamParams) -> Self {
        Self {
            number: value.number,
            field_player_color: value.field_player_color.into(),
            goalkeeper_color: value.goalkeeper_color.into(),
        }
    }
}

/// This class contains parameters that toggle behavior that is useful for testing.
#[pyclass(frozen, from_py_object, get_all, module = "game_controller")]
#[derive(Clone, Default)]
pub struct TestParams {
    /// Game state transitions are never delayed.
    no_delay: bool,
    /// It is allowed to start/continue a penalty shoot-out even though the game is actually
    /// decided.
    penalty_shootout: bool,
}

#[pymethods]
impl TestParams {
    #[new]
    #[pyo3(signature = (no_delay = false, penalty_shootout = false))]
    fn new(no_delay: bool, penalty_shootout: bool) -> Self {
        Self {
            no_delay,
            penalty_shootout,
        }
    }
}

/// This class contains the parameters of a competition.
///
/// They are given as YAML text (the content of a `params.yaml` file). Raises a ValueError if they
/// can't be parsed. The same object can be used to create the parameters of any number of games.
#[pyclass(frozen, module = "game_controller")]
pub struct CompetitionParams(types::CompetitionParams);

#[pymethods]
impl CompetitionParams {
    #[new]
    fn new(yaml: &str) -> PyResult<Self> {
        serde_yaml::from_str(yaml).map(Self).map_err(|error| {
            PyValueError::new_err(format!("could not parse competition params: {error}"))
        })
    }
}

/// This class contains the parameters of the competition and the game.
#[pyclass(frozen, module = "game_controller")]
pub struct Params(types::Params);

#[pymethods]
impl Params {
    #[new]
    #[pyo3(signature = (
        competition,
        home,
        away,
        kick_off_side,
        side_mapping,
        test_params = None,
    ))]
    fn new(
        competition: &CompetitionParams,
        home: &TeamParams,
        away: &TeamParams,
        kick_off_side: Side,
        side_mapping: SideMapping,
        test_params: Option<TestParams>,
    ) -> Self {
        let test = test_params.unwrap_or_default();
        Self(types::Params {
            competition: competition.0.clone(),
            game: GameParams {
                teams: enum_map::enum_map! {
                    types::Side::Home => home.into(),
                    types::Side::Away => away.into(),
                },
                kick_off_side: kick_off_side.into(),
                side_mapping: side_mapping.into(),
                test: types::TestParams {
                    no_delay: test.no_delay,
                    penalty_shootout: test.penalty_shootout,
                },
            },
        })
    }
}

/// This class is the GameController. The parameters are copied.
// The core GameController is not Sync (its logger is only Send), so objects of this class can only
// be used by the thread that created them.
#[pyclass(unsendable, module = "game_controller")]
pub struct GameController(game_controller_core::GameController);

#[pymethods]
impl GameController {
    #[new]
    fn new(params: &Params) -> Self {
        Self(game_controller_core::GameController::new(
            params.0.clone(),
            Box::new(NullLogger),
        ))
    }

    /// This method lets `dt` seconds pass.
    ///
    /// Timers are updated, and the actions of timers that expire in the meantime are applied.
    fn seek(&mut self, dt: f64) -> PyResult<()> {
        self.0.seek(duration(dt)?);
        Ok(())
    }

    /// This method applies an action if it is legal. Returns whether the action was applied.
    #[pyo3(signature = (action, source = ActionSource::User))]
    fn apply(&mut self, action: &Action, source: ActionSource) -> bool {
        self.0.apply(action.action.clone(), source.into())
    }

    /// This method checks whether an action is currently legal.
    fn is_legal(&mut self, action: &Action) -> bool {
        action.action.is_legal(&self.0.get_context(false))
    }

    /// This method returns the game state as a control message, i.e. the bytes of a
    /// `RoboCupGameControlData` struct (see `RoboCupGameControlData.h`).
    ///
    /// If `true_data` is set, the true game state is returned (as sent to monitors), otherwise the
    /// possibly delayed state that is sent to the players.
    #[pyo3(signature = (packet_number = 0, true_data = false))]
    fn control_message<'py>(
        &self,
        py: Python<'py>,
        packet_number: u8,
        true_data: bool,
    ) -> Bound<'py, PyBytes> {
        let bytes: Bytes = ControlMessage::new(
            self.0.get_game(!true_data),
            &self.0.params,
            packet_number,
            true_data,
        )
        .into();
        PyBytes::new(py, &bytes)
    }
}

/// The GameController for RoboCup soccer: parameters, the game state machine, actions, and an
/// automatic referee for simulated games.
#[pymodule]
fn game_controller(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Side>()?;
    module.add_class::<SideMapping>()?;
    module.add_class::<SetPlay>()?;
    module.add_class::<PenaltyCall>()?;
    module.add_class::<Color>()?;
    module.add_class::<ActionSource>()?;
    module.add_class::<TeamParams>()?;
    module.add_class::<TestParams>()?;
    module.add_class::<CompetitionParams>()?;
    module.add_class::<Params>()?;
    module.add_class::<GameController>()?;
    actions::register(module)?;
    autoreferee::register(module)?;
    Ok(())
}

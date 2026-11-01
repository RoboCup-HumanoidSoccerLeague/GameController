//! This module exposes the automatic referee from [game_controller_autoreferee] to Python.
//!
//! In each simulation step, the caller should call [crate::GameController::seek], then
//! [Autoreferee::update] (and execute the returned commands). [Autoreferee::ball_contact] should be
//! called whenever a player touches the ball. See [game_controller_autoreferee] for the coordinate
//! system (lengths are in meters, angles in radians).

use std::collections::HashMap;

use enum_map::EnumMap;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyDict};

use game_controller_autoreferee::{Config, Features, PlayerPose as CorePlayerPose, World};

use crate::{duration, player_number, GameController, Side};

/// This function creates the Python class `Features`, an `enum.IntFlag` with the same members as
/// [Features] (plus `ALL`), so that both can't get out of sync.
fn features_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let members = PyDict::new(py);
    for (name, flag) in Features::all().iter_names() {
        members.set_item(name, flag.bits())?;
    }
    members.set_item("ALL", Features::all().bits())?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("module", "game_controller")?;
    let class = py
        .import("enum")?
        .getattr("IntFlag")?
        .call(("Features", members), Some(&kwargs))?;
    class.setattr(
        "__doc__",
        "This class represents the features that the automatic referee should provide. See the \
         documentation of the Rust crate game_controller_autoreferee for their meaning.",
    )?;
    Ok(class)
}

/// This function converts features from Python, or raises a ValueError if there are unknown bits.
fn features(bits: u32) -> PyResult<Features> {
    Features::from_bits(bits)
        .ok_or_else(|| PyValueError::new_err(format!("unknown features in {bits:#x}")))
}

/// This macro defines the class `FieldDimensions` (whose fields all are floats and which has a
/// constructor that takes them as keyword arguments) and its conversion to the dimensions of the
/// automatic referee from a single list of fields.
macro_rules! field_dimensions {
    ($($(#[$meta:meta])* $field:ident),* $(,)?) => {
        /// This class describes the dimensions of the field. The letters refer to the rule book.
        #[pyclass(frozen, get_all, module = "game_controller")]
        pub struct FieldDimensions {
            $($(#[$meta])* $field: f32),*
        }

        #[pymethods]
        impl FieldDimensions {
            #[new]
            #[pyo3(signature = (*, $($field),*))]
            #[allow(clippy::too_many_arguments)]
            fn new($($field: f32),*) -> Self {
                Self { $($field),* }
            }
        }

        impl From<&FieldDimensions> for game_controller_autoreferee::FieldDimensions {
            fn from(value: &FieldDimensions) -> Self {
                Self { $($field: value.$field),* }
            }
        }
    };
}

field_dimensions!(
    /// The length of the field (A).
    length,
    /// The width of the field (B).
    width,
    /// The width of the goal (D).
    goal_width,
    /// The height of the goal.
    goal_height,
    /// The length of the goal area (E).
    goal_area_length,
    /// The width of the goal area (F).
    goal_area_width,
    /// The length of the penalty area (G).
    penalty_area_length,
    /// The width of the penalty area (H).
    penalty_area_width,
    /// The distance of the penalty mark from the goal line (I).
    penalty_mark_distance,
    /// The radius of the center circle (J/2).
    center_circle_radius,
    /// The width of the border strip (K).
    border_strip_width,
    /// The radius of the penalty marks / center mark.
    mark_radius,
    /// The width of the field lines.
    line_width,
    /// The radius of the ball.
    ball_radius,
);

/// This class describes the pose of a player on the field.
#[pyclass(frozen, get_all, module = "game_controller")]
pub struct PlayerPose {
    /// The x-coordinate of the player.
    x: f32,
    /// The y-coordinate of the player.
    y: f32,
    /// The rotation of the player around the z-axis.
    theta: f32,
    /// The approximate radius at ground level of the player when standing upright.
    radius: f32,
}

#[pymethods]
impl PlayerPose {
    #[new]
    fn new(x: f32, y: f32, theta: f32, radius: f32) -> Self {
        Self {
            x,
            y,
            theta,
            radius,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PlayerPose(x={}, y={}, theta={}, radius={})",
            self.x, self.y, self.theta, self.radius
        )
    }
}

/// This enumerates the commands that the simulator should execute.
#[pyclass(module = "game_controller")]
pub enum Command {
    /// Move the ball to a given position and reset its velocity.
    PlaceBall { x: f32, y: f32, z: f32 },
    /// Move a player to a given upright pose.
    PlacePlayer {
        side: Side,
        player: u8,
        x: f32,
        y: f32,
        theta: f32,
    },
    /// Notify the players that the whistle has been blown.
    Whistle {},
}

#[pymethods]
impl Command {
    fn __repr__(&self) -> String {
        match self {
            Self::PlaceBall { x, y, z } => format!("Command.PlaceBall(x={x}, y={y}, z={z})"),
            Self::PlacePlayer {
                side,
                player,
                x,
                y,
                theta,
            } => format!(
                "Command.PlacePlayer(side=Side.{}, player={player}, x={x}, y={y}, theta={theta})",
                match side {
                    Side::Home => "HOME",
                    Side::Away => "AWAY",
                }
            ),
            Self::Whistle {} => "Command.Whistle()".to_string(),
        }
    }
}

impl From<game_controller_autoreferee::Command> for Command {
    fn from(value: game_controller_autoreferee::Command) -> Self {
        match value {
            game_controller_autoreferee::Command::PlaceBall {
                position: [x, y, z],
            } => Self::PlaceBall { x, y, z },
            game_controller_autoreferee::Command::PlacePlayer {
                side,
                player,
                x,
                y,
                theta,
            } => Self::PlacePlayer {
                side: side.into(),
                player: player.into(),
                x,
                y,
                theta,
            },
            game_controller_autoreferee::Command::Whistle => Self::Whistle {},
        }
    }
}

/// This class is the automatic referee. Times are given in seconds.
#[pyclass(module = "game_controller")]
pub struct Autoreferee {
    autoreferee: game_controller_autoreferee::Autoreferee,
    features: Features,
}

#[pymethods]
impl Autoreferee {
    #[new]
    fn new(
        features: u32,
        field: &FieldDimensions,
        time_until_switch_to_set: f64,
        time_until_switch_to_playing: f64,
        penalty_placement_distance: f32,
    ) -> PyResult<Self> {
        let features = self::features(features)?;
        Ok(Self {
            autoreferee: game_controller_autoreferee::Autoreferee::new(Config {
                features,
                field: field.into(),
                time_until_switch_to_set: duration(time_until_switch_to_set)?,
                time_until_switch_to_playing: duration(time_until_switch_to_playing)?,
                penalty_placement_distance,
            }),
            features,
        })
    }

    /// The features that are enabled. Changing them keeps the rest of the state.
    #[getter]
    fn get_features<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        py.import("game_controller")?
            .getattr("Features")?
            .call1((self.features.bits(),))
    }

    #[setter]
    fn set_features(&mut self, features: u32) -> PyResult<()> {
        self.features = self::features(features)?;
        self.autoreferee.set_features(self.features);
        Ok(())
    }

    /// This method runs the automatic referee for one step and returns the commands that the
    /// simulator should execute.
    ///
    /// It may apply actions to the GameController. It should be called after
    /// `GameController.seek` in each simulation step. `ball` is the position of the ball, and
    /// `players` maps (side, player number) to the poses of the players that are physically
    /// present. Raises a ValueError if a player number is invalid.
    fn update(
        &mut self,
        mut game_controller: PyRefMut<'_, GameController>,
        ball: [f32; 3],
        players: HashMap<(Side, u8), PyRef<'_, PlayerPose>>,
    ) -> PyResult<Vec<Command>> {
        let mut world = World {
            ball,
            players: EnumMap::default(),
        };
        for ((side, player), pose) in players {
            let side: game_controller_core::types::Side = side.into();
            let index = usize::from(u8::from(player_number(player)?)) - 1;
            world.players[side][index] = Some(CorePlayerPose {
                x: pose.x,
                y: pose.y,
                theta: pose.theta,
                radius: pose.radius,
            });
        }
        Ok(self
            .autoreferee
            .update(&mut game_controller.0, &world)
            .into_iter()
            .map(Command::from)
            .collect())
    }

    /// This method notifies the automatic referee that a player touched the ball. It should be
    /// called in every simulation step in which the player is in contact with the ball. Raises a
    /// ValueError if the player number is invalid.
    fn ball_contact(
        &mut self,
        mut game_controller: PyRefMut<'_, GameController>,
        side: Side,
        player: u8,
    ) -> PyResult<()> {
        self.autoreferee
            .ball_contact(&mut game_controller.0, side.into(), player_number(player)?);
        Ok(())
    }
}

/// This function adds the classes of the automatic referee to the module.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("Features", features_class(module.py())?)?;
    module.add_class::<FieldDimensions>()?;
    module.add_class::<PlayerPose>()?;
    module.add_class::<Command>()?;
    module.add_class::<Autoreferee>()?;
    Ok(())
}

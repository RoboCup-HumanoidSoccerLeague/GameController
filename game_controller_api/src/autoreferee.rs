//! This module exposes the automatic referee from [game_controller_autoreferee] to C.
//!
//! In each simulation step, the caller should call [crate::gc_seek], then
//! [gc_autoreferee_update], and then [crate::gc_read]. [gc_autoreferee_ball_contact] should be
//! called whenever a player touches the ball. See [game_controller_autoreferee] for the coordinate
//! system (lengths are in meters, angles in radians).

use std::{ffi::c_void, ptr::null_mut, time::Duration};

use enum_map::EnumMap;

use game_controller_autoreferee::{Command, Config, Features, FieldDimensions, PlayerPose, World};
use game_controller_core::{types::Side, GameController};

use crate::{error::player_number, non_null};

bitflags::bitflags! {
    /// This struct represents the features that the automatic referee should provide. The bits
    /// are the same as in [game_controller_autoreferee::Features].
    #[repr(C)]
    pub struct AutorefereeFeatures: u32 {
        /// Place the ball for set plays (including kick-off, penalty kick, penalty shoot-out).
        const PLACE_BALL = 1 << 0;
        /// Check whether the ball went out of bounds and do the appropriate action (depending on
        /// where and who touched last), e.g. goal, throw-in, goal kick, corner kick.
        const BALL_OUT = 1 << 1;
        /// Finish set plays early when the ball is touched by the kicking team.
        const BALL_FREE = 1 << 2;
        /// Switch to Set early if players didn't move sufficiently long.
        const SWITCH_TO_SET = 1 << 3;
        /// Switch to Playing after some time in Set.
        const SWITCH_TO_PLAYING = 1 << 4;
        /// Switch to Finished when the time is up.
        const SWITCH_TO_FINISHED = 1 << 5;
        /// Penalize players that are leaving the field.
        const PENALIZE_LEAVING_THE_FIELD = 1 << 6;
        /// Penalize players that are illegally positioned.
        const PENALIZE_ILLEGAL_POSITIONING = 1 << 7;
        /// Place players that are penalized.
        const PLACE_FOR_PENALTY = 1 << 8;
        /// Start the penalty timers of penalized players right away and unpenalize them when
        /// their timers are up.
        const UNPENALIZE = 1 << 9;
        /// Place players in a penalty shoot-out.
        const PLACE_FOR_PENALTY_SHOOT_OUT = 1 << 10;
    }
}

// Both sets of flags must be kept in sync.
const _: () = assert!(AutorefereeFeatures::all().bits() == Features::all().bits());

/// This struct describes the dimensions of the field. The letters refer to the rule book.
#[repr(C)]
pub struct AutorefereeFieldDimensions {
    /// The length of the field (A).
    pub length: f32,
    /// The width of the field (B).
    pub width: f32,
    /// The width of the goal (D).
    pub goal_width: f32,
    /// The height of the goal.
    pub goal_height: f32,
    /// The length of the goal area (E).
    pub goal_area_length: f32,
    /// The width of the goal area (F).
    pub goal_area_width: f32,
    /// The length of the penalty area (G).
    pub penalty_area_length: f32,
    /// The width of the penalty area (H).
    pub penalty_area_width: f32,
    /// The distance of the penalty mark from the goal line (I).
    pub penalty_mark_distance: f32,
    /// The radius of the center circle (J/2).
    pub center_circle_radius: f32,
    /// The width of the border strip (K).
    pub border_strip_width: f32,
    /// The radius of the penalty marks / center mark.
    pub mark_radius: f32,
    /// The width of the field lines.
    pub line_width: f32,
    /// The radius of the ball.
    pub ball_radius: f32,
}

/// This struct contains the configuration of the automatic referee, including the callbacks with
/// which it executes commands in the simulator. Callbacks may be null.
#[repr(C)]
pub struct AutorefereeConfig {
    /// The features that are enabled.
    pub features: AutorefereeFeatures,
    /// The dimensions of the field.
    pub field: AutorefereeFieldDimensions,
    /// The time (in milliseconds) in Ready after which the automatic referee switches to Set if no
    /// player moved for that long.
    pub time_until_switch_to_set: u64,
    /// The time (in milliseconds) in Set after which the automatic referee switches to Playing.
    pub time_until_switch_to_playing: u64,
    /// The minimum distance between players that are placed for a penalty.
    pub penalty_placement_distance: f32,
    /// An opaque value that is passed to the callbacks (e.g. a this-pointer).
    pub userptr: *mut c_void,
    /// A function that moves the ball to a given position and resets its velocity.
    pub place_ball: Option<extern "C" fn(userptr: *mut c_void, x: f32, y: f32, z: f32)>,
    /// A function that moves a player to a given upright pose.
    pub place_player: Option<
        extern "C" fn(userptr: *mut c_void, side: Side, player: u8, x: f32, y: f32, theta: f32),
    >,
    /// A function that notifies the players that the whistle has been blown.
    pub whistle: Option<extern "C" fn(userptr: *mut c_void)>,
}

/// This struct describes the state of a player as observed by the simulator.
#[repr(C)]
pub struct AutorefereePlayer {
    /// Whether the player is physically present (otherwise the other fields are ignored).
    pub present: bool,
    /// The x-coordinate of the player.
    pub x: f32,
    /// The y-coordinate of the player.
    pub y: f32,
    /// The rotation of the player around the z-axis.
    pub theta: f32,
    /// The approximate radius at ground level of the player when standing upright.
    pub radius: f32,
}

/// This struct describes the state of the world as observed by the simulator.
#[repr(C)]
pub struct AutorefereeWorld {
    /// The position of the ball.
    pub ball: [f32; 3],
    /// The players, indexed by side (0 = home, 1 = away) and player number - 1. Unfortunately, the
    /// number of players must be written as a literal here.
    pub players: [[AutorefereePlayer; 20]; 2],
}

/// This struct is an opaque handle to an automatic referee.
pub struct Autoreferee {
    autoreferee: game_controller_autoreferee::Autoreferee,
    userptr: *mut c_void,
    place_ball: Option<extern "C" fn(*mut c_void, f32, f32, f32)>,
    place_player: Option<extern "C" fn(*mut c_void, Side, u8, f32, f32, f32)>,
    whistle: Option<extern "C" fn(*mut c_void)>,
}

/// This function creates an automatic referee.
///
/// The configuration is copied. The caller takes ownership of the result and must release it with
/// [gc_autoreferee_destroy]. Returns `NULL` on error.
#[no_mangle]
pub extern "C" fn gc_autoreferee_new(config: Option<&AutorefereeConfig>) -> *mut Autoreferee {
    let Some(config) = non_null(config, "config") else {
        return null_mut();
    };
    let field = &config.field;
    Box::into_raw(Box::new(Autoreferee {
        autoreferee: game_controller_autoreferee::Autoreferee::new(Config {
            features: Features::from_bits_truncate(config.features.bits()),
            field: FieldDimensions {
                length: field.length,
                width: field.width,
                goal_width: field.goal_width,
                goal_height: field.goal_height,
                goal_area_length: field.goal_area_length,
                goal_area_width: field.goal_area_width,
                penalty_area_length: field.penalty_area_length,
                penalty_area_width: field.penalty_area_width,
                penalty_mark_distance: field.penalty_mark_distance,
                center_circle_radius: field.center_circle_radius,
                border_strip_width: field.border_strip_width,
                mark_radius: field.mark_radius,
                line_width: field.line_width,
                ball_radius: field.ball_radius,
            },
            time_until_switch_to_set: Duration::from_millis(config.time_until_switch_to_set),
            time_until_switch_to_playing: Duration::from_millis(
                config.time_until_switch_to_playing,
            ),
            penalty_placement_distance: config.penalty_placement_distance,
        }),
        userptr: config.userptr,
        place_ball: config.place_ball,
        place_player: config.place_player,
        whistle: config.whistle,
    }))
}

/// This function destroys an automatic referee.
///
/// # Safety
///
/// `autoreferee` must be `NULL` or have been returned by [gc_autoreferee_new]. It must not be used
/// afterwards.
#[no_mangle]
pub unsafe extern "C" fn gc_autoreferee_destroy(autoreferee: *mut Autoreferee) {
    if !autoreferee.is_null() {
        drop(unsafe { Box::from_raw(autoreferee) });
    }
}

/// This function changes the enabled features of an automatic referee.
///
/// The rest of its state is kept.
#[no_mangle]
pub extern "C" fn gc_autoreferee_set_features(
    autoreferee: Option<&mut Autoreferee>,
    features: AutorefereeFeatures,
) {
    if let Some(autoreferee) = non_null(autoreferee, "autoreferee") {
        autoreferee
            .autoreferee
            .set_features(Features::from_bits_truncate(features.bits()));
    }
}

/// This function runs the automatic referee for one step.
///
/// It may apply actions to the GameController. The callbacks from the configuration are called
/// for the resulting commands before this function returns. It should be called after
/// [crate::gc_seek] in each simulation step.
#[no_mangle]
pub extern "C" fn gc_autoreferee_update(
    autoreferee: Option<&mut Autoreferee>,
    game_controller: Option<&mut GameController>,
    world: Option<&AutorefereeWorld>,
) {
    let (Some(autoreferee), Some(game_controller), Some(world)) = (
        non_null(autoreferee, "autoreferee"),
        non_null(game_controller, "game_controller"),
        non_null(world, "world"),
    ) else {
        return;
    };
    let world = World {
        ball: world.ball,
        players: EnumMap::from_fn(|side| {
            world.players[match side {
                Side::Home => 0,
                Side::Away => 1,
            }]
            .each_ref()
            .map(|player| {
                player.present.then_some(PlayerPose {
                    x: player.x,
                    y: player.y,
                    theta: player.theta,
                    radius: player.radius,
                })
            })
        }),
    };
    for command in autoreferee.autoreferee.update(game_controller, &world) {
        match command {
            Command::PlaceBall {
                position: [x, y, z],
            } => {
                if let Some(place_ball) = autoreferee.place_ball {
                    place_ball(autoreferee.userptr, x, y, z);
                }
            }
            Command::PlacePlayer {
                side,
                player,
                x,
                y,
                theta,
            } => {
                if let Some(place_player) = autoreferee.place_player {
                    place_player(autoreferee.userptr, side, u8::from(player), x, y, theta);
                }
            }
            Command::Whistle => {
                if let Some(whistle) = autoreferee.whistle {
                    whistle(autoreferee.userptr);
                }
            }
        }
    }
}

/// This function notifies the automatic referee that a player touched the ball.
///
/// Returns `false` if an argument is `NULL` or `player` is not a valid player number.
#[no_mangle]
pub extern "C" fn gc_autoreferee_ball_contact(
    autoreferee: Option<&mut Autoreferee>,
    game_controller: Option<&mut GameController>,
    side: Side,
    player: u8,
) -> bool {
    let (Some(autoreferee), Some(game_controller), Some(player)) = (
        non_null(autoreferee, "autoreferee"),
        non_null(game_controller, "game_controller"),
        player_number(player),
    ) else {
        return false;
    };
    autoreferee
        .autoreferee
        .ball_contact(game_controller, side, player);
    true
}

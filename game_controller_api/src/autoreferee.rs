//! This module exposes the automatic referee from [game_controller_autoreferee] to C.
//!
//! In each simulation step, the caller should call [crate::gc_seek], then
//! [gc_autoreferee_update], and then [crate::gc_control_message]. [gc_autoreferee_ball_contact]
//! should be called whenever a player touches the ball. See [game_controller_autoreferee] for the
//! coordinate system (lengths are in meters, angles in radians).

use std::{ffi::c_void, ptr::null_mut, time::Duration};

use enum_map::{Enum, EnumMap};

use game_controller_autoreferee::{Command, Config, Features, FieldDimensions, PlayerPose, World};
use game_controller_core::{types::Side, GameController};

use crate::{
    error::{player_number, set_last_error},
    non_null,
};

/// This struct contains the configuration of the automatic referee, including the callbacks with
/// which it executes commands in the simulator. Callbacks may be null.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct AutorefereeConfig {
    /// The features that are enabled.
    pub features: Features,
    /// The dimensions of the field.
    pub field: FieldDimensions,
    /// The time (in milliseconds) in Ready after which the automatic referee switches to Set if no
    /// player moved for that long.
    pub time_until_switch_to_set: u64,
    /// The time (in milliseconds) in Set after which the automatic referee switches to Playing.
    pub time_until_switch_to_playing: u64,
    /// The time (in milliseconds) for which play stays stopped after a restart (e.g. a throw-in)
    /// has been awarded, before the automatic referee resumes it. The ball is placed right away
    /// (if its spot is free), and play is not resumed before it is there.
    pub time_until_resume: u64,
    /// The time (in milliseconds) between the ball clearly moving (by at least its radius) after
    /// the kicking team touched it and the automatic referee finishing the set play ("ball
    /// free").
    pub time_until_ball_free: u64,
    /// The minimum distance between players that are placed for a penalty.
    pub penalty_placement_distance: f32,
    /// An opaque value that is passed to the callbacks (e.g. a this-pointer).
    pub userptr: *mut c_void,
    /// A function that moves the ball to a given position and resets its velocity. It is called
    /// again if the ball is not observed there after a while.
    pub place_ball: Option<extern "C" fn(userptr: *mut c_void, x: f32, y: f32, z: f32)>,
    /// A function that moves a player to a given upright pose.
    pub place_player: Option<
        extern "C" fn(userptr: *mut c_void, side: Side, player: u8, x: f32, y: f32, theta: f32),
    >,
    /// A function that tells the environment to move a player to its pose for the walk-in. It is
    /// only called for players that are present and not penalized, once they have come to rest (or
    /// after 3 seconds at the latest).
    pub place_for_walk_in: Option<extern "C" fn(userptr: *mut c_void, side: Side, player: u8)>,
    /// A function that is called when all requested walk-in placements have been made (which may
    /// be none, e.g. if no player is present), after which it is safe to proceed (e.g. to switch to
    /// Ready). It is not called if the state changed before.
    pub walk_in_complete: Option<extern "C" fn(userptr: *mut c_void)>,
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
    config: AutorefereeConfig,
}

/// This function checks that features only contain known bits (e.g. a header that is newer than
/// the library could contain more), or stores an error.
fn known_features(features: Features) -> Option<Features> {
    let known = Features::from_bits(features.bits());
    if known.is_none() {
        set_last_error(format!("unknown features in {:#x}", features.bits()));
    }
    known
}

/// This function creates an automatic referee.
///
/// The configuration is copied. The caller takes ownership of the result and must release it with
/// [gc_autoreferee_destroy]. Returns `NULL` on error (including unknown features).
#[no_mangle]
pub extern "C" fn gc_autoreferee_new(config: Option<&AutorefereeConfig>) -> *mut Autoreferee {
    let Some((config, features)) = non_null(config, "config")
        .and_then(|config| Some((config, known_features(config.features)?)))
    else {
        return null_mut();
    };
    Box::into_raw(Box::new(Autoreferee {
        autoreferee: game_controller_autoreferee::Autoreferee::new(Config {
            features,
            field: config.field,
            time_until_switch_to_set: Duration::from_millis(config.time_until_switch_to_set),
            time_until_switch_to_playing: Duration::from_millis(
                config.time_until_switch_to_playing,
            ),
            time_until_resume: Duration::from_millis(config.time_until_resume),
            time_until_ball_free: Duration::from_millis(config.time_until_ball_free),
            penalty_placement_distance: config.penalty_placement_distance,
        }),
        config: *config,
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
/// The rest of its state is kept. Returns `false` if an argument is `NULL` or there are unknown
/// features.
#[no_mangle]
pub extern "C" fn gc_autoreferee_set_features(
    autoreferee: Option<&mut Autoreferee>,
    features: Features,
) -> bool {
    let (Some(autoreferee), Some(features)) = (
        non_null(autoreferee, "autoreferee"),
        known_features(features),
    ) else {
        return false;
    };
    autoreferee.autoreferee.set_features(features);
    true
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
        players: EnumMap::from_fn(|side: Side| {
            world.players[side.into_usize()].each_ref().map(|player| {
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
                if let Some(place_ball) = autoreferee.config.place_ball {
                    place_ball(autoreferee.config.userptr, x, y, z);
                }
            }
            Command::PlacePlayer {
                side,
                player,
                x,
                y,
                theta,
            } => {
                if let Some(place_player) = autoreferee.config.place_player {
                    place_player(
                        autoreferee.config.userptr,
                        side,
                        u8::from(player),
                        x,
                        y,
                        theta,
                    );
                }
            }
            Command::PlaceForWalkIn { side, player } => {
                if let Some(place_for_walk_in) = autoreferee.config.place_for_walk_in {
                    place_for_walk_in(autoreferee.config.userptr, side, u8::from(player));
                }
            }
            Command::WalkInComplete => {
                if let Some(walk_in_complete) = autoreferee.config.walk_in_complete {
                    walk_in_complete(autoreferee.config.userptr);
                }
            }
            Command::Whistle => {
                if let Some(whistle) = autoreferee.config.whistle {
                    whistle(autoreferee.config.userptr);
                }
            }
        }
    }
}

/// This function notifies the automatic referee that a player touched the ball. It should be
/// called in every simulation step in which the player is in contact with the ball.
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

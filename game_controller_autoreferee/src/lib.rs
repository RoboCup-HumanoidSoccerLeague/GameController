//! This crate implements an automatic referee for simulated games. It observes the positions of
//! the players and the ball (supplied by the simulator), applies actions to a [GameController],
//! and returns commands that the simulator should execute (e.g. placing the ball for a set play).
//!
//! The crate does not know anything about C or Python. Bindings for such languages are supposed to
//! be thin adapters around [Autoreferee].
//!
//! # Coordinates and units
//!
//! All positions are in a global coordinate system whose origin is on the surface of the center
//! mark of the field. The x-axis points towards the center of the "left" goal (as seen in the
//! GameController user interface), the z-axis points upwards, and the y-axis completes a
//! right-handed coordinate system. Lengths are in meters, angles in radians.
//!
//! # Usage
//!
//! In each simulation step, the caller should first advance the [GameController] via
//! [GameController::seek], then call [Autoreferee::update] and execute the returned commands.
//! In every step in which a player touches the ball, [Autoreferee::ball_contact] should be called.
//!
//! All timing of the automatic referee (e.g. for game stuck or the ball stop rule) ignores the
//! time while play is stopped.

use std::{f32::consts::PI, time::Duration};

use bitflags::bitflags;
use enum_map::EnumMap;

use game_controller_core::{
    action::VAction,
    actions::{
        FinishHalf, FinishPenaltyShot, FinishSetPlay, FreeSetPlay, GlobalGameStuck, Goal, Penalize,
        StartSetPlay, StopPlay, Unpenalize, WaitForSetPlay,
    },
    timer::{SignedDuration, Timer},
    types::{
        ActionSource, Game, Penalty, PenaltyCall, Phase, PlayerNumber, SetPlay, Side, SideMapping,
        State,
    },
    GameController,
};

mod rules;

use rules::{compute_margins, side_to_sign, Margins};

/// The number of players per team.
pub const NUM_PLAYERS: usize = (PlayerNumber::MAX - PlayerNumber::MIN + 1) as usize;

bitflags! {
    /// This struct represents the features that the automatic referee should provide.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Features: u32 {
        // State transitions.
        /// Switch to Set early if players didn't move sufficiently long.
        const SWITCH_TO_SET = 1 << 0;
        /// Switch to Playing after some time in Set.
        const SWITCH_TO_PLAYING = 1 << 1;
        /// Switch to Finished when the time is up (and the ball has stopped moving).
        const SWITCH_TO_FINISHED = 1 << 2;

        // Situations involving the ball.
        /// Check whether the ball went out of bounds and do the appropriate action (depending on
        /// where and who touched last), e.g. goal, throw-in, goal kick, corner kick. Also ends
        /// penalty shots in a penalty shoot-out.
        const BALL_OUT = 1 << 3;
        /// Do not count goals that are scored directly from restarts that don't allow it (e.g.
        /// from a kick-off or a throw-in). Instead, a goal kick or corner kick is awarded. Without
        /// this flag, every ball that is in a goal counts. Only has an effect together with
        /// [Self::BALL_OUT].
        const INVALID_GOAL = 1 << 4;
        /// Finish set plays early when the ball is touched by the kicking team.
        const BALL_FREE = 1 << 5;
        /// Award an indirect free kick to the opponents when the kicker of a throw-in, goal kick
        /// or corner kick touches the ball again before another player does.
        const DOUBLE_TOUCH = 1 << 6;
        /// Call local game stuck (penalizing the player closest to a ball that doesn't move) and
        /// global game stuck (no player close to the ball).
        const GAME_STUCK = 1 << 7;
        /// Move the ball away from a player when it is stuck between its legs.
        const CLEAR_BALL = 1 << 8;

        // Penalties.
        /// Penalize players that are leaving the field.
        const PENALIZE_LEAVING_THE_FIELD = 1 << 9;
        /// Penalize players that are illegally positioned. For penalty kicks, the goalkeeper is
        /// placed on the goal line instead, and leaving it early results in a goal.
        const PENALIZE_ILLEGAL_POSITIONING = 1 << 10;
        /// Penalize players that move in Set.
        const PENALIZE_MOTION_IN_SET = 1 << 11;
        /// Penalize players that move while play is stopped.
        const PENALIZE_MOTION_IN_STOP = 1 << 12;
        /// Start the penalty timers of penalized players right away (as if they had been returned
        /// by a robot handler) and unpenalize them when their timers are up. This doesn't check
        /// whether they are placed correctly.
        const UNPENALIZE = 1 << 13;

        // Placing players and the ball.
        /// Place players that are penalized (and substitutes that enter the game) at the edge of
        /// the carpet in their own half.
        const PLACE_FOR_PENALTY = 1 << 14;
        /// Place players in a penalty shoot-out.
        const PLACE_FOR_PENALTY_SHOOT_OUT = 1 << 15;
        /// Place the ball for set plays (including kick-off, penalty kick, penalty shoot-out).
        const PLACE_BALL = 1 << 16;
    }
}

/// This struct describes the dimensions of the field. The letters refer to the rule book.
#[derive(Clone, Debug)]
pub struct FieldDimensions {
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

/// This struct contains the constant configuration of the automatic referee.
#[derive(Clone, Debug)]
pub struct Config {
    /// The features that are enabled.
    pub features: Features,
    /// The dimensions of the field.
    pub field: FieldDimensions,
    /// The time in Ready after which the automatic referee switches to Set if no player moved
    /// for that long (the first part of the Ready state is excluded).
    pub time_until_switch_to_set: Duration,
    /// The time in Set after which the automatic referee switches to Playing.
    pub time_until_switch_to_playing: Duration,
    /// The minimum distance between players that are placed for a penalty.
    pub penalty_placement_distance: f32,
}

/// This struct describes the pose of a player on the field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerPose {
    /// The x-coordinate of the player.
    pub x: f32,
    /// The y-coordinate of the player.
    pub y: f32,
    /// The rotation of the player around the z-axis.
    pub theta: f32,
    /// The approximate radius at ground level of the player when standing upright.
    pub radius: f32,
}

/// This struct describes the physical state of the world as observed by the simulator.
#[derive(Clone, Debug)]
pub struct World {
    /// The position of the ball.
    pub ball: [f32; 3],
    /// The poses of the players, indexed by (player number - 1). A player that is not physically
    /// present is [None].
    pub players: EnumMap<Side, [Option<PlayerPose>; NUM_PLAYERS]>,
}

/// This enumerates the commands that the simulator should execute.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Move the ball to a given position and reset its velocity.
    PlaceBall {
        /// The new position of the ball.
        position: [f32; 3],
    },
    /// Move a player to a given upright pose.
    PlacePlayer {
        /// The side of the player.
        side: Side,
        /// The number of the player.
        player: PlayerNumber,
        /// The new x-coordinate of the player.
        x: f32,
        /// The new y-coordinate of the player.
        y: f32,
        /// The new rotation of the player around the z-axis.
        theta: f32,
    },
    /// Notify the players that the whistle has been blown.
    Whistle,
}

/// This enumerates the kinds of restarts that impose restrictions on scoring or touching the ball.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RestartKind {
    /// A kick-off: no direct goal against the opponents.
    KickOff,
    /// An indirect free kick (including throw-ins): no direct goal against the opponents.
    Indirect,
    /// A direct free kick (including goal kicks, corner kicks and penalty kicks).
    Direct,
    /// A penalty shot in a penalty shoot-out.
    PenaltyShot,
}

/// This struct describes the restart of play that is currently in effect, i.e. which restrictions
/// apply until the ball has been touched by another player.
#[derive(Clone, Debug)]
struct Restart {
    /// The kind of the restart.
    kind: RestartKind,
    /// The set play that caused the restart.
    set_play: SetPlay,
    /// The side that takes the restart.
    side: Side,
    /// The player that touched the ball first (i.e. took the kick) and when it did so.
    kicker: Option<(PlayerNumber, Duration)>,
    /// The position of the ball when the kicker touched it first.
    kick_position: [f32; 2],
    /// Whether any player other than the kicker has touched the ball since.
    other_touch: bool,
    /// Whether the kicker touched the ball outside of the center circle.
    kicker_touched_outside_center_circle: bool,
    /// Whether the ball clearly moved since the kick.
    ball_moved_since_kick: bool,
}

/// This struct represents the automatic referee. It contains its configuration and the state that
/// it keeps between updates.
pub struct Autoreferee {
    config: Config,
    /// The game state at the end of the last observation (used to detect changes).
    last_game: Option<Game>,
    /// The GameController time when the current state (or phase) began.
    time_when_state_began: Duration,
    /// The GameController time when the current set play began.
    time_when_set_play_began: Duration,
    /// The GameController time when any player moved last (only tracked in Ready).
    time_when_player_moved: Option<Duration>,
    /// The poses of the players when they were considered to move last.
    last_poses: EnumMap<Side, [Option<PlayerPose>; NUM_PLAYERS]>,
    /// The side which touched the ball last.
    last_contact: Option<Side>,
    /// The player which is allowed to take the current kick-off / penalty kick.
    kicking_player: Option<PlayerNumber>,
    /// Whether players have been penalized and have not been on the field since.
    returning: EnumMap<Side, [bool; NUM_PLAYERS]>,
    /// Whether players (of the defending team) have been outside the avoidance region since the
    /// current set play began.
    left_avoidance: EnumMap<Side, [bool; NUM_PLAYERS]>,
    /// Whether players were inside their own goal area in the last update.
    in_goal_area: EnumMap<Side, [bool; NUM_PLAYERS]>,
    /// The position of the ball in the last update.
    ball: [f32; 3],
    /// The position of the ball when it was considered to move last.
    ball_reference: Option<[f32; 3]>,
    /// The GameController time when the ball moved last.
    time_when_ball_moved: Duration,
    /// The position of the ball (and the time) since which it hasn't moved significantly, for
    /// local game stuck.
    stuck_reference: Option<([f32; 2], Duration)>,
    /// The GameController time when any player was close to the ball last, for global game stuck.
    time_when_player_near_ball: Duration,
    /// The GameController time since when the ball is stuck between the legs of each player.
    ball_stuck_since: EnumMap<Side, [Option<Duration>; NUM_PLAYERS]>,
    /// The last touch of the ball and when it ended.
    last_touch: Option<(Side, PlayerNumber, Duration)>,
    /// The restart of play that is currently in effect.
    restart: Option<Restart>,
    /// A clock that only advances while play is not stopped. All timing (except for motion while
    /// play is stopped) uses this clock.
    clock: Duration,
    /// The GameController time when [Self::clock] was advanced last.
    last_gc_time: Option<Duration>,
    /// The time (according to [Self::clock]) when each player touched the ball last.
    last_contact_time: EnumMap<Side, [Option<Duration>; NUM_PLAYERS]>,
    /// The GameController time from which on players must not move (in Set or while play is
    /// stopped).
    motion_check_from: Option<Duration>,
    /// The poses of the players that must not move (captured at [Self::motion_check_from]).
    motion_reference: Option<EnumMap<Side, [Option<PlayerPose>; NUM_PLAYERS]>>,
}

/// This function returns the index of a player number in the player arrays.
fn index(player: PlayerNumber) -> usize {
    (u8::from(player) - PlayerNumber::MIN) as usize
}

/// This function returns an iterator over all players of both teams.
fn all_players() -> impl Iterator<Item = (Side, PlayerNumber)> {
    [Side::Home, Side::Away]
        .into_iter()
        .flat_map(|side| PlayerNumber::all().map(move |player| (side, player)))
}

/// This function applies an action that is triggered by the automatic referee.
fn apply(gc: &mut GameController, action: VAction) -> bool {
    gc.apply(action, ActionSource::Autoreferee)
}

impl Autoreferee {
    /// The distance a player must have moved to count as moving.
    const MOVEMENT_DISTANCE: f32 = 0.005;
    /// The angle a player must have turned to count as moving.
    const MOVEMENT_ANGLE: f32 = 0.05;
    /// The time after the beginning of a set play in which ball contacts don't finish it.
    const BALL_FREE_GRACE_PERIOD: Duration = Duration::from_millis(500);
    /// The distance the ball must have moved to count as moving.
    const BALL_MOVEMENT_DISTANCE: f32 = 0.01;
    /// The time the ball must not have moved to be at rest.
    const BALL_REST_DURATION: Duration = Duration::from_secs(1);
    /// The distance the ball must have moved after a kick to have clearly moved.
    const BALL_CLEARLY_MOVED_DISTANCE: f32 = 0.1;
    /// The time after which the half ends even if the ball is still moving (ball stop rule).
    const BALL_STOP_RULE_MAX_EXTENSION: SignedDuration = SignedDuration::seconds(10);
    /// Contacts of the same player that are closer in time than this count as one touch.
    const TOUCH_DEBOUNCE: Duration = Duration::from_millis(200);
    /// The time after the beginning of a set play in which defenders may leave the avoidance
    /// region (if they have been in there since the beginning).
    const AVOIDANCE_GRACE_PERIOD: SignedDuration = SignedDuration::seconds(10);
    /// The distance from the ball (from the edge of a player) within which a player counts as
    /// close for global game stuck.
    const GLOBAL_GAME_STUCK_DISTANCE: f32 = 1.0;
    /// The time without any player close to the ball after which global game stuck is called.
    const GLOBAL_GAME_STUCK_DURATION: Duration = Duration::from_secs(30);
    /// The distance the ball must move to not count as stuck for local game stuck.
    const LOCAL_GAME_STUCK_BALL_DISTANCE: f32 = 0.1;
    /// The distance from the ball (from the edge of a player) within which a player can be
    /// penalized for local game stuck.
    const LOCAL_GAME_STUCK_PROXIMITY: f32 = 0.5;
    /// The time without significant ball movement after which local game stuck is called.
    const LOCAL_GAME_STUCK_DURATION: Duration = Duration::from_secs(10);
    /// The time the ball must be stuck between the legs of a player before it is cleared.
    const CLEAR_BALL_DURATION: Duration = Duration::from_secs(1);
    /// The distance in front of a player (from its edge) to which a stuck ball is moved.
    const CLEAR_BALL_DISTANCE: f32 = 0.1;
    /// The time after entering Set / stopping play after which players must not move anymore.
    const MOTION_SETTLE_TIME: Duration = Duration::from_secs(1);
    /// The distance a player must move to be penalized for motion in Set / Stop.
    const MOTION_DISTANCE: f32 = 0.1;
    /// The angle a player must turn to be penalized for motion in Set / Stop.
    const MOTION_ANGLE: f32 = 0.3;

    /// This function creates a new automatic referee.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            last_game: None,
            time_when_state_began: Duration::ZERO,
            time_when_set_play_began: Duration::ZERO,
            time_when_player_moved: None,
            last_poses: EnumMap::default(),
            last_contact: None,
            kicking_player: None,
            returning: EnumMap::default(),
            left_avoidance: EnumMap::default(),
            in_goal_area: EnumMap::default(),
            ball: [0.0; 3],
            ball_reference: None,
            time_when_ball_moved: Duration::ZERO,
            stuck_reference: None,
            time_when_player_near_ball: Duration::ZERO,
            ball_stuck_since: EnumMap::default(),
            last_touch: None,
            restart: None,
            clock: Duration::ZERO,
            last_gc_time: None,
            last_contact_time: EnumMap::default(),
            motion_check_from: None,
            motion_reference: None,
        }
    }

    /// This function advances the clock of the automatic referee by the GameController time that
    /// passed since the last call. Since it is unknown when exactly play was stopped or resumed in
    /// between, the time only counts if play was not stopped at the beginning and at the end.
    fn advance_clock(&mut self, gc: &GameController) {
        let now = gc.get_time();
        if let Some(last) = self.last_gc_time {
            if !self.last_game.as_ref().is_some_and(|game| game.stopped)
                && !gc.get_game(false).stopped
            {
                self.clock += now.saturating_sub(last);
            }
        }
        self.last_gc_time = Some(now);
    }

    /// This function changes the enabled features (e.g. because the user toggled them).
    pub fn set_features(&mut self, features: Features) {
        self.config.features = features;
    }

    /// This function runs the automatic referee for one step. It may apply actions to the
    /// GameController and returns commands that the simulator should execute.
    pub fn update(&mut self, gc: &mut GameController, world: &World) -> Vec<Command> {
        let mut commands = vec![];
        let features = self.config.features;
        self.advance_clock(gc);

        // Observe changes that happened since the last update (e.g. by the user or timers). This
        // is repeated after each step so that the next step sees consistent timestamps.
        self.observe(gc, world, &mut commands);

        self.detect_movement(world);
        self.track_ball(world);
        self.update_returning(gc, world);
        self.update_avoidance(gc, world);

        if features.intersects(
            Features::PENALIZE_LEAVING_THE_FIELD | Features::PENALIZE_ILLEGAL_POSITIONING,
        ) {
            self.penalize_players(gc, world);
            self.observe(gc, world, &mut commands);
        }
        if features.contains(Features::PENALIZE_ILLEGAL_POSITIONING) {
            self.check_penalty_goalkeeper(gc, world);
            self.observe(gc, world, &mut commands);
        }
        if features.intersects(Features::PENALIZE_MOTION_IN_SET | Features::PENALIZE_MOTION_IN_STOP)
        {
            self.check_motion(gc, world);
            self.observe(gc, world, &mut commands);
        }
        if features.contains(Features::GAME_STUCK) {
            self.check_game_stuck(gc, world);
            self.observe(gc, world, &mut commands);
        }
        if features.contains(Features::CLEAR_BALL) {
            self.clear_ball(gc, world, &mut commands);
        }
        if features.contains(Features::SWITCH_TO_SET) {
            self.switch_to_set(gc);
            self.observe(gc, world, &mut commands);
        }
        if features.contains(Features::SWITCH_TO_PLAYING) {
            self.switch_to_playing(gc);
            self.observe(gc, world, &mut commands);
        }
        if features.contains(Features::BALL_OUT) {
            self.check_ball_out(gc, world);
            self.observe(gc, world, &mut commands);
        }
        if features.contains(Features::SWITCH_TO_FINISHED) {
            self.switch_to_finished(gc);
            self.observe(gc, world, &mut commands);
        }
        if features.contains(Features::UNPENALIZE) {
            self.unpenalize_players(gc);
            self.observe(gc, world, &mut commands);
        }

        commands
    }

    /// This function must be called when a player touches the ball. It should be called in every
    /// simulation step in which the player is in contact with the ball (contacts in quick
    /// succession count as one touch), since clearing a ball that is stuck between a player's legs
    /// depends on it.
    pub fn ball_contact(&mut self, gc: &mut GameController, side: Side, player: PlayerNumber) {
        self.advance_clock(gc);
        let now = self.clock;

        // Save the last contact for the next time the ball goes out of bounds.
        self.last_contact = Some(side);
        self.last_contact_time[side][index(player)] = Some(now);

        // Check if this contact ends a set play.
        let game = gc.get_game(false);
        if self.config.features.contains(Features::BALL_FREE)
            && game.phase != Phase::PenaltyShootout
            && game.state == State::Playing
            && game.set_play != SetPlay::NoSetPlay
            && game.kicking_side == Some(side)
            && now >= self.time_when_state_began + Self::BALL_FREE_GRACE_PERIOD
            && now >= self.time_when_set_play_began + Self::BALL_FREE_GRACE_PERIOD
        {
            apply(gc, VAction::FinishSetPlay(FinishSetPlay));
        }

        // Contacts of the same player in quick succession count as one touch.
        let is_new_touch = !self.last_touch.is_some_and(|(s, p, time)| {
            s == side && p == player && now <= time + Self::TOUCH_DEBOUNCE
        });
        self.last_touch = Some((side, player, now));
        if !is_new_touch {
            return;
        }

        let [ball_x, ball_y, _] = self.ball;
        let ball_outside_center_circle = ball_x.hypot(ball_y)
            > self.config.field.center_circle_radius + self.config.field.ball_radius;
        let Some(restart) = self.restart.as_mut() else {
            return;
        };
        let second_touch_by_kicker = match restart.kicker {
            None if side == restart.side => {
                restart.kicker = Some((player, now));
                restart.kick_position = [ball_x, ball_y];
                restart.kicker_touched_outside_center_circle |= ball_outside_center_circle;
                false
            }
            Some((kicker, _)) if side == restart.side && player == kicker => {
                restart.kicker_touched_outside_center_circle |= ball_outside_center_circle;
                !restart.other_touch && restart.ball_moved_since_kick
            }
            _ => {
                restart.other_touch = true;
                false
            }
        };
        if !second_touch_by_kicker {
            return;
        }
        let (kind, set_play) = (restart.kind, restart.set_play);
        let game = gc.get_game(false);
        if kind == RestartKind::PenaltyShot {
            // The penalty shot ends if the striker plays the ball a second time.
            if self.config.features.contains(Features::BALL_OUT) {
                apply(gc, VAction::FinishPenaltyShot(FinishPenaltyShot));
            }
        } else if self.config.features.contains(Features::DOUBLE_TOUCH)
            && matches!(
                set_play,
                SetPlay::ThrowIn | SetPlay::GoalKick | SetPlay::CornerKick
            )
            && game.phase != Phase::PenaltyShootout
            && game.state == State::Playing
            && rules::players_on_field(game, &self.last_poses[side], side) >= 3
        {
            // The opponents get an indirect free kick where the infringement happened, i.e. the
            // ball stays where it is.
            self.restart = None;
            if apply(
                gc,
                VAction::StartSetPlay(StartSetPlay {
                    side: Some(-side),
                    set_play: SetPlay::IndirectFreeKick,
                }),
            ) {
                apply(gc, VAction::StopPlay(StopPlay { resume: true }));
            }
        }
    }

    /// This function compares the current game state to the one from the last observation and
    /// reacts to changes (updates timestamps, places the ball / players, blows the whistle).
    fn observe(&mut self, gc: &GameController, world: &World, commands: &mut Vec<Command>) {
        let now = self.clock;
        let game = gc.get_game(false);
        let Some(last) = self.last_game.replace(game.clone()) else {
            self.time_when_state_began = now;
            self.time_when_set_play_began = now;
            return;
        };
        let features = self.config.features;
        let field = &self.config.field;

        if game.state != last.state || game.phase != last.phase {
            self.time_when_state_began = now;
        }

        if game.set_play != SetPlay::NoSetPlay
            && (game.set_play != last.set_play || game.kicking_side != last.kicking_side)
        {
            self.time_when_set_play_began = now;
            self.kicking_player = None;
            self.left_avoidance = EnumMap::default();
            self.restart = game.kicking_side.map(|side| Restart {
                kind: match game.set_play {
                    SetPlay::KickOff => RestartKind::KickOff,
                    SetPlay::ThrowIn | SetPlay::IndirectFreeKick => RestartKind::Indirect,
                    _ => RestartKind::Direct,
                },
                set_play: game.set_play,
                side,
                kicker: None,
                kick_position: [0.0; 2],
                other_touch: false,
                kicker_touched_outside_center_circle: false,
                ball_moved_since_kick: false,
            });
            // Set plays that start during Playing are placed immediately. Kick-offs and penalty
            // kicks are placed when entering Set.
            if features.contains(Features::PLACE_BALL) && game.state == State::Playing {
                let [x, y, _] = world.ball;
                // Balls are placed on the centers of lines (the dimensions of the field refer to
                // the outer edges of lines).
                let goal_line_x = field.length * 0.5 - field.line_width * 0.5;
                let touchline_y = field.width * 0.5 - field.line_width * 0.5;
                if let Some((x, y)) = match game.set_play {
                    SetPlay::GoalKick => Some((
                        (field.length * 0.5 - field.goal_area_length + field.line_width * 0.5)
                            .copysign(x),
                        (field.goal_area_width * 0.5 - field.line_width * 0.5).copysign(y),
                    )),
                    SetPlay::CornerKick => Some((goal_line_x.copysign(x), touchline_y.copysign(y))),
                    SetPlay::ThrowIn => {
                        Some((x.clamp(-goal_line_x, goal_line_x), touchline_y.copysign(y)))
                    }
                    // Indirect free kicks inside the opponents' penalty area are taken from the
                    // nearest point on the penalty area line parallel to the goal line.
                    SetPlay::IndirectFreeKick => Some(game.kicking_side.map_or((x, y), |side| {
                        let sign = side_to_sign(side, game.sides);
                        let penalty_area_line_x =
                            field.length * 0.5 - field.penalty_area_length + field.line_width * 0.5;
                        let half_width = field.penalty_area_width * 0.5 - field.line_width * 0.5;
                        if sign * x > penalty_area_line_x && y.abs() <= half_width {
                            (sign * penalty_area_line_x, y)
                        } else {
                            (x, y)
                        }
                    })),
                    SetPlay::DirectFreeKick => Some((x, y)),
                    _ => None,
                } {
                    commands.push(Command::PlaceBall {
                        position: [x, y, field.ball_radius],
                    });
                }
            }
        }

        if game.set_play == SetPlay::NoSetPlay
            && last.set_play != SetPlay::NoSetPlay
            && game.state == State::Playing
            && self
                .restart
                .as_ref()
                .is_some_and(|restart| restart.kicker.is_none())
        {
            // A failed free kick lifts all restrictions.
            self.restart = None;
        }
        if !matches!(game.state, State::Ready | State::Set | State::Playing) {
            self.restart = None;
        }

        let must_not_move = game.state == State::Set || game.stopped;
        if !must_not_move {
            self.motion_check_from = None;
            self.motion_reference = None;
        } else if (game.state == State::Set && last.state != State::Set)
            || (game.stopped && !last.stopped)
        {
            // This uses the GameController time since it is also about stopped play.
            self.motion_check_from = Some(gc.get_time() + Self::MOTION_SETTLE_TIME);
            self.motion_reference = None;
        }

        if game.state == State::Set && last.state != State::Set {
            if game.phase == Phase::PenaltyShootout {
                self.restart = game.kicking_side.map(|side| Restart {
                    kind: RestartKind::PenaltyShot,
                    set_play: SetPlay::NoSetPlay,
                    side,
                    kicker: None,
                    kick_position: [0.0; 2],
                    other_touch: false,
                    kicker_touched_outside_center_circle: false,
                    ball_moved_since_kick: false,
                });
                if let Some(kicking_side) = game.kicking_side {
                    if features.contains(Features::PLACE_BALL) {
                        let sign = side_to_sign(kicking_side, game.sides);
                        commands.push(Command::PlaceBall {
                            position: [
                                sign * (field.length * 0.5 - field.penalty_mark_distance),
                                0.0,
                                field.ball_radius,
                            ],
                        });
                    }
                }
            } else if let (SetPlay::KickOff | SetPlay::PenaltyKick, Some(kicking_side)) =
                (game.set_play, game.kicking_side)
            {
                self.kicking_player = rules::determine_kicking_player(
                    game,
                    world,
                    field,
                    &compute_margins(game, world, field),
                    kicking_side,
                );
                if game.set_play == SetPlay::PenaltyKick
                    && features.contains(Features::PENALIZE_ILLEGAL_POSITIONING)
                {
                    self.place_penalty_goalkeeper(game, world, -kicking_side, commands);
                }
                if features.contains(Features::PLACE_BALL) {
                    let x = if game.set_play == SetPlay::KickOff {
                        0.0
                    } else {
                        side_to_sign(kicking_side, game.sides)
                            * (field.length * 0.5 - field.penalty_mark_distance)
                    };
                    commands.push(Command::PlaceBall {
                        position: [x, 0.0, field.ball_radius],
                    });
                }
            }
        }

        if game.state == State::Playing && last.state == State::Set {
            commands.push(Command::Whistle);
        }

        for (side, player) in all_players() {
            let penalty = game.teams[side][player].penalty;
            let last_penalty = last.teams[side][player].penalty;
            if last_penalty == Penalty::NoPenalty
                && penalty != Penalty::NoPenalty
                && penalty != Penalty::MotionInSet
                && features.contains(Features::PLACE_FOR_PENALTY)
            {
                self.place_for_penalty(game, world, side, player, commands);
            } else if game.phase == Phase::PenaltyShootout
                && last_penalty != Penalty::NoPenalty
                && penalty == Penalty::NoPenalty
                && features.contains(Features::PLACE_FOR_PENALTY_SHOOT_OUT)
            {
                self.place_for_penalty_shot(game, world, side, player, commands);
            } else if game.phase != Phase::PenaltyShootout
                && last_penalty == Penalty::Substitute
                && penalty != Penalty::Substitute
                && features.contains(Features::PLACE_FOR_PENALTY)
            {
                // Substitutes enter the game from the same place as penalized players. They
                // inherit the penalty of the substituted player (or are picked up).
                self.place_for_penalty(game, world, side, player, commands);
            }
        }
    }

    /// This function moves a penalized player to an unoccupied position at the edge of the carpet
    /// next to the touchline. Positions are tried with increasing distance from the height of the
    /// own penalty mark, but only in the own half and in front of the own goal line.
    fn place_for_penalty(
        &self,
        game: &Game,
        world: &World,
        side: Side,
        player: PlayerNumber,
        commands: &mut Vec<Command>,
    ) {
        let Some(pose) = world.players[side][index(player)] else {
            return;
        };
        let field = &self.config.field;
        let distance = self.config.penalty_placement_distance;
        let sign = side_to_sign(side, game.sides);
        let y = (field.width * 0.5 + field.border_strip_width - pose.radius).copysign(pose.y);
        let theta = if y > 0.0 { -PI * 0.5 } else { PI * 0.5 };

        // Other players (and players that have just been placed) occupy their spots.
        let occupied: Vec<(f32, f32)> = all_players()
            .filter(|&other| other != (side, player))
            .filter_map(|(s, p)| world.players[s][index(p)].map(|pose| (pose.x, pose.y)))
            .chain(commands.iter().filter_map(|command| match command {
                Command::PlacePlayer { x, y, .. } => Some((*x, *y)),
                _ => None,
            }))
            .collect();
        let clearance = |team_x: f32| {
            occupied
                .iter()
                .map(|(x, oy)| (x - sign * team_x).hypot(oy - y))
                .fold(f32::INFINITY, f32::min)
        };

        let candidates = rules::penalty_placement_candidates(
            -(field.length * 0.5 - field.penalty_mark_distance),
            -field.length * 0.5 + pose.radius,
            -pose.radius,
            distance,
        );
        // If all positions are occupied, the one with the most space is taken.
        let team_x = candidates
            .iter()
            .copied()
            .find(|&team_x| clearance(team_x) >= distance)
            .or_else(|| {
                candidates
                    .iter()
                    .copied()
                    .max_by(|a, b| clearance(*a).total_cmp(&clearance(*b)))
            })
            .unwrap_or(candidates[0]);
        commands.push(Command::PlacePlayer {
            side,
            player,
            x: sign * team_x,
            y,
            theta,
        });
    }

    /// This function places a player that has just been selected for a penalty shot, i.e. the
    /// striker behind the penalty mark and the goalkeeper on the goal line.
    fn place_for_penalty_shot(
        &self,
        game: &Game,
        world: &World,
        side: Side,
        player: PlayerNumber,
        commands: &mut Vec<Command>,
    ) {
        let (Some(kicking_side), Some(_)) = (game.kicking_side, world.players[side][index(player)])
        else {
            return;
        };
        let field = &self.config.field;
        let sign = side_to_sign(side, game.sides);
        let team_x = if side == kicking_side {
            field.length * 0.5 - field.penalty_area_length + field.line_width * 0.5
        } else {
            -(field.length * 0.5 - field.line_width * 0.5)
        };
        commands.push(Command::PlacePlayer {
            side,
            player,
            x: sign * team_x,
            y: 0.0,
            theta: if sign > 0.0 { 0.0 } else { PI },
        });
    }

    /// This function places the goalkeeper of the defending team on the closest point of the goal
    /// line that is not blocked by a goal post (moving it perpendicular to the goal line and
    /// keeping its orientation) if it is not on the goal line when entering Set for a penalty
    /// kick.
    fn place_penalty_goalkeeper(
        &self,
        game: &Game,
        world: &World,
        side: Side,
        commands: &mut Vec<Command>,
    ) {
        let Some(goalkeeper) = game.teams[side].goalkeeper else {
            return;
        };
        let Some(pose) = world.players[side][index(goalkeeper)] else {
            return;
        };
        let field = &self.config.field;
        let sign = side_to_sign(side, game.sides);
        if game.teams[side][goalkeeper].penalty != Penalty::NoPenalty
            || rules::is_on_own_goal_line(sign * pose.x, &pose, field)
        {
            return;
        }
        let max_y = (field.goal_width * 0.5 - pose.radius).max(0.0);
        commands.push(Command::PlacePlayer {
            side,
            player: goalkeeper,
            x: sign * -(field.length * 0.5 - field.line_width * 0.5),
            y: pose.y.clamp(-max_y, max_y),
            theta: pose.theta,
        });
    }

    /// This function tracks the movement of the ball and the progress of the current restart.
    fn track_ball(&mut self, world: &World) {
        let now = self.clock;
        self.ball = world.ball;
        let [x, y, z] = world.ball;
        if !self.ball_reference.is_some_and(|[rx, ry, rz]| {
            (x - rx).hypot(y - ry).hypot(z - rz) <= Self::BALL_MOVEMENT_DISTANCE
        }) {
            self.ball_reference = Some(world.ball);
            self.time_when_ball_moved = now;
        }
        if let Some(restart) = self.restart.as_mut() {
            if restart.kicker.is_some()
                && (x - restart.kick_position[0]).hypot(y - restart.kick_position[1])
                    > Self::BALL_CLEARLY_MOVED_DISTANCE
            {
                restart.ball_moved_since_kick = true;
            }
        }
    }

    /// This function returns whether the ball has been at rest for some time (and since the
    /// given point in time).
    fn is_ball_at_rest(&self, now: Duration, since: Duration) -> bool {
        now >= self.time_when_ball_moved.max(since) + Self::BALL_REST_DURATION
    }

    /// This function keeps track of which players of the defending team have been outside the
    /// avoidance region since the current set play began.
    fn update_avoidance(&mut self, gc: &GameController, world: &World) {
        let game = gc.get_game(false);
        // While the region is not in effect (e.g. while play is stopped to place the ball), players
        // can't leave it.
        if !rules::is_avoidance_region_active(game) {
            return;
        }
        let margins = compute_margins(game, world, &self.config.field);
        for (side, player) in all_players() {
            if !rules::is_in_avoidance_region(
                game,
                world,
                &self.config.field,
                &margins,
                side,
                player,
            ) {
                self.left_avoidance[side][index(player)] = true;
            }
        }
    }

    /// This function awards a goal to the attacking team if the goalkeeper leaves the goal line
    /// during a penalty kick before the striker has touched the ball.
    fn check_penalty_goalkeeper(&mut self, gc: &mut GameController, world: &World) {
        let game = gc.get_game(false);
        let Some(restart) = self.restart.as_ref() else {
            return;
        };
        if game.state != State::Playing
            || game.stopped
            || restart.kicker.is_some()
            || !(restart.kind == RestartKind::PenaltyShot
                || restart.set_play == SetPlay::PenaltyKick)
        {
            return;
        }
        let defending_side = -restart.side;
        // In a penalty shoot-out, the goalkeeper might not be designated as such.
        let Some(goalkeeper) = game.teams[defending_side].goalkeeper.or_else(|| {
            PlayerNumber::all()
                .find(|&player| game.teams[defending_side][player].penalty == Penalty::NoPenalty)
        }) else {
            return;
        };
        let Some(pose) = world.players[defending_side][index(goalkeeper)] else {
            return;
        };
        if game.teams[defending_side][goalkeeper].penalty == Penalty::NoPenalty
            && !rules::is_on_own_goal_line(
                side_to_sign(defending_side, game.sides) * pose.x,
                &pose,
                &self.config.field,
            )
        {
            apply(gc, VAction::Goal(Goal { side: restart.side }));
        }
    }

    /// This function penalizes players that move in Set or while play is stopped.
    fn check_motion(&mut self, gc: &mut GameController, world: &World) {
        let now = gc.get_time();
        if self.motion_check_from.is_none_or(|from| now < from) {
            return;
        }
        let Some(reference) = self.motion_reference.as_mut() else {
            // Only players that are playing at this point are checked.
            let game = gc.get_game(false);
            self.motion_reference = Some(EnumMap::from_fn(|side| {
                std::array::from_fn(|i| {
                    world.players[side][i]
                        .filter(|_| game.teams[side].players[i].penalty == Penalty::NoPenalty)
                })
            }));
            return;
        };
        let game = gc.get_game(false);
        let call = if game.stopped {
            self.config
                .features
                .contains(Features::PENALIZE_MOTION_IN_STOP)
                .then_some(PenaltyCall::MotionInStop)
        } else {
            self.config
                .features
                .contains(Features::PENALIZE_MOTION_IN_SET)
                .then_some(PenaltyCall::MotionInSet)
        };
        let moved: Vec<(Side, PlayerNumber)> = all_players()
            .filter(|&(side, player)| {
                let (Some(reference_pose), Some(pose)) = (
                    reference[side][index(player)],
                    world.players[side][index(player)],
                ) else {
                    return false;
                };
                game.teams[side][player].penalty == Penalty::NoPenalty
                    && ((pose.x - reference_pose.x).hypot(pose.y - reference_pose.y)
                        > Self::MOTION_DISTANCE
                        || rules::normalize_angle(pose.theta - reference_pose.theta).abs()
                            > Self::MOTION_ANGLE)
            })
            .collect();
        for &(side, player) in &moved {
            // Each player is only checked until it has moved once.
            reference[side][index(player)] = None;
        }
        if let Some(call) = call {
            for (side, player) in moved {
                apply(gc, VAction::Penalize(Penalize { side, player, call }));
            }
        }
    }

    /// This function calls local game stuck (if the ball has not moved for some time while players
    /// are close to it) and global game stuck (if no player has been close to the ball for some
    /// time).
    fn check_game_stuck(&mut self, gc: &mut GameController, world: &World) {
        let now = self.clock;
        let game = gc.get_game(false);
        // While play is stopped, the clock doesn't advance, so nothing happens.
        if game.stopped {
            return;
        }
        if game.phase == Phase::PenaltyShootout
            || game.state != State::Playing
            || game.set_play != SetPlay::NoSetPlay
        {
            self.time_when_player_near_ball = now;
            self.stuck_reference = None;
            return;
        }
        let [ball_x, ball_y, _] = world.ball;
        // The distances of all playing players (from their edges) to the ball.
        let distances: Vec<((Side, PlayerNumber), f32)> = all_players()
            .filter(|&(side, player)| game.teams[side][player].penalty == Penalty::NoPenalty)
            .filter_map(|(side, player)| {
                world.players[side][index(player)].map(|pose| {
                    (
                        (side, player),
                        (pose.x - ball_x).hypot(pose.y - ball_y) - pose.radius,
                    )
                })
            })
            .collect();

        if distances
            .iter()
            .any(|(_, distance)| *distance <= Self::GLOBAL_GAME_STUCK_DISTANCE)
        {
            self.time_when_player_near_ball = now;
        } else if now >= self.time_when_player_near_ball + Self::GLOBAL_GAME_STUCK_DURATION {
            apply(gc, VAction::GlobalGameStuck(GlobalGameStuck));
            return;
        }

        let since = match self.stuck_reference {
            Some(([x, y], since))
                if (ball_x - x).hypot(ball_y - y) <= Self::LOCAL_GAME_STUCK_BALL_DISTANCE =>
            {
                since
            }
            _ => {
                self.stuck_reference = Some(([ball_x, ball_y], now));
                now
            }
        };
        if now >= since + Self::LOCAL_GAME_STUCK_DURATION {
            if let Some(((side, player), _)) = distances
                .iter()
                .filter(|(_, distance)| *distance <= Self::LOCAL_GAME_STUCK_PROXIMITY)
                .min_by(|(_, distance1), (_, distance2)| distance1.total_cmp(distance2))
            {
                apply(
                    gc,
                    VAction::Penalize(Penalize {
                        side: *side,
                        player: *player,
                        call: PenaltyCall::LocalGameStuck,
                    }),
                );
                self.stuck_reference = None;
            }
        }
    }

    /// This function moves the ball in front of a player if it has been stuck between its legs
    /// for some time, i.e. it has been in contact with the player and within its radius.
    fn clear_ball(&mut self, gc: &GameController, world: &World, commands: &mut Vec<Command>) {
        let now = self.clock;
        let game = gc.get_game(false);
        // While play is stopped, the clock doesn't advance, so nothing happens.
        if game.stopped {
            return;
        }
        let playing = game.state == State::Playing;
        let field = &self.config.field;
        let [ball_x, ball_y, _] = world.ball;
        for (side, player) in all_players() {
            let in_contact = self.last_contact_time[side][index(player)]
                .is_some_and(|time| now <= time + Self::TOUCH_DEBOUNCE);
            let since = &mut self.ball_stuck_since[side][index(player)];
            let Some(pose) = world.players[side][index(player)].filter(|pose| {
                playing && in_contact && (pose.x - ball_x).hypot(pose.y - ball_y) < pose.radius
            }) else {
                *since = None;
                continue;
            };
            let stuck_since = *since.get_or_insert(now);
            if now >= stuck_since + Self::CLEAR_BALL_DURATION {
                *since = None;
                let distance = pose.radius + field.ball_radius + Self::CLEAR_BALL_DISTANCE;
                let max_x = field.length * 0.5 - field.ball_radius;
                let max_y = field.width * 0.5 - field.ball_radius;
                commands.push(Command::PlaceBall {
                    position: [
                        (pose.x + distance * pose.theta.cos()).clamp(-max_x, max_x),
                        (pose.y + distance * pose.theta.sin()).clamp(-max_y, max_y),
                        field.ball_radius,
                    ],
                });
                break;
            }
        }
    }

    /// This function detects whether any player has moved. This is only relevant in Ready.
    fn detect_movement(&mut self, world: &World) {
        let now = self.clock;
        for (side, player) in all_players() {
            let last = &mut self.last_poses[side][index(player)];
            match (world.players[side][index(player)], *last) {
                (Some(pose), Some(last_pose)) => {
                    if (pose.x - last_pose.x).hypot(pose.y - last_pose.y) > Self::MOVEMENT_DISTANCE
                        || rules::normalize_angle(pose.theta - last_pose.theta).abs()
                            > Self::MOVEMENT_ANGLE
                    {
                        *last = Some(pose);
                        self.time_when_player_moved = Some(now);
                    }
                }
                (pose, _) => *last = pose,
            }
        }
    }

    /// This function keeps track of which players are returning from a penalty, i.e. have been
    /// penalized and have not been on the field since.
    fn update_returning(&mut self, gc: &GameController, world: &World) {
        let game = gc.get_game(false);
        for (side, player) in all_players() {
            let returning = &mut self.returning[side][index(player)];
            if game.teams[side][player].penalty != Penalty::NoPenalty {
                *returning = true;
            } else if world.players[side][index(player)]
                .is_some_and(|pose| !rules::is_outside_field(&pose, &self.config.field))
            {
                *returning = false;
            }
        }
    }

    /// This function penalizes players that left the field or are illegally positioned.
    fn penalize_players(&mut self, gc: &mut GameController, world: &World) {
        let features = self.config.features;
        let game = gc.get_game(false);
        let margins = compute_margins(game, world, &self.config.field);

        // At most three players of a team may be in their own goal area. In Playing, the players
        // that entered last are illegal, in Set the ones closest to the border.
        let goal_area_excess: Vec<(Side, PlayerNumber)> = if game.phase != Phase::PenaltyShootout
            && matches!(game.state, State::Set | State::Playing)
        {
            [Side::Home, Side::Away]
                .into_iter()
                .flat_map(|side| {
                    rules::goal_area_excess(
                        game,
                        &margins,
                        side,
                        &self.in_goal_area[side],
                        game.state == State::Playing,
                    )
                    .into_iter()
                    .map(move |player| (side, player))
                })
                .collect()
        } else {
            vec![]
        };
        self.in_goal_area = EnumMap::from_fn(|side| {
            std::array::from_fn(|i| {
                game.teams[side].players[i].penalty == Penalty::NoPenalty
                    && margins[side][i].is_some_and(|m| m.own_goal_area >= 0.0)
            })
        });

        for (side, player) in all_players() {
            let game = gc.get_game(false);
            let call = if features.contains(Features::PENALIZE_LEAVING_THE_FIELD)
                && rules::is_leaving_the_field(game, world, &self.config.field, side, player)
            {
                Some(PenaltyCall::LeavingTheField)
            } else if features.contains(Features::PENALIZE_ILLEGAL_POSITIONING)
                && (goal_area_excess.contains(&(side, player))
                    || self.is_illegally_positioned(gc, world, &margins, side, player))
            {
                Some(PenaltyCall::IllegalPosition)
            } else {
                None
            };
            if let Some(call) = call {
                apply(gc, VAction::Penalize(Penalize { side, player, call }));
            }
        }
    }

    /// This function checks if a player should be penalized for illegal position.
    fn is_illegally_positioned(
        &mut self,
        gc: &GameController,
        world: &World,
        margins: &EnumMap<Side, [Option<Margins>; NUM_PLAYERS]>,
        side: Side,
        player: PlayerNumber,
    ) -> bool {
        let game = gc.get_game(false);
        // Players must be on the field in Set, unless they are still returning from a penalty.
        // Unclear whether the rules actually still require this.
        if game.phase != Phase::PenaltyShootout
            && game.state == State::Set
            && game.teams[side][player].penalty == Penalty::NoPenalty
            && !self.returning[side][index(player)]
            && world.players[side][index(player)]
                .is_some_and(|pose| rules::is_outside_field(&pose, &self.config.field))
        {
            return true;
        }
        let result =
            rules::is_illegally_positioned(game, world, &self.config.field, margins, side, player);
        match result {
            rules::Positioning::Legal => false,
            rules::Positioning::Illegal => true,
            // Players that were in the avoidance region when the set play began get some time to
            // leave it. Players that enter it (again) are penalized immediately.
            rules::Positioning::InAvoidanceRegion => {
                let elapsed = SignedDuration::try_from(
                    gc.params.competition.set_plays[game.set_play].duration,
                )
                .unwrap_or(SignedDuration::ZERO)
                    - game.secondary_timer.get_remaining();
                self.left_avoidance[side][index(player)]
                    || (matches!(game.secondary_timer, Timer::Started { .. })
                        && elapsed >= Self::AVOIDANCE_GRACE_PERIOD)
            }
            // If there hasn't been a designated kicker for this set play yet, the first one that
            // tries to enter the respective area is chosen. This assumes that players can't
            // teleport into that area, so if multiple players enter at the same time, it doesn't
            // matter which one is chosen.
            rules::Positioning::IllegalUnlessKicker => {
                if self.kicking_player.is_none() && game.state == State::Playing {
                    self.kicking_player = Some(player);
                }
                self.kicking_player != Some(player)
            }
        }
    }

    /// This function switches to Set when no player moved for some time in Ready.
    fn switch_to_set(&mut self, gc: &mut GameController) {
        let now = self.clock;
        let game = gc.get_game(false);
        if game.phase == Phase::PenaltyShootout || game.state != State::Ready {
            return;
        }
        let duration = self.config.time_until_switch_to_set;
        if now < self.time_when_state_began + duration {
            // Players are expected to start moving in the beginning.
            self.time_when_player_moved = None;
        } else if self
            .time_when_player_moved
            .is_some_and(|time| now > time + duration)
        {
            apply(gc, VAction::WaitForSetPlay(WaitForSetPlay));
        }
    }

    /// This function switches to Playing after some time in Set.
    fn switch_to_playing(&mut self, gc: &mut GameController) {
        let now = self.clock;
        let game = gc.get_game(false);
        if game.phase != Phase::PenaltyShootout
            && game.state == State::Set
            && now > self.time_when_state_began + self.config.time_until_switch_to_playing
        {
            apply(gc, VAction::FreeSetPlay(FreeSetPlay));
        }
    }

    /// This function checks whether the ball went out of bounds and applies the respective
    /// action.
    fn check_ball_out(&mut self, gc: &mut GameController, world: &World) {
        let game = gc.get_game(false);
        if game.state != State::Playing || game.stopped {
            return;
        }
        let field = &self.config.field;
        let [x, y, z] = world.ball;
        let beyond_goal_line = x.abs() > field.length * 0.5 + field.ball_radius;
        let beyond_touchline = y.abs() > field.width * 0.5 + field.ball_radius;
        // The whole ball must be between the goal posts and under the crossbar.
        let in_goal = y.abs() + field.ball_radius < field.goal_width * 0.5
            && z + field.ball_radius < field.goal_height;

        if game.phase == Phase::PenaltyShootout {
            let Some(kicking_side) = game.kicking_side else {
                return;
            };
            let sign = side_to_sign(kicking_side, game.sides);
            if beyond_goal_line && sign * x > 0.0 && in_goal {
                apply(gc, VAction::Goal(Goal { side: kicking_side }));
            } else if beyond_goal_line || beyond_touchline {
                apply(gc, VAction::FinishPenaltyShot(FinishPenaltyShot));
            } else if let Some((_, kick_time)) = self.restart.as_ref().and_then(|restart| {
                (restart.kind == RestartKind::PenaltyShot)
                    .then_some(restart.kicker)
                    .flatten()
            }) {
                // The penalty shot is over when the ball has come to a stop after the striker
                // touched it.
                if self.is_ball_at_rest(self.clock, kick_time) {
                    apply(gc, VAction::FinishPenaltyShot(FinishPenaltyShot));
                }
            }
            return;
        }

        // The team that defends the half in which the ball is. It gets the ball if it is unknown
        // who touched it last.
        let defending_side = match (game.sides, x > 0.0) {
            (SideMapping::HomeDefendsLeftGoal, true)
            | (SideMapping::HomeDefendsRightGoal, false) => Side::Home,
            _ => Side::Away,
        };
        let (side, set_play) = if beyond_goal_line {
            if in_goal {
                match self.judge_goal(game, -defending_side) {
                    Some(restart) => restart,
                    None => {
                        apply(
                            gc,
                            VAction::Goal(Goal {
                                side: -defending_side,
                            }),
                        );
                        return;
                    }
                }
            } else if self.last_contact.unwrap_or(-defending_side) == -defending_side {
                (defending_side, SetPlay::GoalKick)
            } else {
                (-defending_side, SetPlay::CornerKick)
            }
        } else if beyond_touchline {
            (
                self.last_contact.map_or(defending_side, |side| -side),
                SetPlay::ThrowIn,
            )
        } else {
            return;
        };
        // The ball is placed by the command that results from observing the new set play, so play
        // can be resumed right away.
        if apply(
            gc,
            VAction::StartSetPlay(StartSetPlay {
                side: Some(side),
                set_play,
            }),
        ) {
            apply(gc, VAction::StopPlay(StopPlay { resume: true }));
        }
    }

    /// This function checks whether a goal for a given side would be valid given the restart that
    /// is in effect (if invalid goals are detected at all). If it is not, the restart that follows
    /// instead is returned.
    fn judge_goal(&self, game: &Game, scoring_side: Side) -> Option<(Side, SetPlay)> {
        if !self.config.features.contains(Features::INVALID_GOAL) {
            return None;
        }
        let restart = self.restart.as_ref()?;
        if restart.other_touch {
            return None;
        }
        // The team that would concede the goal.
        let defending_side = -scoring_side;
        if restart.side == defending_side {
            // A team may not score an own goal directly from its restart.
            return (restart.kind != RestartKind::PenaltyShot)
                .then_some((scoring_side, SetPlay::CornerKick));
        }
        let valid = match restart.kind {
            RestartKind::Direct | RestartKind::PenaltyShot => true,
            RestartKind::Indirect => false,
            // A team with at most two players on the field may score after the kicker has touched
            // the ball outside the center circle.
            RestartKind::KickOff => {
                restart.kicker_touched_outside_center_circle
                    && rules::players_on_field(game, &self.last_poses[scoring_side], scoring_side)
                        <= 2
            }
        };
        (!valid).then_some((defending_side, SetPlay::GoalKick))
    }

    /// This function switches to Finished when the time is up.
    fn switch_to_finished(&mut self, gc: &mut GameController) {
        let game = gc.get_game(false);
        if game.state == State::Finished || !game.primary_timer.get_remaining().is_negative() {
            return;
        }
        // The half is extended while the ball is moving (but not forever).
        if game.phase != Phase::PenaltyShootout
            && !self.is_ball_at_rest(self.clock, Duration::ZERO)
            && game.primary_timer.get_remaining() > -Self::BALL_STOP_RULE_MAX_EXTENSION
        {
            return;
        }
        if game.phase == Phase::PenaltyShootout {
            apply(gc, VAction::FinishPenaltyShot(FinishPenaltyShot));
        } else if game.set_play != SetPlay::PenaltyKick {
            apply(gc, VAction::FinishHalf(FinishHalf));
        }
    }

    /// This function starts the penalty timers of penalized players and unpenalizes them when the
    /// timers have elapsed. Both is done by the same action ([Unpenalize]), which corresponds to
    /// clicking the player in the user interface.
    fn unpenalize_players(&mut self, gc: &mut GameController) {
        for (side, player) in all_players() {
            let p = &gc.get_game(false).teams[side][player];
            // Applying the action while the timer is running would reset the timer.
            let timer_stopped_or_elapsed = match p.penalty_timer {
                Timer::Stopped => true,
                Timer::Started { .. } => p.penalty_timer.get_remaining().is_zero(),
            };
            if timer_stopped_or_elapsed
                && !matches!(
                    p.penalty,
                    Penalty::NoPenalty
                        | Penalty::MotionInSet
                        | Penalty::PickedUp
                        | Penalty::Substitute
                        | Penalty::SentOff
                )
            {
                apply(
                    gc,
                    VAction::Unpenalize(Unpenalize {
                        side,
                        player,
                        force: false,
                    }),
                );
            }
        }
    }
}

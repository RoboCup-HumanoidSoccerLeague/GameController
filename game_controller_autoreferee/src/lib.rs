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
//! Whenever a player touches the ball, [Autoreferee::ball_contact] should be called.

use std::{f32::consts::PI, time::Duration};

use bitflags::bitflags;
use enum_map::EnumMap;

use game_controller_core::{
    action::VAction,
    actions::{
        FinishHalf, FinishPenaltyShot, FinishSetPlay, FreeSetPlay, Goal, Penalize, StartSetPlay,
        StopPlay, Unpenalize, WaitForSetPlay,
    },
    timer::Timer,
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
        /// Place players that are penalized at the edge of the carpet in their own half.
        const PLACE_FOR_PENALTY = 1 << 8;
        /// Start the penalty timers of penalized players right away (as if they had been returned
        /// by a robot handler) and unpenalize them when their timers are up. This doesn't check
        /// whether they are placed correctly.
        const UNPENALIZE = 1 << 9;
        /// Place players in a penalty shoot-out.
        const PLACE_FOR_PENALTY_SHOOT_OUT = 1 << 10;
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
        }
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

        // Observe changes that happened since the last update (e.g. by the user or timers). This
        // is repeated after each step so that the next step sees consistent timestamps.
        self.observe(gc, world, &mut commands);

        self.detect_movement(gc, world);
        self.update_returning(gc, world);

        if features.intersects(
            Features::PENALIZE_LEAVING_THE_FIELD | Features::PENALIZE_ILLEGAL_POSITIONING,
        ) {
            self.penalize_players(gc, world);
            self.observe(gc, world, &mut commands);
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

    /// This function must be called when a player touches the ball.
    pub fn ball_contact(&mut self, gc: &mut GameController, side: Side, _player: PlayerNumber) {
        // Save the last contact for the next time the ball goes out of bounds.
        self.last_contact = Some(side);

        // Check if this contact ends a set play.
        let now = gc.get_time();
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
    }

    /// This function compares the current game state to the one from the last observation and
    /// reacts to changes (updates timestamps, places the ball / players, blows the whistle).
    fn observe(&mut self, gc: &GameController, world: &World, commands: &mut Vec<Command>) {
        let now = gc.get_time();
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
                    // TODO: move the ball out of the defenders' penalty area
                    SetPlay::DirectFreeKick | SetPlay::IndirectFreeKick => Some((x, y)),
                    _ => None,
                } {
                    commands.push(Command::PlaceBall {
                        position: [x, y, field.ball_radius],
                    });
                }
            }
        }

        if game.state == State::Set && last.state != State::Set {
            if game.phase == Phase::PenaltyShootout {
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

    /// This function detects whether any player has moved. This is only relevant in Ready.
    fn detect_movement(&mut self, gc: &GameController, world: &World) {
        let now = gc.get_time();
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
        let margins = compute_margins(gc.get_game(false), world, &self.config.field);
        for (side, player) in all_players() {
            let game = gc.get_game(false);
            let call = if features.contains(Features::PENALIZE_LEAVING_THE_FIELD)
                && rules::is_leaving_the_field(game, world, &self.config.field, side, player)
            {
                Some(PenaltyCall::LeavingTheField)
            } else if features.contains(Features::PENALIZE_ILLEGAL_POSITIONING)
                && self.is_illegally_positioned(gc, world, &margins, side, player)
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
        let result = rules::is_illegally_positioned(
            game,
            &gc.params,
            world,
            &self.config.field,
            margins,
            side,
            player,
        );
        match result {
            rules::Positioning::Legal => false,
            rules::Positioning::Illegal => true,
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
        let now = gc.get_time();
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
        let now = gc.get_time();
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
        let in_goal = y.abs() < field.goal_width * 0.5 && z < field.goal_height;

        if game.phase == Phase::PenaltyShootout {
            let Some(kicking_side) = game.kicking_side else {
                return;
            };
            let sign = side_to_sign(kicking_side, game.sides);
            if beyond_goal_line && sign * x > 0.0 && in_goal {
                apply(gc, VAction::Goal(Goal { side: kicking_side }));
            } else if beyond_goal_line || beyond_touchline {
                apply(gc, VAction::FinishPenaltyShot(FinishPenaltyShot));
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
                // TODO: check if the goal was valid (e.g. not directly from an indirect free kick)
                apply(
                    gc,
                    VAction::Goal(Goal {
                        side: -defending_side,
                    }),
                );
                return;
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

    /// This function switches to Finished when the time is up.
    fn switch_to_finished(&mut self, gc: &mut GameController) {
        let game = gc.get_game(false);
        if game.state == State::Finished || !game.primary_timer.get_remaining().is_negative() {
            return;
        }
        // TODO: Check if the ball is stationary.
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

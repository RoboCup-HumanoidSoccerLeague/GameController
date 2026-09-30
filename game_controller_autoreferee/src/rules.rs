//! This module contains the geometric rules that the automatic referee checks.

use std::f32::consts::PI;

use enum_map::EnumMap;

use game_controller_core::{
    timer::{SignedDuration, Timer},
    types::{Game, Params, Penalty, Phase, PlayerNumber, SetPlay, Side, SideMapping, State},
};

use crate::{index, FieldDimensions, PlayerPose, World, NUM_PLAYERS};

/// This function returns the factor that transforms x-coordinates between the global coordinate
/// system and the coordinate system of a team, in which the own goal is at negative x and the
/// opponent goal at positive x (the transformation is its own inverse).
pub fn side_to_sign(side: Side, side_mapping: SideMapping) -> f32 {
    // The left goal is at positive global x.
    if (side == Side::Home) == (side_mapping == SideMapping::HomeDefendsLeftGoal) {
        -1.0
    } else {
        1.0
    }
}

/// This function normalizes an angle to [-pi, pi).
pub fn normalize_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(2.0 * PI) - PI
}

/// This struct describes how far a player is inside certain areas. A value is the distance from
/// the closest point where the player would be completely outside the area. Negative values mean
/// that the player is (completely) outside.
#[derive(Clone, Copy, Debug)]
pub struct Margins {
    /// The margin with respect to the own goal area.
    pub own_goal_area: f32,
    /// The margin with respect to the own penalty area.
    pub own_penalty_area: f32,
    /// The margin with respect to the opponent penalty area.
    pub opponent_penalty_area: f32,
}

/// This function computes the margins of all players that are present.
pub fn compute_margins(
    game: &Game,
    world: &World,
    field: &FieldDimensions,
) -> EnumMap<Side, [Option<Margins>; NUM_PLAYERS]> {
    EnumMap::from_fn(|side| {
        let sign = side_to_sign(side, game.sides);
        world.players[side].map(|pose| {
            pose.map(|pose| {
                let team_x = sign * pose.x;
                let r = pose.radius;
                let y_goal_area = (field.goal_area_width * 0.5 + r - pose.y)
                    .min(field.goal_area_width * 0.5 + r + pose.y);
                let y_penalty_area = (field.penalty_area_width * 0.5 + r - pose.y)
                    .min(field.penalty_area_width * 0.5 + r + pose.y);
                let x_own_goal_area = (-field.length * 0.5 + field.goal_area_length + r) - team_x;
                let x_own_penalty_area =
                    (-field.length * 0.5 + field.penalty_area_length + r) - team_x;
                let x_opponent_penalty_area =
                    team_x - (field.length * 0.5 - field.penalty_area_length - r);
                Margins {
                    own_goal_area: y_goal_area.min(x_own_goal_area),
                    own_penalty_area: y_penalty_area.min(x_own_penalty_area),
                    opponent_penalty_area: y_penalty_area.min(x_opponent_penalty_area),
                }
            })
        })
    })
}

/// This function determines which player of the kicking team is allowed to take a kick-off or
/// penalty kick, i.e. the one closest to the mark (but not on it) in the respective area.
pub fn determine_kicking_player(
    game: &Game,
    world: &World,
    field: &FieldDimensions,
    margins: &EnumMap<Side, [Option<Margins>; NUM_PLAYERS]>,
    side: Side,
) -> Option<PlayerNumber> {
    let sign = side_to_sign(side, game.sides);
    PlayerNumber::all()
        .filter(|&player| game.teams[side][player].penalty == Penalty::NoPenalty)
        .filter_map(|player| {
            let pose = world.players[side][index(player)]?;
            let margins = margins[side][index(player)]?;
            let team_x = sign * pose.x;
            match game.set_play {
                SetPlay::KickOff => {
                    let distance = pose.x.hypot(pose.y);
                    (team_x > -(field.line_width * 0.5 + pose.radius)
                        && distance < field.center_circle_radius + pose.radius
                        && distance >= field.mark_radius + pose.radius)
                        .then_some((player, distance))
                }
                SetPlay::PenaltyKick => {
                    let distance =
                        (team_x - (field.length * 0.5 - field.penalty_mark_distance)).hypot(pose.y);
                    ((margins.opponent_penalty_area > 0.0
                        || distance < field.center_circle_radius + pose.radius)
                        && distance >= field.mark_radius + pose.radius)
                        .then_some((player, distance))
                }
                _ => None,
            }
        })
        .min_by(|(_, distance1), (_, distance2)| distance1.total_cmp(distance2))
        .map(|(player, _)| player)
}

/// This function checks whether a player is completely outside the field of play.
pub fn is_outside_field(pose: &PlayerPose, field: &FieldDimensions) -> bool {
    pose.x.abs() > field.length * 0.5 + pose.radius
        || pose.y.abs() > field.width * 0.5 + pose.radius
}

/// This function returns the x-coordinates (in the coordinate system of a team) at which a
/// penalized player may be placed, in the order in which they should be tried: starting at
/// `start`, with increasing distance, alternating towards the own goal line and the halfway line.
/// Coordinates beyond `min` (own goal line) or `max` (halfway line) are skipped, so once one bound
/// is reached, only the other direction is continued.
pub fn penalty_placement_candidates(start: f32, min: f32, max: f32, step: f32) -> Vec<f32> {
    // This can only happen for degenerate dimensions (or NaN), for which clamp would panic.
    if min.is_nan() || max.is_nan() || min > max {
        return vec![(min + max) * 0.5];
    }
    let start = start.clamp(min, max);
    let mut candidates = vec![start];
    if step <= 0.0 {
        return candidates;
    }
    for k in 1.. {
        let towards_goal_line = start - k as f32 * step;
        let towards_halfway_line = start + k as f32 * step;
        if towards_goal_line < min && towards_halfway_line > max {
            break;
        }
        if towards_goal_line >= min {
            candidates.push(towards_goal_line);
        }
        if towards_halfway_line <= max {
            candidates.push(towards_halfway_line);
        }
    }
    candidates
}

/// This function checks a very crude approximation of leaving the field, i.e. whether an
/// unpenalized player is outside the carpeted area.
pub fn is_leaving_the_field(
    game: &Game,
    world: &World,
    field: &FieldDimensions,
    side: Side,
    player: PlayerNumber,
) -> bool {
    let Some(pose) = world.players[side][index(player)] else {
        return false;
    };
    game.teams[side][player].penalty == Penalty::NoPenalty
        && (pose.x.abs() > field.length * 0.5 + field.border_strip_width
            || pose.y.abs() > field.width * 0.5 + field.border_strip_width)
}

/// This enumerates the results of checking the position of a player.
pub enum Positioning {
    /// The player is positioned legally.
    Legal,
    /// The player is positioned illegally.
    Illegal,
    /// The player is positioned illegally unless it is the designated kicker.
    IllegalUnlessKicker,
}

/// The time after the beginning of a set play after which defenders must keep their distance to
/// the ball.
const SET_PLAY_DISTANCE_GRACE_PERIOD: SignedDuration = SignedDuration::seconds(10);

/// This function checks if a player is illegally positioned.
pub fn is_illegally_positioned(
    game: &Game,
    params: &Params,
    world: &World,
    field: &FieldDimensions,
    margins: &EnumMap<Side, [Option<Margins>; NUM_PLAYERS]>,
    side: Side,
    player: PlayerNumber,
) -> Positioning {
    use Positioning::{Illegal, IllegalUnlessKicker, Legal};

    if game.phase == Phase::PenaltyShootout
        || !matches!(game.state, State::Set | State::Playing)
        || game.teams[side][player].penalty != Penalty::NoPenalty
    {
        return Legal;
    }
    let (Some(pose), Some(m)) = (
        world.players[side][index(player)],
        margins[side][index(player)],
    ) else {
        return Legal;
    };
    let r = pose.radius;
    let team_x = side_to_sign(side, game.sides) * pose.x;

    // Check if too many players of this team are in the own goal area. The players furthest
    // inside (or with lower numbers) are counted first.
    if m.own_goal_area >= 0.0 {
        let players_further_inside = PlayerNumber::all()
            .filter(|&other| {
                other != player
                    && game.teams[side][other].penalty == Penalty::NoPenalty
                    && margins[side][index(other)].is_some_and(|other_m| {
                        other_m.own_goal_area > m.own_goal_area
                            || (other_m.own_goal_area == m.own_goal_area
                                && u8::from(other) < u8::from(player))
                    })
            })
            .count();
        if players_further_inside >= 3 {
            return Illegal;
        }
    }

    let is_kicking_team = game.kicking_side == Some(side);
    match game.set_play {
        SetPlay::KickOff => {
            let in_opponent_half = team_x > -(field.line_width * 0.5 + r);
            let distance_to_center = pose.x.hypot(pose.y);
            let in_center_circle = distance_to_center < field.center_circle_radius + r;
            if is_kicking_team {
                if game.state == State::Set && distance_to_center < field.mark_radius + r {
                    Illegal
                } else if !in_opponent_half {
                    Legal
                } else if !in_center_circle {
                    Illegal
                } else {
                    IllegalUnlessKicker
                }
            } else if in_opponent_half || in_center_circle {
                Illegal
            } else {
                Legal
            }
        }
        SetPlay::PenaltyKick => {
            if is_kicking_team {
                let mark_x = field.length * 0.5 - field.penalty_mark_distance;
                let distance_to_mark = (team_x - mark_x).hypot(pose.y);
                // No player may stand on the mark in Set or be beyond it.
                if (game.state == State::Set && distance_to_mark < field.mark_radius + r)
                    || team_x > mark_x
                {
                    Illegal
                } else if m.opponent_penalty_area > 0.0
                    || distance_to_mark < field.center_circle_radius + r
                {
                    IllegalUnlessKicker
                } else {
                    Legal
                }
            } else if game.teams[side].goalkeeper == Some(player) {
                // The goalkeeper must be on the goal line. When it is outside the penalty area
                // during Playing, that's also fine (e.g. when it is unpenalized during the kick).
                let on_goal_line = pose.y.abs() <= field.goal_width * 0.5
                    && (team_x + (field.length * 0.5 - field.line_width * 0.5)).abs() <= r;
                if on_goal_line || (game.state == State::Playing && m.own_penalty_area <= 0.0) {
                    Legal
                } else {
                    Illegal
                }
            } else {
                let own_mark_x = -(field.length * 0.5 - field.penalty_mark_distance);
                let distance_to_own_mark = (team_x - own_mark_x).hypot(pose.y);
                if m.own_penalty_area > 0.0
                    || team_x < own_mark_x
                    || distance_to_own_mark < field.center_circle_radius + r
                {
                    Illegal
                } else {
                    Legal
                }
            }
        }
        SetPlay::GoalKick
        | SetPlay::DirectFreeKick
        | SetPlay::IndirectFreeKick
        | SetPlay::ThrowIn
        | SetPlay::CornerKick => {
            // Defenders get some time to leave the area around the ball.
            let elapsed =
                SignedDuration::try_from(params.competition.set_plays[game.set_play].duration)
                    .unwrap_or(SignedDuration::ZERO)
                    - game.secondary_timer.get_remaining();
            if is_kicking_team
                || game.kicking_side.is_none()
                || game.state != State::Playing
                || game.stopped
                || !matches!(game.secondary_timer, Timer::Started { .. })
                || elapsed < SET_PLAY_DISTANCE_GRACE_PERIOD
            {
                return Legal;
            }
            let distance_to_ball = (pose.x - world.ball[0]).hypot(pose.y - world.ball[1]);
            if distance_to_ball < field.center_circle_radius + r
                || (game.set_play == SetPlay::GoalKick && m.opponent_penalty_area > 0.0)
            {
                Illegal
            } else {
                Legal
            }
        }
        SetPlay::NoSetPlay => Legal,
    }
}

//! This module contains the geometric rules that the automatic referee checks.

use std::f32::consts::PI;

use enum_map::EnumMap;

use game_controller_core::types::{
    Game, Penalty, Phase, PlayerNumber, SetPlay, Side, SideMapping, State,
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

/// This function checks whether a player is on its own goal line between the goal posts.
pub fn is_on_own_goal_line(team_x: f32, pose: &PlayerPose, field: &FieldDimensions) -> bool {
    pose.y.abs() <= field.goal_width * 0.5
        && (team_x + (field.length * 0.5 - field.line_width * 0.5)).abs() <= pose.radius
}

/// This function returns the number of players of a team that are on the field, i.e. that are
/// present (according to the given poses) and not penalized.
pub fn players_on_field(
    game: &Game,
    poses: &[Option<PlayerPose>; NUM_PLAYERS],
    side: Side,
) -> usize {
    PlayerNumber::all()
        .filter(|&player| {
            game.teams[side][player].penalty == Penalty::NoPenalty && poses[index(player)].is_some()
        })
        .count()
}

/// This function returns whether the avoidance region of a free kick (including goal kicks, corner
/// kicks and throw-ins) is currently in effect.
pub fn is_avoidance_region_active(game: &Game) -> bool {
    game.phase != Phase::PenaltyShootout
        && game.state == State::Playing
        && !game.stopped
        && game.kicking_side.is_some()
        && matches!(
            game.set_play,
            SetPlay::GoalKick
                | SetPlay::DirectFreeKick
                | SetPlay::IndirectFreeKick
                | SetPlay::ThrowIn
                | SetPlay::CornerKick
        )
}

/// This function checks whether a player of the defending team is within the avoidance region of
/// a free kick. Players on their own goal line between the goal posts are exempt.
pub fn is_in_avoidance_region(
    game: &Game,
    world: &World,
    field: &FieldDimensions,
    margins: &EnumMap<Side, [Option<Margins>; NUM_PLAYERS]>,
    side: Side,
    player: PlayerNumber,
) -> bool {
    if !is_avoidance_region_active(game)
        || game.kicking_side != Some(-side)
        || game.teams[side][player].penalty != Penalty::NoPenalty
    {
        return false;
    }
    let (Some(pose), Some(m)) = (
        world.players[side][index(player)],
        margins[side][index(player)],
    ) else {
        return false;
    };
    let sign = side_to_sign(side, game.sides);
    if is_on_own_goal_line(sign * pose.x, &pose, field) {
        return false;
    }
    let [ball_x, ball_y, _] = world.ball;
    // The avoidance region of a goal kick is the kicking team's penalty area. For free kicks
    // inside the defenders' penalty area, the defenders must also be outside of it.
    let ball_team_x = sign * ball_x;
    let ball_in_own_penalty_area = ball_team_x <= -field.length * 0.5 + field.penalty_area_length
        && ball_y.abs() <= field.penalty_area_width * 0.5;
    (pose.x - ball_x).hypot(pose.y - ball_y) < field.center_circle_radius + pose.radius
        || (game.set_play == SetPlay::GoalKick && m.opponent_penalty_area > 0.0)
        || (ball_in_own_penalty_area && m.own_penalty_area > 0.0)
}

/// This function returns the players of a team that exceed the limit of three players in the own
/// goal area. If `prefer_newcomers` is set, players that were not inside before (according to
/// `previously_inside`) are chosen first. Otherwise (and among those), the players closest to the
/// border of the goal area (and then those with higher numbers) are chosen.
pub fn goal_area_excess(
    game: &Game,
    margins: &EnumMap<Side, [Option<Margins>; NUM_PLAYERS]>,
    side: Side,
    previously_inside: &[bool; NUM_PLAYERS],
    prefer_newcomers: bool,
) -> Vec<PlayerNumber> {
    let mut inside: Vec<(PlayerNumber, f32)> = PlayerNumber::all()
        .filter(|&player| game.teams[side][player].penalty == Penalty::NoPenalty)
        .filter_map(|player| {
            margins[side][index(player)]
                .filter(|m| m.own_goal_area >= 0.0)
                .map(|m| (player, m.own_goal_area))
        })
        .collect();
    let excess = inside.len().saturating_sub(MAX_PLAYERS_IN_GOAL_AREA);
    inside.sort_by(|(player1, margin1), (player2, margin2)| {
        let newcomer =
            |player: &PlayerNumber| prefer_newcomers && !previously_inside[index(*player)];
        newcomer(player2)
            .cmp(&newcomer(player1))
            .then(margin1.total_cmp(margin2))
            .then(u8::from(*player2).cmp(&u8::from(*player1)))
    });
    inside
        .into_iter()
        .take(excess)
        .map(|(player, _)| player)
        .collect()
}

/// The maximum number of players of a team that may be in the own goal area.
const MAX_PLAYERS_IN_GOAL_AREA: usize = 3;

/// This enumerates the results of checking the position of a player.
pub enum Positioning {
    /// The player is positioned legally.
    Legal,
    /// The player is positioned illegally.
    Illegal,
    /// The player is positioned illegally unless it is the designated kicker.
    IllegalUnlessKicker,
    /// The player is in the avoidance region of a free kick. Whether this is illegal depends on
    /// whether it has had time to leave.
    InAvoidanceRegion,
}

/// This function checks if a player is illegally positioned with respect to the rules of the
/// current set play. The limit of players in the goal area is checked by [goal_area_excess].
pub fn is_illegally_positioned(
    game: &Game,
    world: &World,
    field: &FieldDimensions,
    margins: &EnumMap<Side, [Option<Margins>; NUM_PLAYERS]>,
    side: Side,
    player: PlayerNumber,
) -> Positioning {
    use Positioning::{Illegal, IllegalUnlessKicker, InAvoidanceRegion, Legal};

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
                // The goalkeeper is placed on the goal line when entering Set. Leaving it early in
                // Playing results in a goal instead of a penalty.
                Legal
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
            if is_in_avoidance_region(game, world, field, margins, side, player) {
                InAvoidanceRegion
            } else {
                Legal
            }
        }
        SetPlay::NoSetPlay => Legal,
    }
}

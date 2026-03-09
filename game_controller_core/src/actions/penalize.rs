use serde::{Deserialize, Serialize};

use crate::action::{Action, ActionContext, VAction};
use crate::actions::Unpenalize;
use crate::timer::{BehaviorAtZero, RunCondition, Timer};
use crate::types::{Penalty, PenaltyCall, Phase, PlayerNumber, Side, State};

/// This struct defines an action to apply a penalty to players.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Penalize {
    /// The side whose player is penalized.
    pub side: Side,
    /// The number of the player who is penalized.
    pub player: PlayerNumber,
    /// The penalty which has been called for the player.
    pub call: PenaltyCall,
}

impl Action for Penalize {
    fn execute(&self, c: &mut ActionContext) {
        // Map the penalty call to a (penalty, warning, caution) tuple.
        let mut penalty = match self.call {
            PenaltyCall::IllegalPosition => (Some(Penalty::IllegalPositioning), false, false),
            PenaltyCall::MotionInSet => (Some(Penalty::MotionInSet), false, false),
            PenaltyCall::LocalGameStuck => (Some(Penalty::LocalGameStuck), false, false),
            PenaltyCall::IncapableRobot => (Some(Penalty::IncapableRobot), false, false),
            PenaltyCall::RequestForPickUp => (Some(Penalty::PickedUp), false, false),
            PenaltyCall::BallHolding => (Some(Penalty::BallHolding), false, false),
            PenaltyCall::LeavingTheField => (Some(Penalty::LeavingTheField), false, false),
            PenaltyCall::PlayingWithArmsHands => (Some(Penalty::PlayingWithArmsHands), true, false),
            PenaltyCall::Pushing => (Some(Penalty::Pushing), false, true),
            PenaltyCall::Warn => (None, true, false),
            PenaltyCall::Caution => (None, false, true),
            PenaltyCall::SendOff => (Some(Penalty::SentOff), false, false),
        };

        if penalty.1 {
            c.game.teams[self.side][self.player].warnings += 1;
            if c.game.teams[self.side][self.player].warnings >= 2 {
                c.game.teams[self.side][self.player].warnings = 0;
                penalty.2 = true;
            }
        }

        if penalty.2 {
            c.game.teams[self.side][self.player].cautions += 1;
            if c.game.teams[self.side][self.player].cautions >= 2 {
                c.game.teams[self.side][self.player].cautions = 0;
                penalty.0 = Some(Penalty::SentOff);
            }
        }

        if let Some(penalty) = penalty.0 {
            c.game.teams[self.side][self.player].penalty_timer = match penalty {
                Penalty::SentOff => Timer::Stopped,
                _ => Timer::Started {
                    remaining: ({
                        // The duration is composed of the base duration plus the increment for
                        // each previous incremental penalty of this team.
                        (c.params.competition.penalties[penalty].duration
                            + c.params.competition.penalty_duration_increment
                                * c.game.teams[self.side].penalty_counter)
                            .try_into()
                            .unwrap()
                    }),
                    run_condition: RunCondition::ReadyOrPlaying,
                    // Motion in Set is removed automatically.
                    behavior_at_zero: if penalty == Penalty::MotionInSet {
                        BehaviorAtZero::Expire(vec![VAction::Unpenalize(Unpenalize {
                            side: self.side,
                            player: self.player,
                            force: false,
                        })])
                    } else {
                        BehaviorAtZero::Clip
                    },
                },
            };
            c.game.teams[self.side][self.player].penalty = penalty;
            if c.params.competition.penalties[penalty].incremental {
                c.game.teams[self.side].penalty_counter += 1;
            }
        }
    }

    fn is_legal(&self, c: &ActionContext) -> bool {
        c.game.teams[self.side][self.player].penalty == Penalty::NoPenalty
            && (match self.call {
                PenaltyCall::RequestForPickUp => true,
                PenaltyCall::IllegalPosition => {
                    c.game.phase != Phase::PenaltyShootout
                        && (c.game.state == State::Ready // Not possible in this state, but can
                                                         // happen if it happens shortly before a
                                                         // goal and the GameController presses the
                                                         // goal first.
                            || c.game.state == State::Set
                            || c.game.state == State::Playing)
                }
                PenaltyCall::MotionInSet => c.game.state == State::Set,
                PenaltyCall::IncapableRobot => {
                    c.game.state == State::Ready
                        || c.game.state == State::Set
                        || c.game.state == State::Playing
                }
                PenaltyCall::LocalGameStuck => {
                    c.game.phase != Phase::PenaltyShootout && c.game.state == State::Playing
                }
                PenaltyCall::BallHolding => {
                    c.game.state == State::Ready // Not possible in this state, but can happen in
                                                 // Playing shortly before a goal and the
                                                 // GameController operator clicks the goal first.
                        || c.game.state == State::Playing
                }
                PenaltyCall::Pushing => {
                    // Not possible in Set, but can happen in Ready shortly before the timer
                    // expires.
                    (c.game.phase != Phase::PenaltyShootout
                        && (c.game.state == State::Ready || c.game.state == State::Set))
                        || c.game.state == State::Playing
                }
                PenaltyCall::PlayingWithArmsHands => {
                    c.game.state == State::Ready // Not possible in this state, but can happen in
                                                 // Playing shortly before a goal and the
                                                 // GameController oprtator clicks the goal first.
                        || c.game.state == State::Playing
                }
                PenaltyCall::LeavingTheField => {
                    // Not possible in Set, but can happen in Ready shortly before the timer
                    // expires.
                    (c.game.phase != Phase::PenaltyShootout
                        && (c.game.state == State::Ready || c.game.state == State::Set))
                        || c.game.state == State::Playing
                }
                PenaltyCall::Warn => {
                    // "Manual Interaction by Team Members" is always possible. Technically even if
                    // the player already has a penalty.
                    true
                }
                PenaltyCall::Caution => {
                    // "Damage to the Field" is always possible. Technically even if the player
                    // already has penalty.
                    true
                }
                PenaltyCall::SendOff => true,
            })
    }
}

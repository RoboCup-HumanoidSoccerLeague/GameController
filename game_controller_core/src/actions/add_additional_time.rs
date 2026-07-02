use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::action::{Action, ActionContext};
use crate::timer::{BehaviorAtZero, RunCondition, Timer};
use crate::types::{Phase, State};

/// This struct defines an action that adds a minute of additional time.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AddAdditionalTime;

impl AddAdditionalTime {
    const MINUTE: Duration = Duration::from_secs(60);
}

impl Action for AddAdditionalTime {
    fn execute(&self, c: &mut ActionContext) {
        c.game.primary_timer = Timer::Started {
            remaining: c.game.primary_timer.get_remaining() + Self::MINUTE,
            run_condition: RunCondition::MainTimer,
            behavior_at_zero: BehaviorAtZero::Overflow,
        };
        c.game
            .teams
            .values_mut()
            .filter(|team| !team.illegal_communication)
            .for_each(|team| {
                team.message_budget += c.params.competition.messages_per_team_per_additional_minute;
            });
    }

    fn is_legal(&self, c: &ActionContext) -> bool {
        !matches!(c.game.state, State::Initial | State::Finished)
            && matches!(c.game.primary_timer, Timer::Started { .. })
            && match c.game.phase {
                Phase::FirstHalf | Phase::SecondHalf => Some(c.params.competition.half_duration),
                Phase::FirstExtraHalf | Phase::SecondExtraHalf => {
                    c.params.competition.extra_half_duration
                }
                Phase::PenaltyShootout => None,
            }
            .is_some_and(|duration| c.game.primary_timer.get_remaining() + Self::MINUTE < duration)
    }
}

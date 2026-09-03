use serde::{Deserialize, Serialize};

use crate::action::{Action, ActionContext};
use crate::actions::{FinishHalf, FinishPenaltyShot, StartSetPlay};
use crate::types::{Phase, SetPlay, Side, State};

/// This struct defines an action for when a goal has been scored.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    /// The side which has scored a goal.
    pub side: Side,
}

impl Action for Goal {
    fn execute(&self, c: &mut ActionContext) {
        // Mercy rule: At a certain goal difference, the game is finished.
        let mercy_rule = c
            .params
            .competition
            .mercy_rule_score_difference
            .is_some_and(|mercy_rule_score_difference| {
                c.game.phase != Phase::PenaltyShootout
                    && !c.game.teams[self.side].illegal_communication
                    && (c.game.teams[self.side].score + 1)
                        >= c.game.teams[-self.side].score + mercy_rule_score_difference
            });

        if !c.game.teams[self.side].illegal_communication {
            c.game.teams[self.side].score += 1;
        }
        if mercy_rule {
            c.game.phase = match c.game.phase {
                Phase::FirstHalf | Phase::SecondHalf => Phase::SecondHalf,
                Phase::FirstExtraHalf | Phase::SecondExtraHalf => Phase::SecondExtraHalf,
                Phase::PenaltyShootout => panic!("only possible outside of penalty shoot-out"),
            };
            FinishHalf.execute(c);
        } else if c.game.phase != Phase::PenaltyShootout {
            // A kick-off for the other team.
            StartSetPlay {
                side: Some(-self.side),
                set_play: SetPlay::KickOff,
            }
            .execute(c);
        } else {
            c.game.teams[self.side].penalty_shot_mask |=
                1u16 << (c.game.teams[self.side].penalty_shot - 1);
            FinishPenaltyShot.execute(c);
        }
    }

    fn is_legal(&self, c: &ActionContext) -> bool {
        c.game.state == State::Playing
            && (c.game.phase != Phase::PenaltyShootout
                || c.game.kicking_side.is_none_or(|side| side == self.side))
    }
}

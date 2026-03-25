use serde::{Deserialize, Serialize};

use crate::action::{Action, ActionContext};

/// This struct defines an action to start or resume play ("(emergency) stop").
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StopPlay {
    /// Whether play should be stopped (false) or resumed (true).
    pub resume: bool,
}

impl Action for StopPlay {
    fn execute(&self, c: &mut ActionContext) {
        c.game.stopped = !self.resume;
    }

    fn is_legal(&self, c: &ActionContext) -> bool {
        c.game.stopped == self.resume
    }
}

//! This module defines example game situations for screenshots. Each scenario drives the real
//! [GameController] through a sequence of actions, so that the resulting states (including the
//! legality of every button) are genuine. The same scenarios are rendered by the egui UI and, for
//! comparison, by the original web UI (see `tools/screenshots`).

use std::time::Duration;

use enum_map::EnumMap;

use game_controller_core::{
    action::VAction,
    actions::{
        FinishHalf, FreePenaltyShot, FreeSetPlay, Goal, Penalize, SelectPenaltyShotPlayer,
        StartExtraTime, StartPenaltyShootout, StartSetPlay, StopPlay, SwitchHalf, Timeout,
        WaitForPenaltyShot, WaitForSetPlay,
    },
    log::NullLogger,
    types::{ActionSource, Params, PenaltyCall, PlayerNumber, SetPlay, Side, SideMapping},
    GameController,
};
use game_controller_runtime::{connection_status::ConnectionStatus, UiState};

use crate::actions::NUM_OF_PLAYERS;

/// A UI interaction that is performed after the state has been set (by clicking the n-th button
/// with the given label).
#[derive(Clone, Debug)]
pub struct Click {
    pub label: &'static str,
    pub index: usize,
}

/// A rendered situation.
pub struct Scenario {
    /// The file name stem of the screenshot.
    pub name: &'static str,
    /// The parameters of the game.
    pub params: Params,
    /// The state that is passed to the UI. Its legality mask is computed for the action list given
    /// to [make_scenarios].
    pub state: UiState,
    /// The clicks to perform before taking the screenshot.
    pub clicks: Vec<Click>,
}

/// This trait wraps concrete actions into [VAction] (the core has no `From` implementations).
trait IntoVAction {
    fn into_vaction(self) -> VAction;
}

macro_rules! impl_into_vaction {
    ($($action:ident),*) => {
        $(impl IntoVAction for $action {
            fn into_vaction(self) -> VAction {
                VAction::$action(self)
            }
        })*
    };
}

impl_into_vaction!(
    FinishHalf,
    FreePenaltyShot,
    FreeSetPlay,
    Goal,
    Penalize,
    SelectPenaltyShotPlayer,
    StartExtraTime,
    StartPenaltyShootout,
    StartSetPlay,
    StopPlay,
    SwitchHalf,
    Timeout,
    WaitForPenaltyShot,
    WaitForSetPlay
);

/// A small driver around a [GameController] that panics if a scenario applies an illegal action,
/// so scenarios can't silently drift from the rules.
struct Sim {
    gc: GameController,
}

impl Sim {
    fn new(params: Params) -> Self {
        Self {
            gc: GameController::new(params, Box::new(NullLogger)),
        }
    }

    fn act(&mut self, action: impl IntoVAction) -> &mut Self {
        let action = action.into_vaction();
        assert!(
            action.is_legal(&self.gc.get_context(false)),
            "illegal scenario action {action:?}"
        );
        self.gc.apply(action, ActionSource::User);
        self
    }

    fn wait(&mut self, seconds: f32) -> &mut Self {
        self.gc.seek(Duration::from_secs_f32(seconds));
        self
    }

    fn kick_off_and_play(&mut self, side: Option<Side>) -> &mut Self {
        self.act(StartSetPlay {
            side,
            set_play: SetPlay::KickOff,
        })
        .set_and_play()
    }

    /// This function goes from Ready to Playing.
    fn set_and_play(&mut self) -> &mut Self {
        self.wait(20.0)
            .act(WaitForSetPlay)
            .wait(3.0)
            .act(FreeSetPlay)
    }

    fn state(&mut self, declared: &[VAction]) -> UiState {
        let legal_actions = {
            let context = self.gc.get_context(false);
            declared.iter().map(|a| a.is_legal(&context)).collect()
        };
        UiState {
            connection_status: connection_pattern(),
            game: self.gc.get_game(false).clone(),
            legal_actions,
            undo_actions: self.gc.get_undo_actions(5),
        }
    }
}

/// A fixed mixture of connection states, so all three icons appear.
fn connection_pattern() -> EnumMap<Side, [ConnectionStatus; NUM_OF_PLAYERS]> {
    let pattern = |offset: usize| {
        std::array::from_fn(|i| match (i + offset) % 7 {
            3 => ConnectionStatus::Bad,
            5 => ConnectionStatus::Offline,
            _ => ConnectionStatus::Good,
        })
    };
    EnumMap::from_array([pattern(0), pattern(2)])
}

fn player(n: u8) -> PlayerNumber {
    PlayerNumber::new(n)
}

/// This function builds all scenarios for the given parameters. `declared` is the list of actions
/// for which the legality mask is computed (the egui list or the one of the web UI).
pub fn make_scenarios(params: &Params, declared: &[VAction]) -> Vec<Scenario> {
    let mirrored = {
        let mut params = params.clone();
        params.game.side_mapping = SideMapping::HomeDefendsRightGoal;
        params.game.kick_off_side = Side::Away;
        params
    };
    let mut scenarios = Vec::new();
    let mut add = |name, params: &Params, sim: &mut Sim, clicks: Vec<Click>| {
        scenarios.push(Scenario {
            name,
            params: params.clone(),
            state: sim.state(declared),
            clicks,
        });
    };

    add("01_initial", params, &mut Sim::new(params.clone()), vec![]);

    let mut sim = Sim::new(params.clone());
    sim.act(StartSetPlay {
        side: Some(Side::Home),
        set_play: SetPlay::KickOff,
    })
    .wait(12.4);
    add("02_ready", params, &mut sim, vec![]);

    // A game in progress: a goal, a few penalties and a free kick.
    let playing = |sim: &mut Sim| {
        sim.kick_off_and_play(Some(Side::Home))
            .wait(95.0)
            // A goal starts the kick-off of the other team.
            .act(Goal { side: Side::Home })
            .set_and_play()
            .wait(130.2)
            .act(Penalize {
                side: Side::Home,
                player: player(3),
                call: PenaltyCall::Pushing,
            })
            .wait(14.0)
            .act(Penalize {
                side: Side::Away,
                player: player(2),
                call: PenaltyCall::IncapableRobot,
            })
            .wait(6.0)
            .act(Penalize {
                side: Side::Away,
                player: player(5),
                call: PenaltyCall::RequestForPickUp,
            })
            .wait(3.0);
    };
    let mut sim = Sim::new(params.clone());
    playing(&mut sim);
    add("03_playing", params, &mut sim, vec![]);
    add(
        "04_penalty_selected",
        params,
        &mut sim,
        vec![Click {
            label: "Pushing",
            index: 0,
        }],
    );
    add(
        "05_substitute",
        params,
        &mut sim,
        vec![Click {
            label: "Substitute",
            index: 0,
        }],
    );
    sim.act(StartSetPlay {
        side: Some(Side::Away),
        set_play: SetPlay::CornerKick,
    })
    .wait(4.0);
    add("06_corner_kick", params, &mut sim, vec![]);
    // The corner kick has stopped play; it is resumed and then stopped by the operator.
    sim.act(StopPlay { resume: true })
        .wait(8.0)
        .act(StopPlay { resume: false })
        .wait(2.0);
    add("07_stopped", params, &mut sim, vec![]);

    let mut sim = Sim::new(params.clone());
    sim.kick_off_and_play(Some(Side::Home))
        .wait(300.0)
        .act(FinishHalf)
        .wait(65.0);
    add("08_half_time", params, &mut sim, vec![]);
    sim.act(SwitchHalf)
        .act(Timeout {
            side: Some(Side::Away),
        })
        .wait(17.0);
    add("09_timeout", params, &mut sim, vec![]);

    let mut sim = Sim::new(params.clone());
    sim.kick_off_and_play(Some(Side::Home))
        .wait(60.0)
        .act(FinishHalf)
        .act(SwitchHalf)
        .kick_off_and_play(Some(Side::Away))
        .wait(60.0)
        .act(FinishHalf)
        .wait(5.0);
    add("10_full_time", params, &mut sim, vec![]);
    if params.competition.extra_half_duration.is_some() {
        sim.act(StartExtraTime)
            .kick_off_and_play(Some(Side::Home))
            .wait(30.0)
            .act(FinishHalf)
            .act(SwitchHalf)
            .kick_off_and_play(Some(Side::Away))
            .wait(30.0)
            .act(FinishHalf)
            .wait(5.0);
    }
    sim.act(StartPenaltyShootout {
        sides: SideMapping::HomeDefendsLeftGoal,
    })
    // The home team takes the first shot.
    .act(SelectPenaltyShotPlayer {
        side: Side::Home,
        player: player(4),
        goalkeeper: false,
    })
    .act(SelectPenaltyShotPlayer {
        side: Side::Away,
        player: player(1),
        goalkeeper: true,
    })
    .act(WaitForPenaltyShot)
    .wait(4.0)
    .act(FreePenaltyShot)
    .wait(11.0)
    .act(Goal { side: Side::Home })
    .wait(2.0);
    add("11_penalty_shootout", params, &mut sim, vec![]);

    let mut sim = Sim::new(mirrored.clone());
    sim.kick_off_and_play(Some(Side::Away))
        .wait(42.0)
        .act(Penalize {
            side: Side::Away,
            player: player(1),
            call: PenaltyCall::BallHolding,
        })
        .wait(1.0);
    add("12_mirrored", &mirrored, &mut sim, vec![]);

    scenarios
}

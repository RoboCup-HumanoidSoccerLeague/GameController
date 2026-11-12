//! This module implements the main window from which a game is operated.
//!
//! The layout consists of two team panels with the center panel (clock, state buttons, penalty
//! calls) in between and a row of undo buttons below. All sizes are multiples of a "rem" that scales
//! with the window, so that the rows of the team panels line up with those of the center panel.

use std::time::Duration;

use eframe::egui::{self, Align2, Color32, Id, Pos2, Rect, Ui, Vec2};
use enum_map::EnumMap;

use game_controller_core::{
    action::VAction,
    actions::{Penalize, SelectGoalkeeper, SelectPenaltyShotPlayer, Substitute, Undo, Unpenalize},
    timer::{SignedDuration, Timer},
    types::{
        Game, Params, Penalty, PenaltyCall, Phase, PlayerNumber, SetPlay, Side, SideMapping, State,
    },
};
use game_controller_runtime::{
    connection_status::{ConnectionStatus, ConnectionStatusMap},
    UiState,
};

use crate::{
    actions::{GameAction, Legal, TeamAction, NUM_OF_UNDO_ACTIONS, PENALTIES},
    style::{
        self, accessible_text, button_frame, color_name, font, icons, jersey_background,
        jersey_text, paint_galley, paint_tabular, text_button, tw, ButtonState, Weight, FLASH,
    },
};

/// The state of the player selection of a team panel (substitution, goalkeeper selection, or
/// penalty shooter selection).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SelectState {
    #[default]
    Default,
    Goalkeeper,
    PlayerOut,
    PlayerIn {
        player_out: u8,
    },
    /// The jersey (goalkeeper or field player) of the next penalty shooter/keeper is selected.
    PsoColor,
    PsoPlayer {
        goalkeeper: bool,
    },
}

/// The main window.
pub struct MainView {
    params: Params,
    team_names: EnumMap<Side, String>,
    selected_penalty_call: Option<usize>,
    select_state: EnumMap<Side, SelectState>,
}

/// The geometry unit. All lengths of the layout are multiples of it.
#[derive(Clone, Copy)]
struct Rem(f32);

impl Rem {
    /// The rem is 1.1% of the width or 1.9% of the height (whichever is smaller), but at least 16
    /// and at most 40 points.
    fn for_screen(size: Vec2) -> Self {
        Self((size.x * 0.011).min(size.y * 0.019).clamp(16.0, 40.0))
    }

    fn of(self, n: f32) -> f32 {
        self.0 * n
    }
}

const GAP: f32 = 0.5;
const ROW_H: f32 = 4.0;
const CLOCK_H: f32 = 11.0;
const STATE_H: f32 = 2.0 * ROW_H + GAP;
const TEAM_HEADER_H: f32 = CLOCK_H + GAP - 2.0 * ROW_H - 3.0 * GAP;
/// The flex basis, minimum and maximum width of a team panel.
const TEAM_BASIS: f32 = 18.125;
const TEAM_MAX: f32 = 24.0;

/// This function returns cell `i` of a rectangle that is split into `n` equal cells along the x
/// axis with gaps.
fn nth_column(rect: Rect, n: usize, gap: f32, i: usize) -> Rect {
    let w = (rect.width() - gap * (n as f32 - 1.0)) / n as f32;
    let x = rect.left() + i as f32 * (w + gap);
    Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height()))
}

/// This function returns cell `i` of a rectangle that is split into `n` equal cells along the y
/// axis with gaps.
fn nth_row(rect: Rect, n: usize, gap: f32, i: usize) -> Rect {
    let h = (rect.height() - gap * (n as f32 - 1.0)) / n as f32;
    let y = rect.top() + i as f32 * (h + gap);
    Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), h))
}

/// This function returns whether a key was pressed in this frame and removes its presses from the
/// input, so that widgets that are created afterwards don't react to them.
fn consume_key_presses(ui: &Ui, key: egui::Key) -> bool {
    ui.input_mut(|i| {
        let before = i.events.len();
        i.events.retain(
            |event| !matches!(event, egui::Event::Key { key: k, pressed: true, .. } if *k == key),
        );
        i.events.len() != before
    })
}

/// This function returns the union of two cells.
fn span(a: Rect, b: Rect) -> Rect {
    a.union(b)
}

/// This function formats a timer as `[-]MM:SS` (rounding up).
pub fn format_mmss(timer: &Timer) -> String {
    let seconds = match timer {
        Timer::Started { remaining, .. } => ceil_seconds(*remaining),
        Timer::Stopped => 0,
    };
    let sign = if seconds < 0 { "-" } else { "" };
    let seconds = seconds.unsigned_abs();
    format!("{sign}{:02}:{:02}", seconds / 60, seconds % 60)
}

fn ceil_seconds(duration: SignedDuration) -> i64 {
    duration.whole_seconds() + i64::from(duration.subsec_nanoseconds() > 0)
}

/// This function returns whether a timer shows less than ten (whole) seconds.
fn below_ten_seconds(timer: &Timer) -> bool {
    matches!(timer, Timer::Started { remaining, .. } if remaining.whole_seconds() < 10)
}

fn phase_description(game: &Game) -> &'static str {
    match game.phase {
        Phase::FirstHalf if game.state == State::Finished => "Half-Time Break",
        Phase::FirstHalf => "First Half",
        Phase::SecondHalf if game.state == State::Initial => "Half-Time Break",
        Phase::SecondHalf => "Second Half",
        Phase::FirstExtraHalf if game.state == State::Finished => "Extra Time",
        Phase::FirstExtraHalf => "First Extra Half",
        Phase::SecondExtraHalf if game.state == State::Initial => "Extra Time",
        Phase::SecondExtraHalf => "Second Extra Half",
        Phase::PenaltyShootout => "Penalty Shoot-out",
    }
}

fn set_play_description(set_play: SetPlay) -> &'static str {
    match set_play {
        SetPlay::NoSetPlay => "",
        SetPlay::KickOff => "Kick-off",
        SetPlay::DirectFreeKick => "Direct Free Kick",
        SetPlay::IndirectFreeKick => "Indirect Free Kick",
        SetPlay::PenaltyKick => "Penalty Kick",
        SetPlay::ThrowIn => "Throw-in",
        SetPlay::GoalKick => "Goal Kick",
        SetPlay::CornerKick => "Corner Kick",
    }
}

fn state_description(game: &Game) -> String {
    match game.state {
        State::Timeout => return "Timeout".into(),
        State::Initial => return "Initial".into(),
        State::Finished => return "Finished".into(),
        _ => {}
    }
    let prefix = set_play_description(game.set_play);
    let state = match game.state {
        State::Ready => " Ready",
        State::Set => " Set",
        _ if prefix.is_empty() => "Playing",
        _ => "",
    };
    format!("{prefix}{state}")
}

fn penalty_description(penalty: Penalty) -> &'static str {
    match penalty {
        Penalty::NoPenalty => "No Penalty",
        Penalty::IllegalPositioning => "Illegal Positioning",
        Penalty::MotionInSet => "Motion in Set",
        Penalty::MotionInStop => "Motion in Stop",
        Penalty::LocalGameStuck => "Local Game Stuck",
        Penalty::IncapableRobot => "Incapable Robot",
        Penalty::PickedUp => "Picked Up",
        Penalty::BallHolding => "Ball Holding",
        Penalty::LeavingTheField => "Leaving the Field",
        Penalty::PlayingWithArmsHands => "Arms / Hands",
        Penalty::Pushing => "Pushing",
        Penalty::Cautioned => "Cautioned",
        Penalty::SentOff => "Sent Off",
        Penalty::Substitute => "Substitute",
    }
}

/// This function returns the label of an undo button for the action that it would undo.
fn action_name(action: &VAction) -> String {
    match action {
        VAction::AddAdditionalTime(_) => "Add Additional Time".into(),
        VAction::FinishHalf(_) | VAction::FinishPenaltyShot(_) => "Finish".into(),
        VAction::FinishSetPlay(_) => "Set Play Complete".into(),
        VAction::FreePenaltyShot(_) | VAction::FreeSetPlay(_) => "Playing".into(),
        VAction::GlobalGameStuck(_) => "Global Game Stuck".into(),
        VAction::Goal(_) => "Goal".into(),
        VAction::Penalize(penalize) => PENALTIES
            .iter()
            .find(|(_, call)| *call == penalize.call)
            .map_or("Penalize", |(label, _)| label)
            .into(),
        VAction::SelectGoalkeeper(_) | VAction::SelectPenaltyShotPlayer(_) => "Select".into(),
        VAction::StartExtraTime(_) => "Extra Time".into(),
        VAction::StartPenaltyShootout(_) => "Penalty Shoot-out".into(),
        VAction::StartSetPlay(start) => match start.set_play {
            SetPlay::KickOff => "Ready".into(),
            set_play => set_play_description(set_play).into(),
        },
        VAction::StopPlay(stop) => if stop.resume {
            "Resume Play"
        } else {
            "Stop Play"
        }
        .into(),
        VAction::Substitute(_) => "Substitute".into(),
        VAction::SwitchHalf(_) => "Second Half".into(),
        VAction::TeamMessage(_) => "Team Message".into(),
        VAction::Timeout(timeout) => if timeout.side.is_some() {
            "Timeout"
        } else {
            "Referee Timeout"
        }
        .into(),
        VAction::Undo(_) => "Undo".into(),
        VAction::Unpenalize(_) => "Unpenalize".into(),
        VAction::WaitForPenaltyShot(_) | VAction::WaitForSetPlay(_) => "Set".into(),
    }
}

/// Everything that is needed to draw one frame.
struct Ctx<'a> {
    game: &'a Game,
    legal: Legal<'a>,
    connection_status: &'a ConnectionStatusMap,
    rem: Rem,
    /// Whether flashing elements currently show their flash color.
    flash_on: bool,
    /// Whether the shift key is held (alternative button functions).
    alternative: bool,
    actions: Vec<VAction>,
    /// Whether something is flashing (so repaints must be scheduled).
    flashing: bool,
}

impl MainView {
    /// This function creates the main window for a game with given parameters and team names
    /// (home, away).
    pub fn new(params: Params, team_names: [String; 2]) -> Self {
        let [home, away] = team_names;
        Self {
            params,
            team_names: EnumMap::from_array([home, away]),
            selected_penalty_call: None,
            select_state: EnumMap::default(),
        }
    }

    /// This function draws the main window for a state and returns the actions that the user
    /// triggered. Nothing is drawn if the state doesn't (yet) contain the legality of the declared
    /// actions.
    pub fn ui(&mut self, ui: &mut Ui, state: &UiState) -> Vec<VAction> {
        let screen = ui.max_rect();
        ui.painter().rect_filled(screen, 0.0, tw::WHITE);
        let Some(legal) = Legal::new(&state.legal_actions) else {
            return vec![];
        };

        // A selected penalty call that is not legal anymore is deselected.
        if self
            .selected_penalty_call
            .is_some_and(|call| !legal.penalty_call(call))
        {
            self.selected_penalty_call = None;
        }

        let time = ui.input(|i| i.time);
        let mut cx = Ctx {
            game: &state.game,
            legal,
            connection_status: &state.connection_status,
            rem: Rem::for_screen(screen.size()),
            flash_on: time.rem_euclid(1.0) < 0.5,
            alternative: ui.input(|i| i.modifiers.shift),
            actions: vec![],
            flashing: false,
        };

        // The space bar stops play (an emergency stop that doesn't need the mouse). The key is
        // consumed before the buttons are created, because it would otherwise also click the
        // button that has the keyboard focus.
        if consume_key_presses(ui, egui::Key::Space) {
            cx.actions.push(GameAction::StopPlay.action());
        }

        let r = cx.rem;
        let content = screen.shrink(r.of(0.5));
        let undo_h = r.of(2.5);
        let main = Rect::from_min_max(
            content.min,
            Pos2::new(content.right(), content.bottom() - undo_h - r.of(1.0)),
        );
        let undo = Rect::from_min_max(
            Pos2::new(content.left(), content.bottom() - undo_h),
            content.max,
        );

        // The team panels are at least 18.125rem wide and grow half as much as the center panel,
        // but no further than 24rem.
        let gap = r.of(1.0);
        let free = main.width() - 2.0 * gap;
        let center_basis = r.of(CENTER_BASIS);
        let grow_unit = ((free - 2.0 * r.of(TEAM_BASIS) - center_basis) / 4.0).max(0.0);
        let team_w = (r.of(TEAM_BASIS) + grow_unit).min(r.of(TEAM_MAX));
        let center_w = free - 2.0 * team_w;
        let left = Rect::from_min_size(main.min, Vec2::new(team_w, main.height()));
        let center = Rect::from_min_size(
            Pos2::new(left.right() + gap, main.top()),
            Vec2::new(center_w, main.height()),
        );
        let right = Rect::from_min_size(
            Pos2::new(center.right() + gap, main.top()),
            Vec2::new(team_w, main.height()),
        );
        let (home_rect, away_rect) = match state.game.sides {
            SideMapping::HomeDefendsLeftGoal => (left, right),
            SideMapping::HomeDefendsRightGoal => (right, left),
        };
        let mirror = state.game.sides == SideMapping::HomeDefendsRightGoal;

        self.team_panel(
            ui,
            &mut cx,
            Side::Home,
            home_rect,
            if mirror { -1.0 } else { 1.0 },
        );
        self.center_panel(ui, &mut cx, center);
        self.team_panel(
            ui,
            &mut cx,
            Side::Away,
            away_rect,
            if mirror { 1.0 } else { -1.0 },
        );
        undo_panel(ui, &mut cx, undo, &state.undo_actions);

        if cx.flashing {
            // Repaint at the next flash phase change.
            let to_next = 0.5 - time.rem_euclid(0.5);
            ui.ctx()
                .request_repaint_after(Duration::from_secs_f64(to_next.max(0.01)));
        }
        cx.actions
    }

    fn center_panel(&mut self, ui: &mut Ui, cx: &mut Ctx, rect: Rect) {
        let r = cx.rem;
        let clock = Rect::from_min_size(rect.min, Vec2::new(rect.width(), r.of(CLOCK_H)));
        let state = Rect::from_min_size(
            Pos2::new(rect.left(), clock.bottom() + r.of(GAP)),
            Vec2::new(rect.width(), r.of(STATE_H)),
        );
        let penalties =
            Rect::from_min_max(Pos2::new(rect.left(), state.bottom() + r.of(GAP)), rect.max);
        clock_panel(ui, cx, clock);
        self.state_panel(ui, cx, state);
        self.penalty_panel(ui, cx, penalties);
    }

    fn state_panel(&mut self, ui: &mut Ui, cx: &mut Ctx, rect: Rect) {
        let game = cx.game;
        let in_half_time_break = (matches!(game.phase, Phase::FirstHalf | Phase::FirstExtraHalf)
            && game.state == State::Finished)
            || (matches!(game.phase, Phase::SecondHalf | Phase::SecondExtraHalf)
                && game.state == State::Initial);
        let has_extra_time = self.params.competition.extra_half_duration.is_some();
        let pso = game.phase == Phase::PenaltyShootout;
        let in_play = matches!(game.state, State::Ready | State::Set | State::Playing);

        // (column span, label, game action, active)
        let mut items: Vec<(usize, &str, GameAction, bool)> = vec![];
        items.push((
            5,
            if game.stopped {
                "Resume Play"
            } else {
                "Stop Play"
            },
            if game.stopped {
                GameAction::ResumePlay
            } else {
                GameAction::StopPlay
            },
            game.stopped,
        ));
        // This button is still displayed when we are already in the Initial state of the second
        // half. This is because the state can switch automatically to the second half and it
        // would be bad if the operator clicked the button exactly at that time, but the button
        // switches its meaning to Ready before the button is actually clicked.
        if in_half_time_break {
            items.push((1, "Second Half", GameAction::SwitchHalf, false));
        }
        let last_regular_phase = if has_extra_time {
            Phase::SecondExtraHalf
        } else {
            Phase::SecondHalf
        };
        if game.phase == last_regular_phase && game.state == State::Finished {
            items.push((
                2,
                "Penalty Shots (Left Goal)",
                GameAction::StartPenaltyShootoutRight,
                false,
            ));
            items.push((
                2,
                "Penalty Shots (Right Goal)",
                GameAction::StartPenaltyShootoutLeft,
                false,
            ));
        }
        if game.phase == Phase::SecondHalf && game.state == State::Finished && has_extra_time {
            items.push((4, "Extra Time", GameAction::StartExtraTime, false));
        }
        if !pso
            && (matches!(game.state, State::Initial | State::Timeout)
                || (matches!(game.phase, Phase::FirstHalf | Phase::FirstExtraHalf)
                    && game.state == State::Finished))
        {
            items.push((
                if in_half_time_break { 3 } else { 4 },
                "Ready",
                GameAction::kick_off(game.kicking_side),
                false,
            ));
        }
        if !pso && game.state == State::Playing {
            items.push((1, "Global GS", GameAction::GlobalGameStuck, false));
        }
        if pso || matches!(game.state, State::Ready | State::Set) {
            items.push((
                1,
                "Set",
                if pso {
                    GameAction::WaitForPenaltyShot
                } else {
                    GameAction::WaitForSetPlay
                },
                false,
            ));
        }
        if pso || in_play {
            items.push((
                if pso { 2 } else { 1 },
                "Playing",
                if pso {
                    GameAction::FreePenaltyShot
                } else {
                    GameAction::FreeSetPlay
                },
                false,
            ));
        }
        if !pso && in_play {
            items.push((1, "Ball Free", GameAction::FinishSetPlay, false));
        }
        if pso || in_play {
            items.push((
                1,
                "Finish",
                if pso {
                    GameAction::FinishPenaltyShot
                } else {
                    GameAction::FinishHalf
                },
                false,
            ));
        }
        items.push((1, "Referee Timeout", GameAction::RefereeTimeout, false));

        // CSS grid auto-placement in five columns: an item that doesn't fit into the rest of a
        // row starts the next row. All rows have the same height.
        let mut placed = vec![];
        let (mut row, mut col) = (0usize, 0usize);
        for item in items {
            if col + item.0 > 5 {
                row += 1;
                col = 0;
            }
            placed.push((row, col, item));
            col += item.0;
        }
        let r = cx.rem;
        let num_rows = row + 1;
        for (row_index, col, (span_n, label, action, active)) in placed {
            let cells = nth_row(rect, num_rows, r.of(GAP), row_index);
            let cell = span(
                nth_column(cells, 5, r.of(GAP), col),
                nth_column(cells, 5, r.of(GAP), col + span_n - 1),
            );
            if text_button(
                ui,
                cell,
                Id::unique(("state", label)),
                label,
                ButtonState {
                    active,
                    legal: cx.legal.game(action),
                },
                r.of(1.0),
            ) {
                cx.actions.push(action.action());
            }
        }
    }

    fn penalty_panel(&mut self, ui: &mut Ui, cx: &mut Ctx, rect: Rect) {
        let r = cx.rem;
        let num_rows = PENALTIES.len().div_ceil(2);
        for (index, (label, _)) in PENALTIES.iter().enumerate() {
            let cells = nth_row(rect, num_rows, r.of(GAP), index / 2);
            let cell = nth_column(cells, 2, r.of(GAP), index % 2);
            let selected = self.selected_penalty_call == Some(index);
            if text_button(
                ui,
                cell,
                Id::unique(("penalty", index)),
                label,
                ButtonState {
                    active: selected,
                    legal: cx.legal.penalty_call(index),
                },
                r.of(1.0),
            ) {
                self.selected_penalty_call = if selected { None } else { Some(index) };
            }
        }
    }

    fn team_panel(&mut self, ui: &mut Ui, cx: &mut Ctx, side: Side, rect: Rect, sign: f32) {
        let r = cx.rem;
        let game = cx.game;
        let team_params = &self.params.game.teams[side];
        let pso = game.phase == Phase::PenaltyShootout;

        // Header: the kicking side indicator and the team name in the jersey color.
        let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), r.of(TEAM_HEADER_H)));
        {
            let painter = ui.painter_at(header);
            let mut size = r.of(1.5);
            let dot = 14.0;
            let name_galley = loop {
                let g = painter.layout_no_wrap(
                    self.team_names[side].clone(),
                    font(size, Weight::SemiBold),
                    jersey_text(team_params.field_player_color),
                );
                if g.size().x + dot + r.of(0.5) <= header.width() || size < 10.0 {
                    break g;
                }
                size -= 1.0;
            };
            let total = dot + r.of(0.5) + name_galley.size().x;
            let x0 = header.center().x - total / 2.0;
            if game.kicking_side == Some(side) {
                painter.circle_filled(
                    Pos2::new(x0 + dot / 2.0, header.center().y),
                    dot / 2.0,
                    tw::BLACK,
                );
            }
            paint_galley(
                &painter,
                name_galley,
                Align2::LEFT_CENTER,
                Pos2::new(x0 + dot + r.of(0.5), header.center().y),
            );
            let kicking = if game.kicking_side == Some(side) {
                " (kicking)"
            } else {
                ""
            };
            accessible_text(
                ui,
                header,
                Id::unique(("team name", side)),
                egui::Role::Heading,
                &format!("{}{kicking}", self.team_names[side]),
                "",
            );
        }

        // The grid of team actions: 3 columns x 4 rows.
        let grid = Rect::from_min_size(
            Pos2::new(rect.left(), header.bottom() + r.of(GAP)),
            Vec2::new(rect.width(), 4.0 * r.of(ROW_H) + 3.0 * r.of(GAP)),
        );
        let cell = |col: usize, row_index: usize| {
            nth_row(nth_column(grid, 3, r.of(GAP), col), 4, r.of(GAP), row_index)
        };
        let (outer, inner) = if sign > 0.0 { (0, 2) } else { (2, 0) };

        let select_state = self.select_state[side];
        let select_label = if pso || cx.alternative || select_state == SelectState::Goalkeeper {
            "Select"
        } else {
            "Substitute"
        };
        let button_id = |name: &str| Id::unique(("team", side, name.to_owned()));
        if text_button(
            ui,
            cell(outer, 0),
            button_id("select"),
            select_label,
            ButtonState {
                active: select_state != SelectState::Default,
                legal: true,
            },
            r.of(1.0),
        ) {
            self.select_state[side] = if select_state != SelectState::Default {
                SelectState::Default
            } else if pso {
                if team_params.goalkeeper_color == team_params.field_player_color {
                    // If the goalkeeper doesn't have a special jersey color, its selection can be
                    // skipped.
                    SelectState::PsoPlayer {
                        goalkeeper: game.kicking_side != Some(side),
                    }
                } else {
                    SelectState::PsoColor
                }
            } else if cx.alternative {
                SelectState::Goalkeeper
            } else {
                SelectState::PlayerOut
            };
        }

        let team_button =
            |ui: &mut Ui, cx: &mut Ctx, rect: Rect, label: &str, action: TeamAction| {
                let active = action.set_play().is_some_and(|set_play| {
                    game.set_play == set_play && game.kicking_side == Some(side)
                });
                if text_button(
                    ui,
                    rect,
                    button_id(label),
                    label,
                    ButtonState {
                        active,
                        legal: cx.legal.team(side, action),
                    },
                    r.of(1.0),
                ) {
                    cx.actions.push(action.action(side));
                }
            };
        team_button(ui, cx, cell(outer, 1), "Timeout", TeamAction::Timeout);
        team_button(
            ui,
            cx,
            cell(outer, 2),
            "Direct Free Kick",
            TeamAction::DirectFreeKick,
        );
        team_button(ui, cx, cell(outer, 3), "Throw-in", TeamAction::ThrowIn);
        team_button(
            ui,
            cx,
            span(cell(1, 0), cell(1, 1)),
            "Goal",
            TeamAction::Goal,
        );
        team_button(
            ui,
            cx,
            cell(1, 2),
            "Indirect Free Kick",
            TeamAction::IndirectFreeKick,
        );
        team_button(ui, cx, cell(1, 3), "Goal Kick", TeamAction::GoalKick);
        team_button(
            ui,
            cx,
            cell(inner, 2),
            "Penalty Kick",
            TeamAction::PenaltyKick,
        );
        team_button(
            ui,
            cx,
            cell(inner, 3),
            "Corner Kick",
            TeamAction::CornerKick,
        );
        team_stats(ui, cx, side, span(cell(inner, 0), cell(inner, 1)), sign);

        // The player buttons fill the rest of the panel.
        let players_rect =
            Rect::from_min_max(Pos2::new(rect.left(), grid.bottom() + r.of(GAP)), rect.max);
        self.player_buttons(ui, cx, side, players_rect, sign);
    }

    fn player_buttons(&mut self, ui: &mut Ui, cx: &mut Ctx, side: Side, rect: Rect, sign: f32) {
        let r = cx.rem;
        let game = cx.game;
        let team = &game.teams[side];
        let team_params = self.params.game.teams[side].clone();
        let select_state = self.select_state[side];

        // (player number or None for a jersey choice, jersey color, legal)
        let entries: Vec<(Option<u8>, game_controller_core::types::Color, bool)> =
            if select_state == SelectState::PsoColor {
                vec![
                    (None, team_params.goalkeeper_color, true),
                    (None, team_params.field_player_color, true),
                ]
            } else {
                PlayerNumber::all()
                    .map(u8::from)
                    .filter(|number| {
                        let penalty = team.players[(*number - 1) as usize].penalty;
                        match select_state {
                            SelectState::PsoPlayer { .. } => true,
                            SelectState::PlayerIn { .. } => penalty == Penalty::Substitute,
                            _ => penalty != Penalty::Substitute,
                        }
                    })
                    .map(|number| {
                        let is_goalkeeper = match select_state {
                            SelectState::PsoPlayer { goalkeeper } => goalkeeper,
                            SelectState::PlayerIn { player_out } => {
                                team.goalkeeper.map(u8::from) == Some(player_out)
                            }
                            _ => team.goalkeeper.map(u8::from) == Some(number),
                        };
                        let color = if is_goalkeeper {
                            team_params.goalkeeper_color
                        } else {
                            team_params.field_player_color
                        };
                        let legal = select_state != SelectState::Default
                            || cx.legal.player(
                                side,
                                number,
                                self.selected_penalty_call,
                                cx.alternative,
                            );
                        (Some(number), color, legal)
                    })
                    .collect()
            };

        let n = entries.len().max(1);
        let gap = r.of(GAP);
        // A button is at least as high as its two lines of text.
        let min_h = r.of(3.0) + 2.0;
        let h = ((rect.height() - gap * (n as f32 - 1.0)) / n as f32).max(min_h);
        let content_h = h * n as f32 + gap * (n as f32 - 1.0);

        let mut draw = |ui: &mut Ui, origin: Pos2| {
            for (i, (number, color, legal)) in entries.iter().enumerate() {
                let button = Rect::from_min_size(
                    origin + Vec2::new(0.0, i as f32 * (h + gap)),
                    Vec2::new(rect.width(), h),
                );
                let player = number.map(|n| &team.players[(n - 1) as usize]);
                let flash = player.is_some_and(|p| {
                    !matches!(
                        p.penalty,
                        Penalty::NoPenalty
                            | Penalty::SentOff
                            | Penalty::Substitute
                            | Penalty::MotionInSet
                    ) && below_ten_seconds(&p.penalty_timer)
                });
                cx.flashing |= flash;
                let fill = if flash && cx.flash_on {
                    FLASH
                } else {
                    jersey_background(*color)
                };
                let label = match number {
                    Some(n) => format!("{} {}", color_name(*color), n),
                    None => color_name(*color).to_owned(),
                };
                let response = button_frame(
                    ui,
                    button,
                    Id::unique(("player", side, *number, i)),
                    &label,
                    ButtonState {
                        active: false,
                        legal: *legal,
                    },
                    Some(fill),
                );
                let text_color = if *legal { tw::BLACK } else { tw::GRAY_500 };
                paint_player_content(
                    ui,
                    cx,
                    button,
                    sign,
                    *color,
                    player.map(|p| {
                        (
                            p,
                            number.unwrap(),
                            cx.connection_status[side][(number.unwrap() - 1) as usize],
                        )
                    }),
                    text_color,
                );
                if response.clicked() {
                    match number {
                        None => {
                            // The first entry is the goalkeeper jersey.
                            self.select_state[side] = SelectState::PsoPlayer { goalkeeper: i == 0 };
                        }
                        Some(number) => self.click_player(cx, side, *number),
                    }
                }
            }
        };

        if content_h <= rect.height() + 0.5 {
            draw(ui, rect.min);
        } else {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            egui::ScrollArea::vertical()
                .id_salt(("players", side))
                .auto_shrink(false)
                .show(&mut child, |ui| {
                    let (space, _) = ui.allocate_exact_size(
                        Vec2::new(rect.width(), content_h),
                        egui::Sense::hover(),
                    );
                    draw(ui, space.min);
                });
        }
    }

    fn click_player(&mut self, cx: &mut Ctx, side: Side, number: u8) {
        let player = PlayerNumber::new(number);
        match self.select_state[side] {
            SelectState::Default => {
                if let Some(call) = self.selected_penalty_call {
                    let call = PENALTIES[call].1;
                    cx.actions
                        .push(VAction::Penalize(Penalize { side, player, call }));
                    if call != PenaltyCall::MotionInSet {
                        self.selected_penalty_call = None;
                    }
                } else {
                    cx.actions.push(VAction::Unpenalize(Unpenalize {
                        side,
                        player,
                        force: cx.alternative,
                    }));
                }
            }
            SelectState::Goalkeeper => {
                cx.actions
                    .push(VAction::SelectGoalkeeper(SelectGoalkeeper { side, player }));
                self.select_state[side] = SelectState::Default;
            }
            SelectState::PlayerOut => {
                self.select_state[side] = SelectState::PlayerIn { player_out: number };
            }
            SelectState::PlayerIn { player_out } => {
                cx.actions.push(VAction::Substitute(Substitute {
                    side,
                    player_out: PlayerNumber::new(player_out),
                    player_in: player,
                }));
                self.select_state[side] = SelectState::Default;
            }
            SelectState::PsoColor => {}
            SelectState::PsoPlayer { goalkeeper } => {
                cx.actions
                    .push(VAction::SelectPenaltyShotPlayer(SelectPenaltyShotPlayer {
                        side,
                        player,
                        goalkeeper,
                    }));
                self.select_state[side] = SelectState::Default;
            }
        }
    }
}

/// The width of the center panel before it grows: roughly five state buttons wide enough for
/// "Referee Timeout" on one line, plus padding and gaps.
const CENTER_BASIS: f32 = 43.5;

fn clock_panel(ui: &mut Ui, cx: &mut Ctx, rect: Rect) {
    let r = cx.rem;
    let game = cx.game;
    let painter = ui.painter_at(rect.expand(r.of(3.0)));
    let line = r.of(1.5);
    let text_font = font(r.of(1.0), Weight::Regular);

    let phase_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), line));
    style::paint_text_centered(
        &painter,
        phase_rect,
        phase_description(game),
        text_font.clone(),
        tw::BLACK,
    );
    accessible_text(
        ui,
        phase_rect,
        Id::unique("phase"),
        egui::Role::Label,
        phase_description(game),
        "",
    );

    // The main clock (6rem, line height 1).
    let clock_rect = Rect::from_min_size(
        Pos2::new(rect.left(), phase_rect.bottom()),
        Vec2::new(rect.width(), r.of(6.0)),
    );
    let started = matches!(game.primary_timer, Timer::Started { .. });
    let flash = started && below_ten_seconds(&game.primary_timer);
    cx.flashing |= flash;
    let clock_color = if flash && cx.flash_on {
        FLASH
    } else {
        tw::BLACK
    };
    let clock_font = font(r.of(6.0), Weight::Medium);
    let text = format_mmss(&game.primary_timer);
    let text_rect = if started {
        paint_tabular(
            &painter,
            &text,
            clock_font,
            clock_color,
            Align2::CENTER_CENTER,
            clock_rect.center(),
        )
    } else {
        // Invisible, but the layout (and the + button) stays the same.
        let w = painter
            .layout_no_wrap(text.clone(), clock_font, clock_color)
            .size()
            .x;
        Rect::from_center_size(clock_rect.center(), Vec2::new(w, r.of(6.0)))
    };
    if started {
        accessible_text(
            ui,
            text_rect,
            Id::unique("primary timer"),
            egui::Role::Timer,
            "Game Clock",
            &text,
        );
    }

    // The + button for additional time, at the upper right of the clock.
    if game.phase != Phase::PenaltyShootout {
        let size = Vec2::new(r.of(1.0) + r.of(0.65), r.of(2.0) + 2.0);
        let plus = Rect::from_min_size(
            Pos2::new(
                text_rect.right() + r.of(2.0) - size.x,
                clock_rect.top() + r.of(1.75),
            ),
            size,
        );
        if text_button(
            ui,
            plus,
            Id::unique("add_additional_time"),
            "+",
            ButtonState {
                active: false,
                legal: cx.legal.game(GameAction::AddAdditionalTime),
            },
            r.of(1.0),
        ) {
            cx.actions.push(GameAction::AddAdditionalTime.action());
        }
    }

    let state_rect = Rect::from_min_size(
        Pos2::new(rect.left(), clock_rect.bottom()),
        Vec2::new(rect.width(), line),
    );
    style::paint_text_centered(
        &painter,
        state_rect,
        &state_description(game),
        text_font,
        tw::BLACK,
    );
    accessible_text(
        ui,
        state_rect,
        Id::unique("state"),
        egui::Role::Label,
        &state_description(game),
        "",
    );

    if matches!(game.secondary_timer, Timer::Started { .. }) {
        let secondary_rect = Rect::from_min_size(
            Pos2::new(rect.left(), state_rect.bottom()),
            Vec2::new(rect.width(), r.of(2.0)),
        );
        paint_tabular(
            &painter,
            &format_mmss(&game.secondary_timer),
            font(r.of(1.5), Weight::Regular),
            tw::BLACK,
            Align2::CENTER_CENTER,
            secondary_rect.center(),
        );
        accessible_text(
            ui,
            secondary_rect,
            Id::unique("secondary timer"),
            egui::Role::Timer,
            "Secondary Clock",
            &format_mmss(&game.secondary_timer),
        );
    }
}

fn team_stats(ui: &mut Ui, cx: &mut Ctx, side: Side, rect: Rect, sign: f32) {
    let r = cx.rem;
    let game = cx.game;
    let team = &game.teams[side];
    let painter = ui.painter_at(rect);
    let illegal = team.illegal_communication;
    let accent = |default: Color32| if illegal { tw::FUCHSIA_400 } else { default };

    // The score (2rem with 2.5rem line height), aligned towards the center panel.
    let score_line = Rect::from_min_size(rect.min, Vec2::new(rect.width(), r.of(2.5)));
    let (anchor, x) = if sign > 0.0 {
        (Align2::RIGHT_CENTER, score_line.right())
    } else {
        (Align2::LEFT_CENTER, score_line.left())
    };
    paint_tabular(
        &painter,
        &team.score.to_string(),
        font(r.of(2.0), Weight::Bold),
        accent(tw::BLACK),
        anchor,
        Pos2::new(x, score_line.center().y),
    );
    accessible_text(
        ui,
        score_line,
        Id::unique(("score", side)),
        egui::Role::Label,
        &format!("Score: {}", team.score),
        "",
    );

    let mut lines: Vec<(String, String, Color32, Color32)> = vec![];
    if game.phase == Phase::PenaltyShootout {
        let label = if game.kicking_side == Some(side) {
            "Shot:"
        } else {
            "Shots:"
        };
        lines.push((
            label.into(),
            team.penalty_shot.to_string(),
            tw::GRAY_600,
            tw::BLACK,
        ));
    } else {
        lines.push((
            "Messages:".into(),
            team.message_budget.to_string(),
            accent(tw::GRAY_600),
            accent(tw::BLACK),
        ));
    }
    lines.push((
        "Penalties:".into(),
        team.penalty_counter.to_string(),
        tw::GRAY_600,
        tw::BLACK,
    ));

    let line_h = r.of(1.5);
    let text_font = font(r.of(1.0), Weight::Regular);
    let mut y = score_line.bottom();
    for (label, value, label_color, value_color) in lines {
        accessible_text(
            ui,
            Rect::from_min_size(
                Pos2::new(rect.left(), y),
                Vec2::new(rect.width(), 2.0 * line_h),
            ),
            Id::unique(("stat", side, label.clone())),
            egui::Role::Label,
            &format!("{label} {value}"),
            "",
        );
        paint_galley(
            &painter,
            painter.layout_no_wrap(label, text_font.clone(), label_color),
            Align2::LEFT_CENTER,
            Pos2::new(rect.left(), y + line_h / 2.0),
        );
        y += line_h;
        paint_tabular(
            &painter,
            &value,
            text_font.clone(),
            value_color,
            Align2::RIGHT_CENTER,
            Pos2::new(rect.right(), y + line_h / 2.0),
        );
        y += line_h;
    }
}

fn paint_player_content(
    ui: &mut Ui,
    cx: &Ctx,
    rect: Rect,
    sign: f32,
    color: game_controller_core::types::Color,
    player: Option<(&game_controller_core::types::Player, u8, ConnectionStatus)>,
    text_color: Color32,
) {
    let r = cx.rem;
    let painter = ui.painter_at(rect);
    let inner = rect.shrink2(Vec2::new(r.of(1.0), 0.0));
    let gap = r.of(1.0);
    let icon = r.of(1.5);
    let number_font = font(r.of(2.0), Weight::Regular);
    let text_font = font(r.of(1.0), Weight::Regular);

    // The icon and the number are at the outer edge, the text column takes the rest.
    let text_rect = if let Some((_, number, status)) = player {
        let number_galley =
            painter.layout_no_wrap(number.to_string(), number_font.clone(), text_color);
        let number_w = number_galley.size().x;
        let digit_w = painter
            .layout_no_wrap("0".into(), number_font.clone(), text_color)
            .size()
            .x;
        let number_w = number_w.max(digit_w * number.to_string().len() as f32);
        let (number_x, icon_x, text_rect) = if sign > 0.0 {
            let number_x = inner.right() - number_w;
            let icon_x = number_x - gap - icon;
            (
                number_x,
                icon_x,
                Rect::from_min_max(inner.min, Pos2::new(icon_x - gap, inner.bottom())),
            )
        } else {
            let number_x = inner.left();
            let icon_x = number_x + number_w + gap;
            (
                number_x,
                icon_x,
                Rect::from_min_max(Pos2::new(icon_x + icon + gap, inner.top()), inner.max),
            )
        };
        paint_tabular(
            &painter,
            &number.to_string(),
            number_font,
            text_color,
            Align2::LEFT_CENTER,
            Pos2::new(number_x, inner.center().y),
        );
        let icon_rect = Rect::from_min_size(
            Pos2::new(icon_x, inner.center().y - icon / 2.0),
            Vec2::splat(icon),
        );
        match status {
            ConnectionStatus::Good => icons::check(&painter, icon_rect, tw::GREEN_600),
            ConnectionStatus::Bad => {
                icons::exclamation_triangle(&painter, icon_rect, tw::YELLOW_400)
            }
            ConnectionStatus::Offline => icons::x_mark(&painter, icon_rect, tw::RED_600),
        }
        text_rect
    } else {
        inner
    };

    // The text column: jersey color and penalty (timer or description).
    let line_h = r.of(1.5);
    let lines = if player.is_some() { 2.0 } else { 1.0 };
    let top = text_rect.center().y - lines * line_h / 2.0;
    let first = Rect::from_min_size(
        Pos2::new(text_rect.left(), top),
        Vec2::new(text_rect.width(), line_h),
    );
    style::paint_text_centered(
        &painter,
        first,
        color_name(color),
        text_font.clone(),
        text_color,
    );
    if let Some((player, _, _)) = player {
        let second = first.translate(Vec2::new(0.0, line_h));
        if matches!(player.penalty_timer, Timer::Started { .. }) {
            paint_tabular(
                &painter,
                &format_mmss(&player.penalty_timer),
                text_font,
                text_color,
                Align2::CENTER_CENTER,
                second.center(),
            );
        } else if player.penalty != Penalty::NoPenalty {
            style::paint_text_centered(
                &painter,
                second,
                penalty_description(player.penalty),
                text_font,
                text_color,
            );
        }
    }
}

fn undo_panel(ui: &mut Ui, cx: &mut Ctx, rect: Rect, undo_actions: &[VAction]) {
    let r = cx.rem;
    for index in 0..NUM_OF_UNDO_ACTIONS {
        // The buttons are ordered from right to left (the rightmost undoes the last action).
        let cell = nth_column(
            rect,
            NUM_OF_UNDO_ACTIONS,
            r.of(GAP),
            NUM_OF_UNDO_ACTIONS - 1 - index,
        );
        let label = undo_actions
            .get(index)
            .map_or_else(|| "Undo".to_owned(), action_name);
        if text_button(
            ui,
            cell,
            Id::unique(("undo", index)),
            &label,
            ButtonState {
                active: false,
                legal: cx.legal.undo(index + 1),
            },
            r.of(1.0),
        ) {
            cx.actions.push(VAction::Undo(Undo {
                states: index as u32 + 1,
            }));
        }
    }
}

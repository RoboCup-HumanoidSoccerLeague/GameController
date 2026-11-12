//! This module implements the main window from which a game is operated.
//!
//! The window consists of two team panels with the center panel (clock, state buttons, penalty
//! calls) in between and a row of undo buttons below. The whole window is zoomed with its size
//! ([zoom_factor]), so all lengths are constants. The team panels and the center panel are made
//! of rows of the same heights, so that they line up.

use std::{sync::Arc, time::Duration};

use eframe::egui::{
    self, text::LayoutJob, vec2, Align, Atom, AtomExt, Atoms, Button, CentralPanel, Label, Layout,
    Panel, Rect, Response, ScrollArea, TextFormat, TextStyle, Ui, Vec2, WidgetInfo, WidgetText,
};
use enum_map::EnumMap;

use game_controller_core::{
    action::VAction,
    actions::{Penalize, SelectGoalkeeper, SelectPenaltyShotPlayer, Substitute, Undo, Unpenalize},
    timer::{SignedDuration, Timer},
    types::{
        Color, Game, Params, Penalty, PenaltyCall, Phase, Player, PlayerNumber, SetPlay, Side,
        SideMapping, State,
    },
};
use game_controller_runtime::{
    connection_status::{ConnectionStatus, ConnectionStatusMap},
    UiState,
};

use crate::{
    actions::{GameAction, Legal, TeamAction, NUM_OF_UNDO_ACTIONS, PENALTIES},
    style::{
        append_tabular, color_name, font, icons, jersey_background, jersey_text, line, panel_frame,
        set_disabled, tabular, tw, Weight, FLASH,
    },
};

/// The state of the player selection of a team panel (substitution, goalkeeper selection, or
/// penalty shooter selection).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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

/// The size of body text. All lengths of the layout are multiples of it.
const REM: f32 = 16.0;
/// The height of a line of body text.
const LINE_HEIGHT: f32 = 1.5 * REM;
/// The space between widgets, and the margin of the panels.
const GAP: f32 = 0.5 * REM;
/// The height of a row of buttons.
const ROW_HEIGHT: f32 = 4.0 * REM;
const CLOCK_HEIGHT: f32 = 11.0 * REM;
/// The state buttons are as high as two rows, in however many rows they are laid out.
const STATE_HEIGHT: f32 = 2.0 * ROW_HEIGHT + GAP;
/// The header and the four rows of buttons of a team panel are as high as the clock and the state
/// buttons, so that the player buttons start where the penalty buttons start.
const TEAM_HEADER_HEIGHT: f32 = CLOCK_HEIGHT + STATE_HEIGHT - 4.0 * ROW_HEIGHT - 3.0 * GAP;
const UNDO_HEIGHT: f32 = 2.5 * REM;
/// A player button is at least as high as its two lines of text.
const MIN_PLAYER_HEIGHT: f32 = 2.0 * LINE_HEIGHT + 2.0;
/// The width of a team panel before it grows, and the width at which it stops growing.
const TEAM_WIDTH: f32 = 18.125 * REM;
const MAX_TEAM_WIDTH: f32 = 24.0 * REM;
/// The width of the center panel before it grows: five state buttons that are wide enough for
/// "Referee Timeout" on one line.
const CENTER_WIDTH: f32 = 43.5 * REM;

/// This function returns the zoom factor of the main window for the size of the window in native
/// points (i.e. at a zoom factor of 1). The body text is 1.1% of the width or 1.9% of the height
/// (whichever is smaller), but at least 16 and at most 40 native points.
pub fn zoom_factor(native_size: Vec2) -> f32 {
    let rem = (native_size.x * 0.011)
        .min(native_size.y * 0.019)
        .clamp(REM, 2.5 * REM);
    // The size is derived from a zoomed one, so it is rounded to keep it from fluctuating.
    (rem / REM * 1000.0).round() / 1000.0
}

/// This function returns the width of a team panel for the width that is available to the two
/// team panels and the center panel. The team panels grow half as fast as the center panel.
fn team_width(available: f32) -> f32 {
    let spare = (available - 2.0 * TEAM_WIDTH - CENTER_WIDTH).max(0.0);
    (TEAM_WIDTH + spare / 4.0).min(MAX_TEAM_WIDTH)
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

/// This function adds a button of a given size that is disabled if its action is not legal.
fn action_button(
    ui: &mut Ui,
    size: impl Into<Vec2>,
    label: &str,
    legal: bool,
    selected: bool,
) -> Response {
    let size = size.into();
    ui.add_sized(size, |ui: &mut Ui| {
        if !legal {
            set_disabled(ui);
        }
        let text = centered_text(ui, label, size.x - 2.0 * ui.spacing().button_padding.x);
        ui.add_enabled(legal, Button::new(text).selected(selected))
    })
}

/// This function lays out the text of a button with centered lines that wrap at a given width.
/// The layout of a button places a text at its left edge, whatever its alignment is, so the rows
/// of the centered text are moved to start there.
fn centered_text(ui: &Ui, text: &str, wrap_width: f32) -> WidgetText {
    let mut job = Arc::unwrap_or_clone(WidgetText::from(text).into_layout_job(
        ui.style(),
        TextStyle::Button.into(),
        ui.text_valign(),
    ));
    job.halign = Align::Center;
    job.wrap.max_width = wrap_width;
    let mut galley = Arc::unwrap_or_clone(ui.fonts_mut(|fonts| fonts.layout_job(job)));
    let offset = vec2(-galley.rect.left(), 0.0);
    for row in &mut galley.rows {
        row.pos += offset;
    }
    galley.rect = galley.rect.translate(offset);
    galley.mesh_bounds = galley.mesh_bounds.translate(offset);
    Arc::new(galley).into()
}

/// Everything that is needed to draw one frame.
struct Ctx<'a> {
    game: &'a Game,
    legal: Legal<'a>,
    connection_status: &'a ConnectionStatusMap,
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
        let Some(legal) = Legal::new(&state.legal_actions) else {
            CentralPanel::default().show(ui, |_| {});
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

        let spacing = ui.spacing_mut();
        spacing.item_spacing = Vec2::splat(GAP);
        spacing.button_padding = vec2(0.25 * REM, 0.125 * REM);

        let frame = panel_frame(GAP as i8);
        Panel::bottom("undo")
            .frame(frame)
            .show_separator_line(false)
            .exact_size(UNDO_HEIGHT + 2.0 * GAP)
            .show(ui, |ui| undo_panel(ui, &mut cx, &state.undo_actions));

        // Every panel has a margin, so there are two margins between each two of them.
        let width = team_width(ui.available_width() - 6.0 * GAP) + 2.0 * GAP;
        let (left, right) = match state.game.sides {
            SideMapping::HomeDefendsLeftGoal => (Side::Home, Side::Away),
            SideMapping::HomeDefendsRightGoal => (Side::Away, Side::Home),
        };
        Panel::left("left team")
            .frame(frame)
            .show_separator_line(false)
            .resizable(false)
            .exact_size(width)
            .show(ui, |ui| self.team_panel(ui, &mut cx, left, true));
        Panel::right("right team")
            .frame(frame)
            .show_separator_line(false)
            .resizable(false)
            .exact_size(width)
            .show(ui, |ui| self.team_panel(ui, &mut cx, right, false));
        CentralPanel::default().frame(frame).show(ui, |ui| {
            clock_panel(ui, &mut cx);
            self.state_panel(ui, &mut cx);
            self.penalty_panel(ui, &mut cx);
        });

        if cx.flashing {
            // Repaint at the next flash phase change.
            let to_next = 0.5 - time.rem_euclid(0.5);
            ui.ctx()
                .request_repaint_after(Duration::from_secs_f64(to_next.max(0.01)));
        }
        cx.actions
    }

    fn state_panel(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let game = cx.game;
        let in_half_time_break = (matches!(game.phase, Phase::FirstHalf | Phase::FirstExtraHalf)
            && game.state == State::Finished)
            || (matches!(game.phase, Phase::SecondHalf | Phase::SecondExtraHalf)
                && game.state == State::Initial);
        let has_extra_time = self.params.competition.extra_half_duration.is_some();
        let pso = game.phase == Phase::PenaltyShootout;
        let in_play = matches!(game.state, State::Ready | State::Set | State::Playing);

        // (column span, label, game action, selected)
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

        // The buttons are laid out in five columns: a button that doesn't fit into the rest of a
        // row starts the next row. All rows have the same height.
        const COLUMNS: usize = 5;
        let mut rows: Vec<Vec<_>> = vec![vec![]];
        let mut used = 0;
        for item in items {
            if used + item.0 > COLUMNS {
                rows.push(vec![]);
                used = 0;
            }
            used += item.0;
            rows.last_mut().unwrap().push(item);
        }
        let column_width = (ui.available_width() - GAP * (COLUMNS as f32 - 1.0)) / COLUMNS as f32;
        let row_height = (STATE_HEIGHT - GAP * (rows.len() as f32 - 1.0)) / rows.len() as f32;
        for row in rows {
            ui.horizontal(|ui| {
                for (span, label, action, selected) in row {
                    let width = column_width * span as f32 + GAP * (span as f32 - 1.0);
                    let legal = cx.legal.game(action);
                    if action_button(ui, [width, row_height], label, legal, selected).clicked() {
                        cx.actions.push(action.action());
                    }
                }
            });
        }
    }

    fn penalty_panel(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        const COLUMNS: usize = 2;
        let rows = PENALTIES.len().div_ceil(COLUMNS) as f32;
        let size = vec2(
            (ui.available_width() - GAP * (COLUMNS as f32 - 1.0)) / COLUMNS as f32,
            (ui.available_height() - GAP * (rows - 1.0)) / rows,
        );
        for (row, calls) in PENALTIES.chunks(COLUMNS).enumerate() {
            ui.horizontal(|ui| {
                for (column, (label, _)) in calls.iter().enumerate() {
                    let index = row * COLUMNS + column;
                    let selected = self.selected_penalty_call == Some(index);
                    let legal = cx.legal.penalty_call(index);
                    if action_button(ui, size, label, legal, selected).clicked() {
                        self.selected_penalty_call = if selected { None } else { Some(index) };
                    }
                }
            });
        }
    }

    /// This function draws the panel of a team. `left` is whether it is the left one, as the two
    /// panels mirror each other.
    fn team_panel(&mut self, ui: &mut Ui, cx: &mut Ctx, side: Side, left: bool) {
        let game = cx.game;
        let team_params = &self.params.game.teams[side];
        let pso = game.phase == Phase::PenaltyShootout;

        // The header: the team name in the jersey color, marked if the team has the kick-off or
        // set play.
        let name_font = font(1.5 * REM, Weight::SemiBold);
        let mut name = LayoutJob::default();
        if game.kicking_side == Some(side) {
            name.append("● ", 0.0, TextFormat::simple(name_font.clone(), tw::BLACK));
        }
        name.append(
            &self.team_names[side],
            0.0,
            TextFormat::simple(name_font, jersey_text(team_params.field_player_color)),
        );
        ui.add_sized(
            [ui.available_width(), TEAM_HEADER_HEIGHT],
            Label::new(name).truncate(),
        );

        // The team actions: three columns of four rows, in which the goal button and the
        // statistics are two rows high. The statistics are next to the center panel.
        let select_state = self.select_state[side];
        let (outer, inner) = if left { (0, 2) } else { (2, 0) };
        let double_row_height = 2.0 * ROW_HEIGHT + GAP;
        ui.columns(3, |columns| {
            let ui = &mut columns[outer];
            let label = if pso || cx.alternative || select_state == SelectState::Goalkeeper {
                "Select"
            } else {
                "Substitute"
            };
            let selecting = select_state != SelectState::Default;
            if action_button(
                ui,
                [ui.available_width(), ROW_HEIGHT],
                label,
                true,
                selecting,
            )
            .clicked()
            {
                self.select_state[side] = if selecting {
                    SelectState::Default
                } else if pso {
                    if team_params.goalkeeper_color == team_params.field_player_color {
                        // If the goalkeeper doesn't have a special jersey color, its selection
                        // can be skipped.
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
            team_button(ui, cx, side, ROW_HEIGHT, "Timeout", TeamAction::Timeout);
            team_button(
                ui,
                cx,
                side,
                ROW_HEIGHT,
                "Direct Free Kick",
                TeamAction::DirectFreeKick,
            );
            team_button(ui, cx, side, ROW_HEIGHT, "Throw-in", TeamAction::ThrowIn);

            let ui = &mut columns[1];
            team_button(ui, cx, side, double_row_height, "Goal", TeamAction::Goal);
            team_button(
                ui,
                cx,
                side,
                ROW_HEIGHT,
                "Indirect Free Kick",
                TeamAction::IndirectFreeKick,
            );
            team_button(ui, cx, side, ROW_HEIGHT, "Goal Kick", TeamAction::GoalKick);

            let ui = &mut columns[inner];
            team_stats(ui, cx, side, double_row_height, left);
            team_button(
                ui,
                cx,
                side,
                ROW_HEIGHT,
                "Penalty Kick",
                TeamAction::PenaltyKick,
            );
            team_button(
                ui,
                cx,
                side,
                ROW_HEIGHT,
                "Corner Kick",
                TeamAction::CornerKick,
            );
        });

        self.player_buttons(ui, cx, side, left);
    }

    /// This function draws the player buttons, which share the rest of the panel (and can be
    /// scrolled if there are too many for it).
    fn player_buttons(&mut self, ui: &mut Ui, cx: &mut Ctx, side: Side, left: bool) {
        let team = &cx.game.teams[side];
        let team_params = self.params.game.teams[side].clone();
        let select_state = self.select_state[side];

        // (player number or None for a jersey choice, jersey color, legal)
        let entries: Vec<(Option<u8>, Color, bool)> = if select_state == SelectState::PsoColor {
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

        let count = entries.len().max(1) as f32;
        // A bit of the height is left over, so that rounding can't make the buttons scroll.
        let available = ui.available_height() - 0.5;
        let size = vec2(
            ui.available_width(),
            ((available - GAP * (count - 1.0)) / count).max(MIN_PLAYER_HEIGHT),
        );
        ScrollArea::vertical()
            .id_salt(("players", side))
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().button_padding.x = REM;
                for (index, (number, color, legal)) in entries.into_iter().enumerate() {
                    let player = number.map(|number| {
                        (
                            number,
                            &team.players[(number - 1) as usize],
                            cx.connection_status[side][(number - 1) as usize],
                        )
                    });
                    let flash = player.is_some_and(|(_, player, _)| {
                        !matches!(
                            player.penalty,
                            Penalty::NoPenalty
                                | Penalty::SentOff
                                | Penalty::Substitute
                                | Penalty::MotionInSet
                        ) && below_ten_seconds(&player.penalty_timer)
                    });
                    cx.flashing |= flash;
                    let button =
                        player_button(ui, color, player, legal, flash && cx.flash_on, left);
                    let response = ui.add_sized(size, |ui: &mut Ui| ui.add_enabled(legal, button));
                    // The button is named the same on both sides (its parts are mirrored).
                    response.widget_info(|| {
                        let name = match number {
                            Some(number) => format!("{} {number}", color_name(color)),
                            None => color_name(color).to_owned(),
                        };
                        WidgetInfo::labeled(egui::Role::Button, legal, name)
                    });
                    if response.clicked() {
                        match number {
                            // The first entry is the goalkeeper jersey.
                            None => {
                                self.select_state[side] = SelectState::PsoPlayer {
                                    goalkeeper: index == 0,
                                };
                            }
                            Some(number) => self.click_player(cx, side, number),
                        }
                    }
                }
            });
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

fn team_button(
    ui: &mut Ui,
    cx: &mut Ctx,
    side: Side,
    height: f32,
    label: &str,
    action: TeamAction,
) {
    // The button of the set play that is in progress is selected.
    let selected = action
        .set_play()
        .is_some_and(|set_play| cx.game.set_play == set_play && cx.game.kicking_side == Some(side));
    let legal = cx.legal.team(side, action);
    if action_button(ui, [ui.available_width(), height], label, legal, selected).clicked() {
        cx.actions.push(action.action(side));
    }
}

/// This function draws the score and the counters of a team. The score is aligned towards the
/// center panel.
fn team_stats(ui: &mut Ui, cx: &Ctx, side: Side, height: f32, left: bool) {
    let game = cx.game;
    let team = &game.teams[side];
    // Illegal communication is highlighted.
    let accent = |color| {
        if team.illegal_communication {
            tw::FUCHSIA_400
        } else {
            color
        }
    };
    let mut counters = vec![];
    if game.phase == Phase::PenaltyShootout {
        let label = if game.kicking_side == Some(side) {
            "Shot:"
        } else {
            "Shots:"
        };
        counters.push((
            label,
            team.penalty_shot.to_string(),
            tw::GRAY_600,
            tw::BLACK,
        ));
    } else {
        counters.push((
            "Messages:",
            team.message_budget.to_string(),
            accent(tw::GRAY_600),
            accent(tw::BLACK),
        ));
    }
    counters.push((
        "Penalties:",
        team.penalty_counter.to_string(),
        tw::GRAY_600,
        tw::BLACK,
    ));

    let size = vec2(ui.available_width(), height);
    ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(size);
        ui.spacing_mut().item_spacing.y = 0.0;
        let towards_center = if left { Align::Max } else { Align::Min };
        ui.with_layout(Layout::top_down(towards_center), |ui| {
            let score = tabular(
                ui,
                &team.score.to_string(),
                font(2.0 * REM, Weight::Bold),
                accent(tw::BLACK),
                2.5 * REM,
            );
            ui.label(score).on_hover_text("Score");
        });
        for (label, value, label_color, value_color) in counters {
            ui.label(line(label, LINE_HEIGHT).color(label_color));
            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                let value = tabular(
                    ui,
                    &value,
                    font(REM, Weight::Regular),
                    value_color,
                    LINE_HEIGHT,
                );
                ui.label(value);
            });
        }
    });
}

/// This function creates the button of a player (or of a jersey, if `player` is [None]): the
/// jersey color and the penalty, the connection status and the number. The number is at the outer
/// edge of the window.
fn player_button<'a>(
    ui: &Ui,
    jersey: Color,
    player: Option<(u8, &Player, ConnectionStatus)>,
    legal: bool,
    flash: bool,
    left: bool,
) -> Button<'a> {
    // A button that can't be clicked keeps its jersey color, but its text is gray.
    let color = if legal { tw::BLACK } else { tw::GRAY_500 };
    let format = TextFormat {
        font_id: font(REM, Weight::Regular),
        color,
        line_height: Some(LINE_HEIGHT),
        ..Default::default()
    };
    // The penalty (its remaining time, or else its name) is shown below the jersey color. The
    // two lines are centered by indenting the shorter one, because the layout of a button
    // doesn't support centered text.
    let penalty = player.and_then(|(_, player, _)| {
        if matches!(player.penalty_timer, Timer::Started { .. }) {
            Some((format_mmss(&player.penalty_timer), true))
        } else if player.penalty != Penalty::NoPenalty {
            Some((penalty_description(player.penalty).to_owned(), false))
        } else {
            None
        }
    });
    let width = |job: LayoutJob| ui.fonts_mut(|fonts| fonts.layout_job(job)).size().x;
    let line_job = |text: &str, tabular: bool| {
        let mut job = LayoutJob::default();
        if tabular {
            append_tabular(ui, &mut job, text, &format, 0.0);
        } else {
            job.append(text, 0.0, format.clone());
        }
        job
    };
    let jersey_width = width(line_job(color_name(jersey), false));
    let penalty_width = penalty.as_ref().map_or(jersey_width, |(text, tabular)| {
        width(line_job(text, *tabular))
    });
    let indent = (penalty_width - jersey_width) / 2.0;
    let mut text = LayoutJob::default();
    text.append(color_name(jersey), indent.max(0.0), format.clone());
    if let Some((penalty, tabular)) = penalty {
        text.append("\n", 0.0, format.clone());
        if tabular {
            append_tabular(ui, &mut text, &penalty, &format, (-indent).max(0.0));
        } else {
            text.append(&penalty, (-indent).max(0.0), format);
        }
    } else if player.is_some() {
        // The line of the penalty is kept, so that the jersey color doesn't move.
        text.append("\n", 0.0, format);
    }
    let mut atoms = Atoms::new(Atom::from(text).atom_grow(true));
    if let Some((number, _, status)) = player {
        let icon = Atom::paint(Vec2::splat(1.5 * REM), move |ui: &Ui, args| {
            let painter = ui.painter();
            match status {
                ConnectionStatus::Good => icons::check(painter, args.rect, tw::GREEN_600),
                ConnectionStatus::Bad => {
                    icons::exclamation_triangle(painter, args.rect, tw::YELLOW_400);
                }
                ConnectionStatus::Offline => icons::x_mark(painter, args.rect, tw::RED_600),
            }
        });
        let number = tabular(
            ui,
            &number.to_string(),
            font(2.0 * REM, Weight::Regular),
            color,
            2.0 * REM,
        );
        if left {
            atoms.push_right(icon);
            atoms.push_right(number);
        } else {
            atoms.push_left(icon);
            atoms.push_left(number);
        }
    }
    let fill = if flash {
        FLASH
    } else {
        jersey_background(jersey)
    };
    Button::new(atoms).fill(fill).gap(REM)
}

fn clock_panel(ui: &mut Ui, cx: &mut Ctx) {
    let game = cx.game;
    let started = matches!(game.primary_timer, Timer::Started { .. });
    let flash = started && below_ten_seconds(&game.primary_timer);
    cx.flashing |= flash;
    let color = if flash && cx.flash_on {
        FLASH
    } else {
        tw::BLACK
    };

    let size = vec2(ui.available_width(), CLOCK_HEIGHT);
    let clock = ui
        .allocate_ui_with_layout(size, Layout::top_down(Align::Center), |ui| {
            ui.set_min_size(size);
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(line(phase_description(game), LINE_HEIGHT));
            let clock = tabular(
                ui,
                &format_mmss(&game.primary_timer),
                font(6.0 * REM, Weight::Medium),
                color,
                6.0 * REM,
            );
            // A clock that isn't running is hidden, but still takes its space.
            let clock = ui.add_visible(started, Label::new(clock));
            ui.label(line(state_description(game), LINE_HEIGHT));
            if matches!(game.secondary_timer, Timer::Started { .. }) {
                let timer = tabular(
                    ui,
                    &format_mmss(&game.secondary_timer),
                    font(1.5 * REM, Weight::Regular),
                    tw::BLACK,
                    2.0 * REM,
                );
                ui.label(timer);
            }
            clock
        })
        .inner;

    // The button for additional time is placed at the upper right of the clock.
    if game.phase != Phase::PenaltyShootout {
        let rect = Rect::from_min_size(
            clock.rect.right_top() + vec2(0.375 * REM, 1.75 * REM),
            vec2(1.625 * REM, 2.125 * REM),
        );
        let legal = cx.legal.game(GameAction::AddAdditionalTime);
        let button = Button::new("+");
        if ui
            .place(rect, |ui: &mut Ui| {
                if !legal {
                    set_disabled(ui);
                }
                ui.add_enabled(legal, button)
            })
            .on_hover_text("Add additional time")
            .clicked()
        {
            cx.actions.push(GameAction::AddAdditionalTime.action());
        }
    }
}

fn undo_panel(ui: &mut Ui, cx: &mut Ctx, undo_actions: &[VAction]) {
    ui.columns(NUM_OF_UNDO_ACTIONS, |columns| {
        // The buttons are ordered from right to left (the rightmost undoes the last action).
        for (index, ui) in columns.iter_mut().rev().enumerate() {
            let label = undo_actions
                .get(index)
                .map_or_else(|| "Undo".to_owned(), action_name);
            let legal = cx.legal.undo(index + 1);
            if action_button(
                ui,
                [ui.available_width(), UNDO_HEIGHT],
                &label,
                legal,
                false,
            )
            .clicked()
            {
                cx.actions.push(VAction::Undo(Undo {
                    states: index as u32 + 1,
                }));
            }
        }
    });
}

//! This module implements the launcher, in which the competition, the teams and other settings are
//! chosen before the game starts.

use eframe::egui::{
    self, Align, Align2, Id, Layout, Pos2, Rect, RichText, Stroke, Ui, UiBuilder, Vec2,
};

use game_controller_core::types::{Color, Side, SideMapping, TeamParams};
use game_controller_runtime::launch::{LaunchData, LaunchSettings, Team};

use crate::style::{self, font, tw, ButtonState, Weight};

/// The number of the team that is part of every competition and is selected by default.
const DEFAULT_TEAM_NUMBER: u8 = 0;

/// The launcher.
pub struct Launcher {
    data: LaunchData,
    settings: LaunchSettings,
}

impl Launcher {
    /// This function creates a launcher that starts with the default settings of the launch data.
    pub fn new(data: LaunchData) -> Self {
        let settings = data.default_settings.clone();
        Self { data, settings }
    }

    /// The launch data that the launcher was created with.
    pub fn data(&self) -> &LaunchData {
        &self.data
    }

    fn team(&self, number: u8) -> &Team {
        self.data
            .teams
            .iter()
            .find(|team| team.number == number)
            .expect("team must exist")
    }

    /// This function returns whether the settings describe a valid game: different teams whose
    /// jerseys can be told apart.
    pub fn settings_are_legal(&self) -> bool {
        let teams = &self.settings.game.teams;
        let (home, away) = (&teams[Side::Home], &teams[Side::Away]);
        home.number != away.number
            && home.field_player_color != away.field_player_color
            && home.field_player_color != away.goalkeeper_color
            && away.field_player_color != home.goalkeeper_color
    }

    fn set_competition(&mut self, id: String) {
        if id == self.settings.competition.id {
            return;
        }
        self.settings.competition.id = id;
        // The default team is part of every competition.
        let default = self.team(DEFAULT_TEAM_NUMBER).clone();
        self.settings.game.teams[Side::Home] = TeamParams {
            number: default.number,
            field_player_color: default.field_player_colors[0],
            goalkeeper_color: default.goalkeeper_colors[0],
        };
        self.settings.game.teams[Side::Away] = TeamParams {
            number: default.number,
            field_player_color: default.field_player_colors[1],
            goalkeeper_color: default.goalkeeper_colors[1],
        };
    }

    /// This function draws the launcher and returns the settings when the user clicks Start.
    pub fn ui(&mut self, ui: &mut Ui) -> Option<LaunchSettings> {
        let screen = ui.max_rect();
        ui.painter().rect_filled(screen, 0.0, tw::WHITE);
        let rem = 16.0;
        let r = |n: f32| n * rem;
        // The content is at most 40rem x 35rem, centered, with 1rem padding.
        let content = Rect::from_center_size(
            screen.center(),
            Vec2::new(screen.width().min(r(40.0)), screen.height().min(r(35.0))),
        )
        .shrink(r(1.0));
        let gap = r(0.75);
        let field_h = r(1.25) + r(0.25) + r(1.75);
        let start_h = r(1.25 * 1.4) + r(1.0) + 2.0;
        let lower_h = field_h + r(0.5) + r(5.5);

        let competition_rect =
            Rect::from_min_size(content.min, Vec2::new(content.width(), field_h));
        let start_rect = Rect::from_min_max(
            Pos2::new(content.left(), content.bottom() - start_h),
            content.max,
        );
        let lower_rect = Rect::from_min_size(
            Pos2::new(content.left(), start_rect.top() - gap - lower_h),
            Vec2::new(content.width(), lower_h),
        );
        let game_rect = Rect::from_min_max(
            Pos2::new(content.left(), competition_rect.bottom() + gap),
            Pos2::new(content.right(), lower_rect.top() - gap),
        );

        // Competition
        let competitions = self.data.competitions.clone();
        let mut competition_id = self.settings.competition.id.clone();
        field(ui, competition_rect, "Competition", |ui, label| {
            let selected = competitions
                .iter()
                .find(|c| c.id == competition_id)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            select(ui, label, "competition", selected, true, |ui| {
                for competition in &competitions {
                    ui.selectable_value(
                        &mut competition_id,
                        competition.id.clone(),
                        &competition.name,
                    );
                }
            });
        });
        self.set_competition(competition_id);

        self.game_settings(ui, game_rect);

        // Network interface / window, casting / testing.
        let cols = three_columns(lower_rect);
        let top = |rect: Rect| Rect::from_min_size(rect.min, Vec2::new(rect.width(), field_h));
        let bottom = |rect: Rect| {
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + field_h + r(0.5)),
                rect.max,
            )
        };
        let interfaces = self.data.network_interfaces.clone();
        let network = &mut self.settings.network;
        field(ui, top(cols[0]), "Network Interface", |ui, label| {
            let selected = network.interface.clone();
            select(ui, label, "interface", selected, true, |ui| {
                for interface in &interfaces {
                    ui.selectable_value(
                        &mut network.interface,
                        interface.id.clone(),
                        &interface.id,
                    );
                }
            });
        });
        let fullscreen = &mut self.settings.window.fullscreen;
        field(ui, top(cols[2]), "Window", |ui, label| {
            ui.add_space(4.0);
            ui.checkbox(fullscreen, "Fullscreen").labelled_by(label);
        });
        details(ui, bottom(cols[0]), "Casting (Advanced)", |ui| {
            ui.checkbox(&mut network.broadcast, "Broadcast");
            ui.checkbox(&mut network.multicast, "Multicast");
        });
        let test = &mut self.settings.game.test;
        details(ui, bottom(cols[2]), "Testing", |ui| {
            ui.checkbox(&mut test.no_delay, "No Delay");
            ui.checkbox(&mut test.penalty_shootout, "Penalty Shoot-out");
        });

        // Start
        let legal = self.settings_are_legal();
        let start_size = Vec2::new(r(1.25) * 2.8 + r(4.0), start_h);
        let start_button = Rect::from_center_size(start_rect.center(), start_size);
        let response = style::button_frame(
            ui,
            start_button,
            Id::unique("start"),
            "Start",
            ButtonState {
                active: false,
                legal,
            },
            None,
        );
        style::paint_text_centered(
            ui.painter(),
            start_button,
            "Start",
            font(r(1.25), Weight::SemiBold),
            if legal { tw::BLACK } else { tw::GRAY_500 },
        );
        response.clicked().then(|| self.settings.clone())
    }

    fn game_settings(&mut self, ui: &mut Ui, rect: Rect) {
        let mirrored = self.settings.game.side_mapping == SideMapping::HomeDefendsRightGoal;
        let cols = three_columns(rect);
        let (home_rect, away_rect) = if mirrored {
            (cols[2], cols[0])
        } else {
            (cols[0], cols[2])
        };
        let competition = self
            .data
            .competitions
            .iter()
            .find(|c| c.id == self.settings.competition.id)
            .expect("competition must exist");
        let mut teams: Vec<Team> = self
            .data
            .teams
            .iter()
            .filter(|team| competition.teams.contains(&team.number))
            .cloned()
            .collect();
        // The teams are sorted by name, ignoring case.
        teams.sort_by_cached_key(|team| team.name.to_lowercase());

        for (side, rect) in [(Side::Home, home_rect), (Side::Away, away_rect)] {
            self.team_settings(ui, rect, side, &teams);
        }

        // The middle column holds the mirror checkbox.
        let middle = cols[1];
        let arrows_size = 24.0;
        let checkbox_h = 24.0;
        let total = arrows_size + 4.0 + checkbox_h;
        let top = middle.center().y - total / 2.0;
        style::icons::arrows_right_left(
            ui.painter(),
            Rect::from_center_size(
                Pos2::new(middle.center().x, top + arrows_size / 2.0),
                Vec2::splat(arrows_size),
            ),
            tw::BLACK,
        );
        let mut mirror = mirrored;
        let checkbox_rect = Rect::from_center_size(
            Pos2::new(middle.center().x, top + total - checkbox_h / 2.0),
            Vec2::new(middle.width() + 16.0, checkbox_h),
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(checkbox_rect)
                .layout(Layout::left_to_right(Align::Center).with_main_justify(false)),
        );
        child.vertical_centered(|ui| {
            ui.checkbox(&mut mirror, "Mirror")
                .on_hover_text("Swap the sides: the home team starts on the right");
        });
        if mirror != mirrored {
            self.settings.game.side_mapping = if mirror {
                SideMapping::HomeDefendsRightGoal
            } else {
                SideMapping::HomeDefendsLeftGoal
            };
        }
    }

    fn team_settings(&mut self, ui: &mut Ui, rect: Rect, side: Side, teams: &[Team]) {
        let rem = 16.0;
        let game = &self.settings.game;
        let other = -side;
        let team_legal = game.teams[Side::Home].number != game.teams[Side::Away].number;
        let field_player_color_legal = game.teams[side].field_player_color
            != game.teams[other].goalkeeper_color
            && game.teams[Side::Home].field_player_color
                != game.teams[Side::Away].field_player_color;
        let goalkeeper_color_legal =
            game.teams[side].goalkeeper_color != game.teams[other].field_player_color;

        // The section frame.
        ui.painter().rect_stroke(
            rect,
            style::RADIUS,
            Stroke::new(1.0, tw::GRAY_600),
            egui::StrokeKind::Inside,
        );
        let inner = rect.shrink(rem * 0.75);
        let title_h = rem * 1.75;
        let title_rect = Rect::from_min_size(inner.min, Vec2::new(inner.width(), title_h));
        style::paint_galley(
            ui.painter(),
            ui.painter().layout_no_wrap(
                match side {
                    Side::Home => "Home",
                    Side::Away => "Away",
                }
                .into(),
                font(rem * 1.25, Weight::SemiBold),
                tw::BLACK,
            ),
            Align2::LEFT_CENTER,
            title_rect.left_center(),
        );
        let mut kick_off = self.settings.game.kick_off_side == side;
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(title_rect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        child.push_id(("kick-off", side), |ui| {
            if ui.radio(kick_off, "Kick-off").clicked() {
                kick_off = true;
            }
        });
        if kick_off {
            self.settings.game.kick_off_side = side;
        }

        // The team and its jersey colors, evenly distributed in the remaining space.
        let field_h = rem * (1.25 + 0.25 + 1.75);
        let body = Rect::from_min_max(
            Pos2::new(inner.left(), title_rect.bottom() + rem * 0.5),
            inner.max,
        );
        let spare = (body.height() - 2.0 * field_h).max(0.0) / 3.0;
        let team_rect = Rect::from_min_size(
            Pos2::new(body.left(), body.top() + spare),
            Vec2::new(body.width(), field_h),
        );
        let colors_rect = team_rect.translate(Vec2::new(0.0, field_h + spare));
        let half = (colors_rect.width() - rem * 0.5) / 2.0;
        let fp_rect = Rect::from_min_size(colors_rect.min, Vec2::new(half, field_h));
        let gk_rect = fp_rect.translate(Vec2::new(half + rem * 0.5, 0.0));

        let team_params = self.settings.game.teams[side].clone();
        let mut number = team_params.number;
        field(ui, team_rect, "Team", |ui, label| {
            let selected = teams
                .iter()
                .find(|t| t.number == number)
                .map(|t| format!("{} ({})", t.name, t.number))
                .unwrap_or_default();
            select(ui, label, ("team", side), selected, team_legal, |ui| {
                for team in teams {
                    ui.selectable_value(
                        &mut number,
                        team.number,
                        format!("{} ({})", team.name, team.number),
                    );
                }
            });
        });
        if number != team_params.number {
            let team = self.team(number).clone();
            self.settings.game.teams[side] = TeamParams {
                number,
                field_player_color: team.field_player_colors[0],
                goalkeeper_color: team.goalkeeper_colors[0],
            };
        }

        let team = self.team(self.settings.game.teams[side].number).clone();
        let params = &mut self.settings.game.teams[side];
        color_field(
            ui,
            fp_rect,
            "Field Player Color",
            ("fp", side),
            &team.field_player_colors,
            &mut params.field_player_color,
            field_player_color_legal,
        );
        color_field(
            ui,
            gk_rect,
            "Goalkeeper Color",
            ("gk", side),
            &team.goalkeeper_colors,
            &mut params.goalkeeper_color,
            goalkeeper_color_legal,
        );
    }
}

/// This function splits a rectangle into the columns `1fr 5rem 1fr` with 0.75rem gaps.
fn three_columns(rect: Rect) -> [Rect; 3] {
    let gap = 12.0;
    let middle = 80.0;
    let side = (rect.width() - middle - 2.0 * gap) / 2.0;
    let col =
        |x: f32, w: f32| Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height()));
    [
        col(rect.left(), side),
        col(rect.left() + side + gap, middle),
        col(rect.right() - side, side),
    ]
}

/// This function draws a labeled form field (a small gray label above a control).
fn field(ui: &mut Ui, rect: Rect, label: &str, add_contents: impl FnOnce(&mut Ui, Id)) {
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.spacing_mut().item_spacing.y = 4.0;
    let label = child.add(
        egui::Label::new(RichText::new(label).size(14.0).color(tw::GRAY_600))
            .wrap_mode(egui::TextWrapMode::Extend),
    );
    add_contents(&mut child, label.id);
}

/// This function draws a dropdown that fills the available width, with a red border if its value
/// is not legal.
fn select(
    ui: &mut Ui,
    label: Id,
    id: impl std::hash::Hash + std::fmt::Debug,
    selected: String,
    legal: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    let border = if legal { tw::GRAY_600 } else { tw::RED_600 };
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        for w in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            w.bg_stroke = Stroke::new(1.0, border);
        }
        egui::ComboBox::from_id_salt(Id::unique(id))
            .selected_text(selected)
            .width(ui.available_width())
            .height(320.0)
            .truncate()
            .icon(chevrons_up_down)
            .show_ui(ui, add_contents)
            .response
            .labelled_by(label);
    });
}

/// The icon of a dropdown (like the native select of macOS).
fn chevrons_up_down(ui: &Ui, rect: Rect, visuals: &egui::style::WidgetVisuals, _open: bool) {
    let rect = Rect::from_center_size(rect.center(), Vec2::new(8.0, 12.0));
    let stroke = Stroke::new(1.5, visuals.fg_stroke.color);
    let painter = ui.painter();
    let (l, r, c) = (rect.left(), rect.right(), rect.center().x);
    painter.line_segment(
        [Pos2::new(l, rect.top() + 4.0), Pos2::new(c, rect.top())],
        stroke,
    );
    painter.line_segment(
        [Pos2::new(c, rect.top()), Pos2::new(r, rect.top() + 4.0)],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(l, rect.bottom() - 4.0),
            Pos2::new(c, rect.bottom()),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(c, rect.bottom()),
            Pos2::new(r, rect.bottom() - 4.0),
        ],
        stroke,
    );
}

fn color_field(
    ui: &mut Ui,
    rect: Rect,
    label: &str,
    id: impl std::hash::Hash + std::fmt::Debug,
    colors: &[Color],
    color: &mut Color,
    legal: bool,
) {
    field(ui, rect, label, |ui, label| {
        select(ui, label, id, color_label(*color), legal, |ui| {
            for c in colors {
                ui.selectable_value(color, *c, color_label(*c));
            }
        });
    });
}

/// The launcher shows the color names in lower case, as they are written in the configuration.
fn color_label(color: Color) -> String {
    style::color_name(color).to_lowercase()
}

/// This function draws a collapsible group of settings (like HTML's details element).
fn details(ui: &mut Ui, rect: Rect, summary: &str, add_contents: impl FnOnce(&mut Ui)) {
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    egui::CollapsingHeader::new(RichText::new(summary).size(14.0).color(tw::GRAY_600))
        .id_salt(summary)
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            add_contents(ui);
        });
}

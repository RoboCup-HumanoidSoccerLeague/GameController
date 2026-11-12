//! This module implements the launcher, in which the competition, the teams and other settings are
//! chosen before the game starts.

use eframe::egui::{
    self, vec2, Align, Button, CentralPanel, Frame, Id, Label, Layout, Panel, Pos2, Rect, RichText,
    Sense, Stroke, Ui, Vec2,
};

use game_controller_core::types::{Color, Side, SideMapping, TeamParams};
use game_controller_runtime::launch::{Competition, LaunchData, LaunchSettings, Team};

use crate::style::{self, font, panel_frame, tw, Weight};

/// The number of the team that is part of every competition and is selected by default.
const DEFAULT_TEAM_NUMBER: u8 = 0;

/// The margin of the window.
const MARGIN: i8 = 16;
/// The space between the settings.
const SPACING: f32 = 12.0;
/// The settings are laid out in two columns with a narrow one in between.
const MIDDLE_COLUMN_WIDTH: f32 = 80.0;
/// The height of the settings of a team.
const TEAM_HEIGHT: f32 = 230.0;

/// The launcher.
pub struct Launcher {
    data: LaunchData,
    settings: LaunchSettings,
    /// The teams of the selected competition, sorted by name. This only changes with the
    /// competition.
    competition_teams: Vec<Team>,
}

/// This function returns a team by its number.
fn find_team(teams: &[Team], number: u8) -> &Team {
    teams
        .iter()
        .find(|team| team.number == number)
        .expect("team must exist")
}

/// This function returns the teams of a competition, sorted by name, ignoring case.
fn competition_teams(data: &LaunchData, competition_id: &str) -> Vec<Team> {
    let competition: &Competition = data
        .competitions
        .iter()
        .find(|c| c.id == competition_id)
        .expect("competition must exist");
    let mut teams: Vec<Team> = data
        .teams
        .iter()
        .filter(|team| competition.teams.contains(&team.number))
        .cloned()
        .collect();
    teams.sort_by_cached_key(|team| team.name.to_lowercase());
    teams
}

impl Launcher {
    /// This function creates a launcher that starts with the default settings of the launch data.
    pub fn new(data: LaunchData) -> Self {
        let settings = data.default_settings.clone();
        let competition_teams = competition_teams(&data, &settings.competition.id);
        Self {
            data,
            settings,
            competition_teams,
        }
    }

    /// The launch data that the launcher was created with.
    pub fn data(&self) -> &LaunchData {
        &self.data
    }

    fn team(&self, number: u8) -> &Team {
        find_team(&self.data.teams, number)
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
        self.competition_teams = competition_teams(&self.data, &id);
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
        let legal = self.settings_are_legal();
        let start = Panel::bottom("start")
            .frame(panel_frame(MARGIN))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    if !legal {
                        style::set_disabled(ui);
                    }
                    let text = RichText::new("Start").font(font(20.0, Weight::SemiBold));
                    let button = Button::new(text).min_size(vec2(120.0, 46.0));
                    ui.add_enabled(legal, button).clicked()
                })
                .inner
            })
            .inner;

        CentralPanel::default()
            .frame(panel_frame(MARGIN))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(SPACING);
                self.competition(ui);
                self.game_settings(ui);
                self.other_settings(ui);
            });

        start.then(|| self.settings.clone())
    }

    fn competition(&mut self, ui: &mut Ui) {
        let competitions = &self.data.competitions;
        let mut competition_id = self.settings.competition.id.clone();
        field(ui, "Competition", |ui, label| {
            let selected = competitions
                .iter()
                .find(|c| c.id == competition_id)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            select(ui, label, "competition", selected, true, |ui| {
                for competition in competitions {
                    ui.selectable_value(
                        &mut competition_id,
                        competition.id.clone(),
                        &competition.name,
                    );
                }
            });
        });
        self.set_competition(competition_id);
    }

    /// This function draws the settings of the two teams, on the sides on which they play, with
    /// the control to swap the sides in between.
    fn game_settings(&mut self, ui: &mut Ui) {
        let mirrored = self.settings.game.side_mapping == SideMapping::HomeDefendsRightGoal;
        let (left, right) = if mirrored {
            (Side::Away, Side::Home)
        } else {
            (Side::Home, Side::Away)
        };
        let width = side_column_width(ui);
        let size = vec2(ui.available_width(), TEAM_HEIGHT);
        ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
            column(ui, vec2(width, TEAM_HEIGHT), |ui| {
                self.team_settings(ui, left);
            });

            let height = 24.0 + 4.0 + ui.spacing().interact_size.y;
            let size = vec2(MIDDLE_COLUMN_WIDTH, height);
            ui.allocate_ui_with_layout(size, Layout::top_down(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                style::icons::arrows_right_left(ui.painter(), rect, tw::BLACK);
                let mut mirror = mirrored;
                ui.checkbox(&mut mirror, "Mirror")
                    .on_hover_text("Swap the sides: the home team starts on the right");
                self.settings.game.side_mapping = if mirror {
                    SideMapping::HomeDefendsRightGoal
                } else {
                    SideMapping::HomeDefendsLeftGoal
                };
            });

            column(ui, vec2(width, TEAM_HEIGHT), |ui| {
                self.team_settings(ui, right);
            });
        });
    }

    fn team_settings(&mut self, ui: &mut Ui, side: Side) {
        let game = &self.settings.game;
        let other = -side;
        let team_legal = game.teams[Side::Home].number != game.teams[Side::Away].number;
        let field_player_color_legal = game.teams[side].field_player_color
            != game.teams[other].goalkeeper_color
            && game.teams[Side::Home].field_player_color
                != game.teams[Side::Away].field_player_color;
        let goalkeeper_color_legal =
            game.teams[side].goalkeeper_color != game.teams[other].field_player_color;

        Frame::group(ui.style())
            .inner_margin(SPACING)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());

                ui.horizontal(|ui| {
                    ui.heading(match side {
                        Side::Home => "Home",
                        Side::Away => "Away",
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.radio_value(&mut self.settings.game.kick_off_side, side, "Kick-off");
                    });
                });
                ui.add_space(SPACING);

                let team_number = self.settings.game.teams[side].number;
                let mut number = team_number;
                let teams = &self.competition_teams;
                field(ui, "Team", |ui, label| {
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
                let team = find_team(&self.data.teams, number);
                let params = &mut self.settings.game.teams[side];
                if number != team_number {
                    *params = TeamParams {
                        number,
                        field_player_color: team.field_player_colors[0],
                        goalkeeper_color: team.goalkeeper_colors[0],
                    };
                }
                ui.add_space(SPACING);

                ui.columns(2, |columns| {
                    color_field(
                        &mut columns[0],
                        "Field Player Color",
                        ("fp", side),
                        &team.field_player_colors,
                        &mut params.field_player_color,
                        field_player_color_legal,
                    );
                    color_field(
                        &mut columns[1],
                        "Goalkeeper Color",
                        ("gk", side),
                        &team.goalkeeper_colors,
                        &mut params.goalkeeper_color,
                        goalkeeper_color_legal,
                    );
                });
            });
    }

    /// This function draws the settings that are not about the game: network and window on the
    /// sides, each with a group of less common settings below.
    fn other_settings(&mut self, ui: &mut Ui) {
        let interfaces = &self.data.network_interfaces;
        let network = &mut self.settings.network;
        let fullscreen = &mut self.settings.window.fullscreen;
        let test = &mut self.settings.game.test;
        let width = side_column_width(ui);
        ui.horizontal_top(|ui| {
            column(ui, vec2(width, 0.0), |ui| {
                field(ui, "Network Interface", |ui, label| {
                    let selected = network.interface.clone();
                    select(ui, label, "interface", selected, true, |ui| {
                        for interface in interfaces {
                            ui.selectable_value(
                                &mut network.interface,
                                interface.id.clone(),
                                &interface.id,
                            );
                        }
                    });
                });
                details(ui, "Casting (Advanced)", |ui| {
                    ui.checkbox(&mut network.broadcast, "Broadcast");
                    ui.checkbox(&mut network.multicast, "Multicast");
                });
            });
            // The middle column is empty.
            ui.add_space(MIDDLE_COLUMN_WIDTH + SPACING);
            column(ui, vec2(width, 0.0), |ui| {
                field(ui, "Window", |ui, label| {
                    ui.checkbox(fullscreen, "Fullscreen").labelled_by(label);
                });
                details(ui, "Testing", |ui| {
                    ui.checkbox(&mut test.no_delay, "No Delay");
                    ui.checkbox(&mut test.penalty_shootout, "Penalty Shoot-out");
                });
            });
        });
    }
}

/// This function returns the width of the side columns of a row that has a narrow column in the
/// middle.
fn side_column_width(ui: &Ui) -> f32 {
    (ui.available_width() - MIDDLE_COLUMN_WIDTH - 2.0 * SPACING) / 2.0
}

/// This function adds a column of a given size (which grows with its contents) to a row.
fn column(ui: &mut Ui, size: Vec2, add_contents: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(size);
        ui.set_max_width(size.x);
        add_contents(ui);
    });
}

/// This function draws a labeled form field (a small gray label above a control).
fn field(ui: &mut Ui, label: &str, add_contents: impl FnOnce(&mut Ui, Id)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let label = ui.add(Label::new(RichText::new(label).small().color(tw::GRAY_600)).truncate());
        add_contents(ui, label.id);
    });
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
    label: &str,
    id: impl std::hash::Hash + std::fmt::Debug,
    colors: &[Color],
    color: &mut Color,
    legal: bool,
) {
    field(ui, label, |ui, label| {
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
fn details(ui: &mut Ui, summary: &str, add_contents: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(RichText::new(summary).small().color(tw::GRAY_600))
        .id_salt(summary)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            add_contents(ui);
        });
}

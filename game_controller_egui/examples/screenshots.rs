//! This program renders screenshots of the GameController UI for the scenarios in
//! [game_controller_egui::scenarios].
//!
//! - `egui --out <dir>` renders the egui UI headlessly (wgpu) to PNG files.
//! - `web-data --actions <file> --out <file>` writes the launch data and the scenario states as JSON
//!   for the web UI harness in `tools/screenshots`, which renders the original React frontend. The
//!   legality masks are computed for the action list that the web UI declares (`--actions`, as
//!   dumped from `frontend/src/actions.js`).

use std::{
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use eframe::egui;
use egui_kittest::{kittest::Queryable, Harness};
use serde::Serialize;

use game_controller_core::{
    action::VAction,
    types::{Color, Params, Side, SideMapping, TeamParams},
};
use game_controller_egui::{
    actions::declared_actions,
    launcher::Launcher,
    main_view::MainView,
    scenarios::{make_scenarios, Scenario},
    style,
};
use game_controller_runtime::{
    cli::Args as GcArgs,
    launch::{make_launch_data, LaunchData},
};

#[derive(Parser)]
struct Args {
    /// The configuration directory.
    #[arg(long, default_value = "config")]
    config: PathBuf,
    /// The competition whose parameters are used.
    #[arg(long, default_value = "small_advanced")]
    competition: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render the egui UI to PNG files.
    Egui {
        #[arg(long)]
        out: PathBuf,
        /// The size of the main window.
        #[arg(long, default_value_t = 1040.0)]
        width: f32,
        #[arg(long, default_value_t = 848.0)]
        height: f32,
        /// The scale factor (physical pixels per point).
        #[arg(long, default_value_t = 2.0)]
        scale: f32,
    },
    /// Write the scenarios as JSON for the web UI harness.
    WebData {
        #[arg(long)]
        actions: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebScenario<'a> {
    name: &'a str,
    params: &'a Params,
    team_names: [String; 2],
    state: &'a game_controller_runtime::UiState,
    clicks: Vec<WebClick>,
}

#[derive(Serialize)]
struct WebClick {
    label: &'static str,
    index: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebData<'a> {
    launchers: Vec<(&'static str, LaunchData)>,
    scenarios: Vec<WebScenario<'a>>,
}

/// This function picks two teams of the competition and jersey colors that don't clash.
fn game_params(config_dir: &Path, launch_data: &LaunchData, competition: &str) -> Result<Params> {
    let config = launch_data
        .competitions
        .iter()
        .find(|c| c.id == competition)
        .context("unknown competition")?;
    let team = |number: u8| {
        launch_data
            .teams
            .iter()
            .find(|t| t.number == number)
            .unwrap()
    };
    let home = team(config.teams[1]);
    let away = team(config.teams[2]);
    let home_params = TeamParams {
        number: home.number,
        field_player_color: home.field_player_colors[0],
        goalkeeper_color: home.goalkeeper_colors[0],
    };
    let pick = |colors: &[Color], avoid: &[Color]| {
        *colors
            .iter()
            .find(|c| !avoid.contains(c))
            .unwrap_or(&colors[0])
    };
    let away_params = TeamParams {
        number: away.number,
        field_player_color: pick(
            &away.field_player_colors,
            &[home_params.field_player_color, home_params.goalkeeper_color],
        ),
        goalkeeper_color: pick(&away.goalkeeper_colors, &[home_params.field_player_color]),
    };
    let mut game = launch_data.default_settings.game.clone();
    game.teams[Side::Home] = home_params;
    game.teams[Side::Away] = away_params;
    Ok(Params {
        competition: serde_yaml::from_reader(File::open(
            config_dir.join(competition).join("params.yaml"),
        )?)?,
        game,
    })
}

/// The launcher states that are rendered: the defaults, and a configured game.
fn launchers(launch_data: &LaunchData, params: &Params) -> Vec<(&'static str, LaunchData)> {
    let mut configured = launch_data.clone();
    configured.default_settings.game = params.game.clone();
    configured.default_settings.game.side_mapping = SideMapping::HomeDefendsRightGoal;
    configured.default_settings.game.kick_off_side = Side::Away;
    vec![
        ("00_launcher", launch_data.clone()),
        ("00_launcher_configured", configured),
    ]
}

fn team_names(launch_data: &LaunchData, params: &Params) -> [String; 2] {
    [Side::Home, Side::Away].map(|side| {
        launch_data
            .teams
            .iter()
            .find(|t| t.number == params.game.teams[side].number)
            .unwrap()
            .name
            .clone()
    })
}

/// The harness puts the UI into a panel with a margin, but the app gets the whole window.
fn whole_window(ui: &mut egui::Ui) -> egui::Ui {
    let rect = ui.ctx().content_rect();
    ui.new_child(egui::UiBuilder::new().max_rect(rect))
}

fn render_scenario(
    scenario: &Scenario,
    names: [String; 2],
    args: (f32, f32, f32),
) -> Harness<'static, MainView> {
    let (width, height, scale) = args;
    let state = scenario.state.clone();
    let view = MainView::new(scenario.params.clone(), names);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(width, height))
        .with_pixels_per_point(scale)
        .with_context_setup(style::setup)
        .with_render_cursor(false)
        .wgpu()
        .build_ui_state(
            move |ui, view: &mut MainView| {
                let _ = view.ui(&mut whole_window(ui), &state);
            },
            view,
        );
    harness.run_steps(2);
    for click in &scenario.clicks {
        harness
            .get_all_by_label(click.label)
            .nth(click.index)
            .unwrap_or_else(|| panic!("no button {:?}", click))
            .click();
        harness.run_steps(2);
        // Move the mouse away, so that no button is drawn hovered.
        harness.event(egui::Event::PointerGone);
        harness.run_steps(2);
    }
    harness
}

fn main() -> Result<()> {
    let args = Args::parse();
    let gc_args = GcArgs::parse_from(["GameController", "--competition", &args.competition]);
    let launch_data = make_launch_data(&args.config, gc_args).context("invalid configuration")?;
    let params = game_params(&args.config, &launch_data, &args.competition)?;

    match args.command {
        Command::Egui {
            out,
            width,
            height,
            scale,
        } => {
            std::fs::create_dir_all(&out)?;
            for (name, data) in launchers(&launch_data, &params) {
                let mut harness = Harness::builder()
                    .with_size(egui::vec2(640.0, 560.0))
                    .with_pixels_per_point(scale)
                    .with_context_setup(style::setup)
                    .wgpu()
                    .build_ui_state(
                        |ui, launcher: &mut Launcher| {
                            let _ = launcher.ui(&mut whole_window(ui));
                        },
                        Launcher::new(data),
                    );
                harness.run_steps(2);
                let image = harness.render().map_err(anyhow::Error::msg)?;
                image.save(out.join(format!("{name}.png")))?;
                eprintln!("wrote {name}");
            }
            for scenario in make_scenarios(&params, &declared_actions()) {
                let mut harness = render_scenario(
                    &scenario,
                    team_names(&launch_data, &scenario.params),
                    (width, height, scale),
                );
                let image = harness.render().map_err(anyhow::Error::msg)?;
                image.save(out.join(format!("{}.png", scenario.name)))?;
                eprintln!("wrote {}", scenario.name);
            }
        }
        Command::WebData { actions, out } => {
            let declared: Vec<VAction> = serde_json::from_reader(
                File::open(&actions).context("could not open the action list")?,
            )
            .context("could not parse the action list")?;
            assert!(
                declared == declared_actions(),
                "the action lists of the web UI and the egui UI differ"
            );
            let scenarios = make_scenarios(&params, &declared);
            let data = WebData {
                launchers: launchers(&launch_data, &params),
                scenarios: scenarios
                    .iter()
                    .map(|s| WebScenario {
                        name: s.name,
                        params: &s.params,
                        team_names: team_names(&launch_data, &s.params),
                        state: &s.state,
                        clicks: s
                            .clicks
                            .iter()
                            .map(|c| WebClick {
                                label: c.label,
                                index: c.index,
                            })
                            .collect(),
                    })
                    .collect(),
            };
            serde_json::to_writer(File::create(&out)?, &data)?;
            eprintln!("wrote {}", out.display());
        }
    }
    Ok(())
}

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

GameController for RoboCup humanoid robot soccer: a Rust desktop app (Cargo workspace) with an egui/eframe user interface. `README.md` documents the user-facing behavior (network protocol, launcher, penalties, undo, etc.) in detail; consult it when changing game logic.

## Commands

```bash
cargo build [-r]              # default member is game_controller_app
cargo run [-r] -- -h          # run the app; CLI args override launcher defaults
cargo clippy --workspace      # rust-analyzer is configured to use clippy
cargo fmt --all
EGUI_INSPECTION=1 cargo run --features eframe/inspection   # expose the UI to egui_inspection (e.g. the egui MCP) on 127.0.0.1:5719
cargo run -p game_controller_logs -- statistics [--header] <log.yaml>...   # or: team-communication <log.yaml>...
```

`dist/mkdist --stage-only --only api [<target>]` (`mkdist.ps1 -StageOnly -Only api` on Windows) builds the C API (`game_controller_api`) and leaves the shared library, headers and competition params in `dist/game_controller_api-<version>-<target>/` (on macOS without a target, the library is universal).

The only tests are the path/options-file tests of `game_controller_runtime` and the action-index test of `game_controller_app` (`cargo test --workspace`). `libclang` is required (bindgen in `game_controller_msgs`). Distributions are built into `dist/` by `dist/mkdist [--only app,api,logs] [--stage-only] [<target>]` (the version is the one in `Cargo.toml`; `mkdist.ps1` on Windows) using the `release-dist` profile. `dist/lib.sh`/`lib.ps1` hold shared functions; each product has `stage_<product>` (creates the directory: Linux prefix tree with `dist/linux/{install,uninstall}`, macOS `.app` in a dmg folder, packed with dmgbuild from a venv using `dist/macos/dmg-settings.py`, Windows folder with `dist/windows/{install,uninstall}.ps1`) and `pack_<product>` (archive/dmg) in `dist/product-<product>.sh`. (CI: `.github/workflows/mkdist.yml`, job `dist` on all runners, triggered on `v*` tags; on tags, the `release` job creates a draft GitHub release with all archives and notes from `dist/release-notes`).

Releases are described in `RELEASING.md`: SemVer with `-rc.N` release candidates; `dist/bump-version <version>` sets the version in `Cargo.toml` (the only place it is defined), updates the lock file, commits "Version X" and creates a lightweight tag.

## Architecture

Crates, from the bottom up:

- **game_controller_core** — pure game logic, no UI or networking. `GameController` (in `lib.rs`) owns the `Game` state, `Params`, the undo history, and a `Logger`. Time is advanced explicitly via `seek(dt)`; timers (`timer.rs`) have a `RunCondition` and may emit actions when they expire, so `seek` splits time at each expiration.
- **game_controller_msgs** — binary wire formats (control/status/monitor messages). Constants come from the C header `headers/RoboCupGameControlData.h` via bindgen (through the `bindings.h` wrapper).
- **game_controller_net** — tokio UDP senders/receivers for each channel; receivers push `Event`s into an mpsc channel.
- **game_controller_runtime** — glues core + net: `start_runtime` spawns network tasks and the `event_loop`, which sends `UiState` to the UI, publishes the (delayed) game to the control message sender, computes the next deadline (timer expiration / whole-second wrap / connection status change), then awaits a deadline, network event, UI action, or shutdown. Also handles launch data from the config directory, CLI (`cli.rs`, including the persistent options file `args` that is inserted before the real arguments), file locations (`paths.rs`: config search path, user directory), and YAML file logging.
- **game_controller_app** — the application (binary + library, egui/eframe, Rust ≥ 1.95). `main.rs` handles `--show-paths`/`--install-config` and starts eframe; `app.rs` (`App`) resolves the config directory, shows the launcher, calls `start_runtime` on Start (UI states arrive through a callback into an `Arc<Mutex<Option<UiState>>>` plus `request_repaint`), sends actions through `RuntimeState::action_sender`, and calls `shutdown_runtime` in `on_exit`. The config directory is the first existing of `--config-dir`/`GAMECONTROLLER_CONFIG`, the user's `config` directory, the installed one relative to the real exe path (`../share/GameController/config` on Linux, `../Resources/config` in the macOS bundle, `config` next to the exe on Windows) and `<exe>/../../config` (source checkout); it is reported on stderr and by `--show-paths`; the per-user directory is `GameController/{args,config,logs}` below `$XDG_CONFIG_HOME` (`~/.config`; logs below `$XDG_DATA_HOME`, `~/.local/share`) on Linux, `~/Library/Application Support` on macOS and `%LOCALAPPDATA%` on Windows (`paths.rs`). `launcher.rs` and `main_view.rs` are the two screens; the views take a `UiState` and return `Vec<VAction>` (they hold only local UI state: selected penalty call, substitution/goalkeeper/shoot-out selection). Both are built from egui panels, layouts and standard widgets (`Button`, `Label`, `ComboBox`, ...); nothing is placed or painted by hand except the "+" button next to the clock (`Ui::place`) and the icons. The main window is scaled as a whole with `Context::set_zoom_factor` (`main_view::zoom_factor`: body text is `clamp(16, min(1.1% width, 1.9% height), 40)` native points), so all lengths in `main_view.rs` are constants in multiples of `REM`; team and center panel rows line up because they use the same row heights. `style.rs` has the color palette, the bundled Inter fonts (`fonts/`, OFL; egui has no font weights, so each weight is its own family), the egui `Style` that gives the widgets their look, the icons, and `tabular`/`append_tabular`, which build layout jobs with fixed-width digits for timers. Buttons whose action is illegal are disabled (`add_enabled`). egui's button layout can't center the lines of multi-line text, which is why the player buttons indent their shorter line.
- **game_controller_autoreferee** — automatic referee for simulators on top of the core (pure Rust; `Features` bitflags, `update(gc, world) -> Vec<Command>`).
- **game_controller_api** — `cdylib` exposing the core as a C ABI (`gc_*` functions); cbindgen writes `headers/GameController.h` (gitignored) at build time.
- **game_controller_logs** — CLI for analyzing YAML log files.

### Actions

All state changes are actions in `game_controller_core/src/actions/`, each a struct implementing `Action { execute, is_legal }`, combined in the `VAction` enum (`action.rs`, via `trait_enum!`), serialized as `{ "type": camelCase, "args": ... }`. Actions often compose by calling other actions' `execute` directly (e.g. `Goal` → `StartSetPlay`/`FinishHalf`).

Adding an action requires: new module + `pub use` in `actions/mod.rs`, a variant in `VAction`, and usually changes to the UI (`game_controller_app`) and to `game_controller_api` if it should be exposed via C.

Key mechanisms in `ActionContext`:
- **Delayed game state (`fork`)**: after some transitions (e.g. to Playing), a copy of the pre-action state is sent to robots for a period (see README "Network Communication"). Subsequent actions are also applied to the delayed state; if one is illegal there (and not accepted by the `ignore` predicate), the delay is canceled. Monitors receive the true state.
- **Undo history**: only `ActionSource::User` actions are recorded; `StopPlay` is excluded and `game.stopped` survives undo.

### UI ↔ runtime legality protocol

The UI never evaluates rules itself. When the game starts, `app.rs` sends `declared_actions()` (a fixed, ordered list of every button's action) through `RuntimeState::subscribed_actions_sender`; each `UiState` then contains `legal_actions` with one bit per declared action in the same order. `game_controller_app/src/actions.rs` defines that order and typed accessors into the mask (`Legal`); a unit test checks that the index functions match `declared_actions()`. The runtime still applies only legal actions, so illegal clicks (e.g. a substitution during Playing) are silently dropped.

## Configuration

`config/teams.yaml` lists all teams and jersey colors. Each competition subdirectory has `params.yaml` (deserialized into `game_controller_core::types::CompetitionParams`; `Option` fields such as `extraHalfDuration`, `mercyRuleScoreDifference`, or `delayAfterPlaying` disable the feature when empty/absent) and `teams.yaml` (participating team numbers). A new team must be added to both the global and the competition's `teams.yaml`.

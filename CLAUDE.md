# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

GameController for RoboCup humanoid robot soccer: a Tauri 2 desktop app with a Rust backend (Cargo workspace) and a React frontend in `frontend/`. `README.md` documents the user-facing behavior (network protocol, launcher, penalties, undo, etc.) in detail; consult it when changing game logic.

## Commands

The frontend must be built before the Rust app compiles (Tauri embeds `frontend/build`):

```bash
cd frontend && npm ci && npm run build   # production frontend build
npm run lint                             # eslint (in frontend/)
npm run format                           # prettier, printWidth 100 (in frontend/)
```

```bash
cargo build [-r]              # default member is game_controller_app
cargo run [-r] -- -h          # run the app; CLI args override launcher defaults
cargo clippy --workspace      # rust-analyzer is configured to use clippy
cargo fmt --all
cargo tauri dev               # needs `cargo install tauri-cli`; runs webpack dev server on :3000 + backend
cargo run -p game_controller_logs -- statistics [--header] <log.yaml>...   # or: team-communication <log.yaml>...
```

There is no test suite in this repository. `libclang` is required (bindgen in `game_controller_msgs`). Distributions are built by `dist/mkdist-{linux,macos,windows.ps1} <version> [<target>]` using the `release-dist` profile (CI: `.github/workflows/mkdist.yml`, triggered on `v*` tags).

The frontend can be opened standalone in a browser (`npm run dev`): `frontend/src/api.js` returns mock data when `window.__TAURI_INTERNALS__` is absent.

## Architecture

Crates, from the bottom up:

- **game_controller_core** — pure game logic, no UI or networking. `GameController` (in `lib.rs`) owns the `Game` state, `Params`, the undo history, and a `Logger`. Time is advanced explicitly via `seek(dt)`; timers (`timer.rs`) have a `RunCondition` and may emit actions when they expire, so `seek` splits time at each expiration.
- **game_controller_msgs** — binary wire formats (control/status/monitor messages). Constants come from the C header `headers/RoboCupGameControlData.h` via bindgen (through the `bindings.h` wrapper).
- **game_controller_net** — tokio UDP senders/receivers for each channel; receivers push `Event`s into an mpsc channel.
- **game_controller_runtime** — glues core + net: `start_runtime` spawns network tasks and the `event_loop`, which sends `UiState` to the UI, publishes the (delayed) game to the control message sender, computes the next deadline (timer expiration / whole-second wrap / connection status change), then awaits a deadline, network event, UI action, or shutdown. Also handles launch data from `config/`, CLI (`cli.rs`), and YAML file logging to `logs/`.
- **game_controller_app** — Tauri main binary. `handlers.rs` exposes the commands `get_launch_data`, `launch`, `sync_with_backend`, `apply_action`, `declare_actions`; state is pushed to the UI via the `state` event. `config/` and `logs/` are resolved as `<exe>/../../`, which is why the binary must live in `target/<profile>/` (dist archives replicate this layout).
- **game_controller_api** — `cdylib` exposing the core as a C ABI (`gc_*` functions); cbindgen writes `headers/GameController.h` (gitignored) at build time.
- **game_controller_logs** — CLI for analyzing YAML log files.

### Actions

All state changes are actions in `game_controller_core/src/actions/`, each a struct implementing `Action { execute, is_legal }`, combined in the `VAction` enum (`action.rs`, via `trait_enum!`), serialized as `{ "type": camelCase, "args": ... }`. Actions often compose by calling other actions' `execute` directly (e.g. `Goal` → `StartSetPlay`/`FinishHalf`).

Adding an action requires: new module + `pub use` in `actions/mod.rs`, a variant in `VAction`, and usually changes to the frontend and to `game_controller_api` if it should be exposed via C.

Key mechanisms in `ActionContext`:
- **Delayed game state (`fork`)**: after some transitions (e.g. to Playing), a copy of the pre-action state is sent to robots for a period (see README "Network Communication"). Subsequent actions are also applied to the delayed state; if one is illegal there (and not accepted by the `ignore` predicate), the delay is canceled. Monitors receive the true state.
- **Undo history**: only `ActionSource::User` actions are recorded; `StopPlay` is excluded and `game.stopped` survives undo.

### Frontend ↔ backend legality protocol

The UI never evaluates rules itself. On startup `Main.jsx` calls `declareActions(getActions())` with a fixed, ordered list of every button's action; each `UiState` returns `legalActions` as a bit per declared action in the same order. `frontend/src/actions.js` defines the index layout (`*_ACTION_BASE`, `NUM_OF_*`, `PENALTIES`), so adding/reordering actions there must keep `getActions()` and the index constants in sync.

## Configuration

`config/teams.yaml` lists all teams and jersey colors. Each competition subdirectory has `params.yaml` (deserialized into `game_controller_core::types::CompetitionParams`; `Option` fields such as `extraHalfDuration`, `mercyRuleScoreDifference`, or `delayAfterPlaying` disable the feature when empty/absent) and `teams.yaml` (participating team numbers). A new team must be added to both the global and the competition's `teams.yaml`.

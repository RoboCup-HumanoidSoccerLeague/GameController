# Screenshots

Renders the same game situations with the original web UI (`frontend/`) and the egui UI
(`game_controller_egui`) for comparison. The situations are defined in
`game_controller_egui/src/scenarios.rs`; they drive the real `game_controller_core`, so states and
button legality are genuine.

```bash
# egui UI (headless wgpu) -> screenshots/egui/
cargo run -p game_controller_egui --example screenshots -- egui --out screenshots/egui

# web UI (production build in Playwright's WebKit with a stand-in for Tauri's IPC) -> screenshots/web/
cd tools/screenshots && npm install && npx playwright install webkit
node web.mjs [--size 1040x848] [--scale 2] [--competition small_advanced] [--skip-build]

# side by side -> screenshots/compare/
uv run --with pillow compare.py
```

`web.mjs` also writes `measurements.json` (element rectangles), which was used to calibrate the egui
layout.

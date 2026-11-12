// Renders screenshots of the original web UI (frontend/) for the scenarios defined in
// game_controller_egui/src/scenarios.rs, so that they can be compared with the egui UI.
//
// The production build of the frontend is loaded into Playwright's WebKit (the engine of the
// WKWebView that Tauri uses on macOS) with a stand-in for Tauri's IPC: the commands return the
// launch data and parameters, and the "state" event delivers states that were computed by the real
// game_controller_core (via `cargo run --example screenshots -- web-data`).
//
// Usage: node web.mjs [--out <dir>] [--competition <id>] [--scale <n>] [--size <w>x<h>]

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { webkit } from "playwright";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const { values: args } = parseArgs({
  options: {
    out: { type: "string", default: join(ROOT, "screenshots/web") },
    competition: { type: "string", default: "small_advanced" },
    scale: { type: "string", default: "2" },
    size: { type: "string", default: "1040x848" },
    "skip-build": { type: "boolean", default: false },
  },
});
const [width, height] = args.size.split("x").map(Number);
mkdirSync(args.out, { recursive: true });

// The action list that the web UI declares (actions.js is an ES module in a CommonJS package, so
// it is imported from source).
const actionsSource = readFileSync(join(ROOT, "frontend/src/actions.js"), "utf8");
const { getActions } = await import(
  "data:text/javascript," + encodeURIComponent(actionsSource)
);
const actionsFile = join(args.out, "web-actions.json");
writeFileSync(actionsFile, JSON.stringify(getActions()));

if (!args["skip-build"]) {
  execFileSync("npm", ["run", "build"], { cwd: join(ROOT, "frontend"), stdio: "inherit" });
}
const dataFile = join(args.out, "web-data.json");
execFileSync(
  "cargo",
  [
    "run",
    "--quiet",
    "-p",
    "game_controller_egui",
    "--example",
    "screenshots",
    "--",
    "--config",
    join(ROOT, "config"),
    "--competition",
    args.competition,
    "web-data",
    "--actions",
    actionsFile,
    "--out",
    dataFile,
  ],
  { cwd: ROOT, stdio: "inherit" },
);
const data = JSON.parse(readFileSync(dataFile, "utf8"));

const MIME = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css" };
const BUILD = join(ROOT, "frontend/build");

// This runs in the page before the app: a minimal implementation of window.__TAURI_INTERNALS__.
const shim = ({ launchData, params, state }) => {
  const callbacks = new Map();
  let nextId = 1;
  let stateHandler = null;
  window.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { label: "main", windowLabel: "main" },
    },
    transformCallback: (callback) => {
      const id = nextId++;
      callbacks.set(id, callback);
      return id;
    },
    unregisterCallback: (id) => callbacks.delete(id),
    invoke: async (cmd, invokeArgs) => {
      switch (cmd) {
        case "get_launch_data":
          return launchData;
        case "launch":
          return null;
        case "plugin:event|listen":
          if (invokeArgs.event === "state") {
            stateHandler = callbacks.get(invokeArgs.handler);
          }
          return nextId++;
        case "plugin:event|unlisten":
          return null;
        case "sync_with_backend":
          return params;
        case "declare_actions":
          // The runtime would now start sending states.
          setTimeout(() => stateHandler && stateHandler({ event: "state", id: 0, payload: state }));
          return null;
        default:
          return null;
      }
    },
  };
};

// Flashing (animated) elements are frozen in their initial phase so screenshots are reproducible.
const FREEZE_CSS = "*, *::before, *::after { animation-play-state: paused !important; }";

const browser = await webkit.launch();

const openPage = async (viewport, shimData) => {
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: Number(args.scale),
  });
  await context.route("http://gc.local/**", (route) => {
    const path = new URL(route.request().url()).pathname;
    const file = join(BUILD, path === "/" ? "index.html" : path);
    route.fulfill({
      body: readFileSync(file),
      contentType: MIME[extname(file)] ?? "application/octet-stream",
    });
  });
  const page = await context.newPage();
  await page.addInitScript(shim, shimData);
  await page.goto("http://gc.local/");
  await page.addStyleTag({ content: FREEZE_CSS });
  return { context, page };
};

const measurements = {};

for (const [name, launchData] of data.launchers) {
  const { context, page } = await openPage({ width: 640, height: 560 }, { launchData });
  await page.getByRole("button", { name: "Start" }).waitFor();
  await page.screenshot({ path: join(args.out, `${name}.png`) });
  console.log(`wrote ${name}`);
  await context.close();
}

// A launch data whose defaults are legal, so the Start button can be clicked.
const startableLaunchData = data.launchers[data.launchers.length - 1][1];

for (const scenario of data.scenarios) {
  const { context, page } = await openPage(
    { width, height },
    { launchData: startableLaunchData, params: scenario.params, state: scenario.state },
  );
  await page.getByRole("button", { name: "Start" }).click();
  await page.getByRole("button", { name: "Referee Timeout" }).waitFor();
  for (const click of scenario.clicks) {
    await page.getByRole("button", { name: click.label, exact: true }).nth(click.index).click();
  }
  await page.mouse.move(0, 0);
  await page.screenshot({ path: join(args.out, `${scenario.name}.png`) });
  measurements[scenario.name] = await page.evaluate(() => {
    const rect = (e) => {
      const r = e.getBoundingClientRect();
      return [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 10) / 10);
    };
    const row = document.querySelector("body > div > div > div");
    return {
      rem: parseFloat(getComputedStyle(document.documentElement).fontSize),
      columns: [...row.children].map(rect),
      buttons: [...document.querySelectorAll("button")].map((b) => [b.innerText, rect(b)]),
    };
  });
  console.log(`wrote ${scenario.name}`);
  await context.close();
}

writeFileSync(join(args.out, "measurements.json"), JSON.stringify(measurements, null, 1));
await browser.close();

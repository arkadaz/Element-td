// Real Chromium/WebGPU smoke test without a Playwright dependency.
//
// Starts an isolated headless Edge profile, drives it through the DevTools
// protocol, clicks the single-player button, captures before/after screenshots
// in %TEMP%, and fails on a runtime error, missed transition, black frame, or
// visually flat frame. Merely producing two different PNGs is not sufficient.

import { spawn } from "node:child_process";
import { mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { createServer } from "node:net";

const EDGE = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
const URL = process.argv[2] ?? "http://127.0.0.1:8080";
// Each smoke run receives its own port from the caller.  This deliberately
// avoids attaching to, or disturbing, a player's existing browser session.
const PORT = Number(process.env.GREEN_TD_DEBUG_PORT ?? "9349");
if (!Number.isInteger(PORT) || PORT < 1024 || PORT > 65535) {
  throw new Error(`GREEN_TD_DEBUG_PORT must be a valid local TCP port, got ${PORT}`);
}
const tempRoot = realpathSync(tmpdir());
const profile = mkdtempSync(join(tempRoot, "green-td-edge-"));
const captureTag = process.env.GREEN_TD_CAPTURE_TAG ?? `p${PORT}`;
const viewportText = process.env.GREEN_TD_WINDOW_SIZE ?? "1440,1000";
const viewportParts = viewportText.split(",").map(Number);
if (viewportParts.length !== 2 || !viewportParts.every(Number.isFinite) ||
    viewportParts.some((value) => value < 320 || value > 6000)) {
  throw new Error(`GREEN_TD_WINDOW_SIZE must be WIDTH,HEIGHT, got ${viewportText}`);
}
const [VIEWPORT_W, VIEWPORT_H] = viewportParts.map((value) => Math.round(value));
const verifyResume = process.env.GREEN_TD_VERIFY_RESUME === "1";
// Isolated fallback coverage: disable WebGPU only for this throwaway Edge
// profile so wgpu selects its WebGL2 path. It never attaches to, alters, or
// relies on the player's browser/device state.
const forceWebGl2 = process.env.GREEN_TD_FORCE_WEBGL2 === "1";
// A deliberately narrow browser-evidence mode. It still clicks the visible
// Campaign menu entry; the URL only asks the Rust app to begin at one of the
// three documented boundary fixtures and to label the synthetic trace.
const diagnosticFixture = Number(process.env.GREEN_TD_CAMPAIGN_FIXTURE ?? "0");
if (![0, 35, 60, 599].includes(diagnosticFixture)) {
  throw new Error(`GREEN_TD_CAMPAIGN_FIXTURE must be 0, 35, 60, or 599, got ${diagnosticFixture}`);
}
const loadedUrl = diagnosticFixture > 0
  ? `${URL}${URL.includes("?") ? "&" : "?"}td_fixture=${diagnosticFixture}`
  : URL;
// A bounded optional observation window for genuine horde captures.  The
// ordinary smoke keeps its fast first-combat frame, while a visual run can
// wait for several authored packets to enter through the production spawn
// cursor instead of pretending a menu screenshot proves mass combat.
const captureDelayMs = Number(process.env.GREEN_TD_CAPTURE_DELAY_MS ?? "0");
if (!Number.isFinite(captureDelayMs) || captureDelayMs < 0 || captureDelayMs > 30_000) {
  throw new Error(`GREEN_TD_CAPTURE_DELAY_MS must be 0..30000, got ${captureDelayMs}`);
}
const qualityCycles = Number(process.env.GREEN_TD_QUALITY_CYCLES ?? "0");
if (!Number.isInteger(qualityCycles) || qualityCycles < 0 || qualityCycles > 2) {
  throw new Error(`GREEN_TD_QUALITY_CYCLES must be 0..2, got ${qualityCycles}`);
}
// Optional continuous-campaign proof: wait long enough for the real schedule
// to stream a successor, then require that the served build reached it. This
// is deliberately opt-in so the fast ordinary smoke remains bounded.
const expectMinEncounter = Number(process.env.GREEN_TD_EXPECT_MIN_ENCOUNTER ?? "0");
if (!Number.isInteger(expectMinEncounter) || expectMinEncounter < 0 || expectMinEncounter > 600) {
  throw new Error(`GREEN_TD_EXPECT_MIN_ENCOUNTER must be 0..600, got ${expectMinEncounter}`);
}
const shotBefore = join(tempRoot, `green-td-edge-menu-${captureTag}.png`);
const shotAfter = join(tempRoot, `green-td-edge-play-${captureTag}.png`);
// Captured after a real bottom-right command-card click and before any board
// click: this proves an armed placement ghost without spending gold or letting
// palette input leak through to the battlefield.
const shotArmed = join(tempRoot, `green-td-edge-armed-${captureTag}.png`);
// A close live board frame after a zoomed grass placement. This makes the
// actual tower body/material construction inspectable instead of asking a
// whole-board horde screenshot to prove tiny geometry is present.
const shotZoomed = join(tempRoot, `green-td-edge-zoomed-${captureTag}.png`);
const shotResumeMenu = join(tempRoot, `green-td-edge-resume-menu-${captureTag}.png`);
const shotResumed = join(tempRoot, `green-td-edge-resumed-${captureTag}.png`);

// Never use an occupied DevTools port.  Merely selecting the first page from a
// pre-existing endpoint can attach a smoke run to a player's actual tab, which
// is both unsafe and invalid evidence.  Binding first makes the test fail
// closed; Edge receives the port only after this guard releases it.
async function requireFreeLoopbackPort(port) {
  await new Promise((resolvePromise, reject) => {
    const guard = createServer();
    guard.once("error", () => {
      reject(new Error(`refusing to use occupied DevTools port ${port}; choose a new GREEN_TD_DEBUG_PORT`));
    });
    guard.listen({ host: "127.0.0.1", port }, () => {
      guard.close(resolvePromise);
    });
  });
}

await requireFreeLoopbackPort(PORT);

const edge = spawn(
  EDGE,
  [
    "--headless=new",
    ...(forceWebGl2
      ? ["--disable-webgpu", "--enable-webgl", "--ignore-gpu-blocklist"]
      : ["--enable-unsafe-webgpu"]),
    "--disable-gpu-sandbox",
    // An isolated visual/input test should exercise the served game, not a
    // globally installed spell checker injecting scripts into its document.
    "--disable-extensions",
    "--remote-debugging-address=127.0.0.1",
    `--remote-debugging-port=${PORT}`,
    `--user-data-dir=${profile}`,
    `--window-size=${VIEWPORT_W},${VIEWPORT_H}`,
    "about:blank",
  ],
  { stdio: "ignore", windowsHide: true },
);

const sleep = (ms) => new Promise((resolvePromise) => setTimeout(resolvePromise, ms));

async function endpoint() {
  for (let i = 0; i < 80; i += 1) {
    try {
      const tabs = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      // A newly spawned, isolated Edge starts exactly one about:blank page.
      // Do not fall back to another tab: that would make a collision with a
      // live browser session look like a valid local smoke test.
      const page = tabs.find((tab) => tab.type === "page" && tab.url === "about:blank");
      if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
    } catch {
      // Edge is still starting.
    }
    await sleep(100);
  }
  throw new Error("Edge DevTools endpoint did not start");
}

class Cdp {
  constructor(url) {
    this.ws = new WebSocket(url);
    this.id = 0;
    this.pending = new Map();
    this.events = [];
  }

  async open() {
    await new Promise((resolvePromise, reject) => {
      this.ws.onopen = resolvePromise;
      this.ws.onerror = reject;
    });
    this.ws.onmessage = (event) => {
      const message = JSON.parse(event.data);
      if (message.id) {
        const pending = this.pending.get(message.id);
        this.pending.delete(message.id);
        if (message.error) pending?.reject(new Error(message.error.message));
        else pending?.resolve(message.result);
      } else {
        this.events.push(message);
      }
    };
  }

  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((resolvePromise, reject) => {
      this.pending.set(id, { resolve: resolvePromise, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
}

async function screenshot(cdp, path) {
  const result = await cdp.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: false,
  });
  writeFileSync(path, Buffer.from(result.data, "base64"));
  return result.data;
}

async function click(cdp, x, y) {
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseMoved", x, y,
  });
  await sleep(80);
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mousePressed", x, y, button: "left", buttons: 1,
    pointerType: "mouse", clickCount: 1,
  });
  await sleep(80);
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseReleased", x, y, button: "left", buttons: 0,
    pointerType: "mouse", clickCount: 1,
  });
}

async function wheel(cdp, x, y, deltaY) {
  await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, pointerType: "mouse" });
  await sleep(60);
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseWheel", x, y, deltaX: 0, deltaY, pointerType: "mouse",
  });
}

async function middleDrag(cdp, from, to) {
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseMoved", x: from[0], y: from[1], pointerType: "mouse",
  });
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mousePressed", x: from[0], y: from[1], button: "middle", buttons: 4,
    pointerType: "mouse", clickCount: 1,
  });
  await sleep(50);
  await cdp.send("Input.dispatchMouseEvent", {
    // CDP reports the pressed button only on down/up. During a move the
    // button is `none` and the held-state lives in `buttons`; sending
    // `middle` here is silently ignored by some Chromium builds.
    type: "mouseMoved", x: to[0], y: to[1], button: "none", buttons: 4, pointerType: "mouse",
  });
  await sleep(50);
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseReleased", x: to[0], y: to[1], button: "middle", buttons: 0,
    pointerType: "mouse", clickCount: 1,
  });
}

// Keep the two contacts inside the actual scene instead of emulating a wheel:
// the Rust app receives egui's real multi-touch gesture, including its release
// frame. This catches the easy-to-miss bug where a pinch ending on grass also
// fires a build click.
async function touchPinch(cdp, centre, startRadius, endRadius) {
  await cdp.send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  const contacts = (radius) => [
    { x: centre[0] - radius, y: centre[1], radiusX: 4, radiusY: 4, force: 1, id: 1 },
    { x: centre[0] + radius, y: centre[1], radiusX: 4, radiusY: 4, force: 1, id: 2 },
  ];
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: contacts(startRadius) });
  // A real pinch is a sequence, not one teleport. In particular, a paused
  // mobile egui frame can coalesce the initial start with the first move;
  // multiple bounded moves give its touch state an actual previous/current
  // pair and mirror how a finger gesture arrives on-device.
  for (const t of [0.32, 0.66, 1.0]) {
    const radius = startRadius + (endRadius - startRadius) * t;
    await sleep(85);
    await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: contacts(radius) });
  }
  await sleep(100);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await sleep(70);
  await cdp.send("Emulation.setTouchEmulationEnabled", { enabled: false });
}

async function campaignSave(cdp) {
  const result = await cdp.send("Runtime.evaluate", {
    expression: `(() => {
      const raw = localStorage.getItem("green_td_save_v10");
      if (!raw) return { exists: false };
      try {
        const save = JSON.parse(raw);
        const campaign = save.campaign || null;
        return {
          exists: true,
          version: save.version,
          mode: save.mode,
          seed: save.seed,
          phase: save.phase,
          wave: save.wave,
          speed: save.speed,
          towerCount: Array.isArray(save.towers) ? save.towers.length : -1,
          towerPositions: Array.isArray(save.towers) ? save.towers.map(tower => ({
            slot: tower.slot,
            pos: tower.pos,
          })) : null,
          creepCount: Array.isArray(save.creeps) ? save.creeps.length : -1,
          encounter: campaign?.encounter ?? null,
          deployedPackets: campaign?.deployed_packets ?? null,
          queuedPacket: campaign?.queued_packet ?? null,
          queuedBodiesLeft: campaign?.queued_bodies_left ?? null,
          spawnedBodies: campaign?.spawned_bodies ?? null,
          rewardIssued: campaign?.reward_issued ?? null,
          activeSeconds: campaign?.active_seconds ?? null,
        };
      } catch (error) {
        return { exists: false, parseError: String(error) };
      }
    })()`,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(`reading Campaign persistence failed: ${JSON.stringify(result.exceptionDetails)}`);
  }
  return result.result.value;
}

async function runtimeQa(cdp) {
  const result = await cdp.send("Runtime.evaluate", {
    expression: `(() => {
      const root = document.documentElement;
      return {
        board: root.getAttribute("data-green-td-board-css"),
        inputProbe: root.getAttribute("data-green-td-input-probe"),
        probeWorld: root.getAttribute("data-green-td-probe-world"),
        invalidProbe: root.getAttribute("data-green-td-invalid-probe"),
        camera: root.getAttribute("data-green-td-camera"),
        hoverWorld: root.getAttribute("data-green-td-hover-world"),
        hoverBuildability: root.getAttribute("data-green-td-hover-buildability"),
        placementFeedback: root.getAttribute("data-green-td-placement-feedback"),
        palette: root.getAttribute("data-green-td-palette-css"),
        cards: root.getAttribute("data-green-td-card-css"),
        cardDefs: root.getAttribute("data-green-td-card-defs"),
        view: root.getAttribute("data-green-td-view-css"),
        speedControl: root.getAttribute("data-green-td-speed-css"),
        commands: root.getAttribute("data-green-td-command-css"),
        paused: root.getAttribute("data-green-td-paused") === "true",
        armedTower: root.getAttribute("data-green-td-armed-tower"),
        armedCost: Number(root.getAttribute("data-green-td-armed-cost") || 0),
        lastTower: root.getAttribute("data-green-td-last-tower"),
        gold: Number(root.getAttribute("data-green-td-gold") || 0),
        mode: root.getAttribute("data-green-td-mode"),
        encounter: root.getAttribute("data-green-td-encounter"),
        phase: root.getAttribute("data-green-td-phase"),
        speed: Number(root.getAttribute("data-green-td-speed") || 0),
        towers: Number(root.getAttribute("data-green-td-towers") || 0),
        creeps: Number(root.getAttribute("data-green-td-creeps") || 0),
        diagnostic: root.getAttribute("data-green-td-diagnostic"),
        backend: root.getAttribute("data-green-td-backend"),
        resumeRect: root.getAttribute("data-green-td-resume-rect"),
        campaignRect: root.getAttribute("data-green-td-campaign-rect"),
      };
    })()`,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(`reading Campaign runtime QA markers failed: ${JSON.stringify(result.exceptionDetails)}`);
  }
  return result.result.value;
}

async function waitForScreen(cdp, wanted, timeoutMs) {
  const started = Date.now();
  let last = null;
  while (Date.now() - started < timeoutMs) {
    const state = await cdp.send("Runtime.evaluate", {
      expression: `(() => {
        const boot = document.getElementById('boot');
        return {
          screen: document.documentElement.getAttribute('data-green-td-screen'),
          ready: document.readyState,
          bootDisplay: boot ? getComputedStyle(boot).display : null,
          bootText: boot ? boot.innerText.slice(0, 400) : null,
          canvas: document.querySelector('canvas') ?
            [document.querySelector('canvas').width, document.querySelector('canvas').height] : null,
        };
      })()`,
      returnByValue: true,
    });
    // During an Edge navigation the isolated execution context may briefly
    // disappear. Treat that as not-ready and keep the bounded wait running;
    // dereferencing the absent value used to turn an ordinary navigation race
    // into an unhelpful TypeError before the app had its 15 second boot window.
    last = state.result?.value ?? { screen: null, evaluating: state };
    if (last.screen === wanted) return;
    await sleep(250);
  }
  throw new Error(
    `Edge did not reach the ${wanted} screen in ${timeoutMs} ms: ${JSON.stringify(last)}`
  );
}

// Chromium may give a headless phone a larger minimum CSS viewport than the
// requested --window-size.  The game publishes the real egui card bounds, so
// use that rather than guessing a title-button y coordinate from CLI input.
async function waitForResumeRect(cdp, timeoutMs) {
  const started = Date.now();
  let last = null;
  while (Date.now() - started < timeoutMs) {
    last = await runtimeQa(cdp);
    const parts = String(last.resumeRect ?? "").split(",").map(Number);
    if (parts.length === 4 && parts.every(Number.isFinite) && parts[2] > 2 && parts[3] > 2) {
      return parts;
    }
    await sleep(100);
  }
  throw new Error(`title did not publish a usable Continue card: ${JSON.stringify(last)}`);
}

async function waitForCampaignRect(cdp, timeoutMs) {
  const started = Date.now();
  let last = null;
  while (Date.now() - started < timeoutMs) {
    last = await runtimeQa(cdp);
    const parts = String(last.campaignRect ?? "").split(",").map(Number);
    if (parts.length === 4 && parts.every(Number.isFinite) && parts[2] > 2 && parts[3] > 2) {
      return parts;
    }
    await sleep(100);
  }
  throw new Error(`title did not publish a usable Campaign card: ${JSON.stringify(last)}`);
}

function commandRects(runtime) {
  const result = new Map();
  for (const entry of String(runtime.commands ?? "").split(";")) {
    const [name, raw] = entry.split(":");
    const rect = String(raw ?? "").split(",").map(Number);
    if (name && rect.length === 4 && rect.every(Number.isFinite)) result.set(name, rect);
  }
  return result;
}

async function clickCommand(cdp, runtime, name) {
  const rect = commandRects(runtime).get(name);
  if (!rect || rect[2] < 2 || rect[3] < 2) {
    throw new Error(`missing visible ${name} command: ${JSON.stringify(runtime)}`);
  }
  await click(cdp, rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5);
}

async function waitForRuntimePhase(cdp, wanted, timeoutMs) {
  const started = Date.now();
  let last = null;
  while (Date.now() - started < timeoutMs) {
    last = await runtimeQa(cdp);
    if (last.phase === wanted) return last;
    await sleep(100);
  }
  throw new Error(`game did not reach ${wanted} automatically in ${timeoutMs} ms: ${JSON.stringify(last)}`);
}

// The fixture's own visible banner and DOM provenance both have to agree with
// the live simulation state. This rejects a build that merely changes the HUD
// denominator or skips a boundary before there is a physical target horde.
async function waitForCampaignDiagnostic(cdp, start, target, timeoutMs) {
  const wanted = `E${start}->E${target}:reached`;
  const began = Date.now();
  let last = null;
  while (Date.now() - began < timeoutMs) {
    last = await runtimeQa(cdp);
    if (last.diagnostic === wanted && Number(last.encounter) === target &&
        last.phase === "Combat" && last.speed === 100 && last.creeps > 0) {
      return last;
    }
    await sleep(80);
  }
  throw new Error(`Campaign diagnostic did not reach a live E${target}: ${JSON.stringify(last)}`);
}

async function analyseScreenshot(cdp, png, crop = null) {
  const source = JSON.stringify(`data:image/png;base64,${png}`);
  const cropSource = JSON.stringify(crop);
  const result = await cdp.send("Runtime.evaluate", {
    expression: `(async () => {
      const image = new Image();
      image.src = ${source};
      await image.decode();
      const crop = ${cropSource};
      const sx = crop ? Math.round(image.width * crop.x) : 0;
      const sy = crop ? Math.round(image.height * crop.y) : 0;
      const sw = crop ? Math.round(image.width * crop.width) : image.width;
      const sh = crop ? Math.round(image.height * crop.height) : image.height;
      const scale = Math.min(1, 360 / sw);
      const canvas = document.createElement('canvas');
      canvas.width = Math.max(1, Math.round(sw * scale));
      canvas.height = Math.max(1, Math.round(sh * scale));
      const context = canvas.getContext('2d', { willReadFrequently: true });
      context.drawImage(image, sx, sy, sw, sh, 0, 0, canvas.width, canvas.height);
      const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
      let min = 255;
      let max = 0;
      let sum = 0;
      let lit = 0;
      const histogram = Array(16).fill(0);
      for (let i = 0; i < pixels.length; i += 4) {
        const luminance = Math.round(
          pixels[i] * 0.2126 + pixels[i + 1] * 0.7152 + pixels[i + 2] * 0.0722
        );
        min = Math.min(min, luminance);
        max = Math.max(max, luminance);
        sum += luminance;
        if (luminance >= 16) lit += 1;
        histogram[Math.min(15, luminance >> 4)] += 1;
      }
      const total = pixels.length / 4;
      const activeBands = histogram.filter(count => count >= total * 0.002).length;
      return {
        min, max, range: max - min, mean: sum / total,
        litRatio: lit / total, activeBands,
      };
    })()`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(`screenshot analysis failed: ${JSON.stringify(result.exceptionDetails)}`);
  }
  return result.result.value;
}

let failed = false;
try {
  const cdp = new Cdp(await endpoint());
  await cdp.open();
  await Promise.all([
    cdp.send("Page.enable"),
    cdp.send("Runtime.enable"),
    cdp.send("Log.enable"),
  ]);
  await cdp.send("Page.bringToFront");
  await cdp.send("Emulation.setFocusEmulationEnabled", { enabled: true });
  if (forceWebGl2) {
    // Installed before page navigation in this isolated temporary profile.
    // Chromium accepted --disable-webgpu but still exposed navigator.gpu, so
    // the normal eframe adapter chooser selected BrowserWebGpu. Removing only
    // that capability makes the same chooser take its WebGL2 fallback; the
    // later runtime `backend === "Gl"` assertion remains the actual proof.
    await cdp.send("Page.addScriptToEvaluateOnNewDocument", {
      source: `(() => {
        try { Object.defineProperty(Navigator.prototype, "gpu", { configurable: true, get: () => undefined }); } catch (_) {}
        try { Object.defineProperty(navigator, "gpu", { configurable: true, value: undefined }); } catch (_) {}
      })();`,
    });
  }
  // Headless Chromium enforces a desktop-sized CSS viewport from --window-size
  // on some hosts.  Override the device metrics before navigation so a 390px
  // mobile test is genuinely 390 CSS pixels, not a scaled 496px desktop page.
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    width: VIEWPORT_W,
    height: VIEWPORT_H,
    deviceScaleFactor: 1,
    mobile: VIEWPORT_W <= 600,
  });
  await cdp.send("Page.navigate", { url: loadedUrl });
  // The optimized bundle is large enough that a cold browser can spend more
  // than twelve seconds compiling WASM. Wait for the app's actual title frame,
  // not a guessed timeout that may still be looking at the boot splash.
  await waitForScreen(cdp, "title", 15_000);
  await sleep(700);
  const before = await screenshot(cdp, shotBefore);
  const menuPixels = await analyseScreenshot(cdp, before);
  await cdp.send("Runtime.evaluate", {
    expression: `window.__tdPointerQa = [];
      for (const name of ['pointerdown', 'pointerup', 'mousedown', 'mouseup', 'click']) {
        document.addEventListener(name, e => window.__tdPointerQa.push(
          [name, Math.round(e.clientX), Math.round(e.clientY), e.button]
        ), true);
      }`,
  });

  // Click the live egui Campaign card rather than assuming a desktop title
  // stack fits a 390px phone viewport.
  const [campaignX, campaignY, campaignW, campaignH] = await waitForCampaignRect(cdp, 3_000);
  await click(cdp, campaignX + campaignW * 0.5, campaignY + campaignH * 0.5);
  await waitForScreen(cdp, "playing", 6_000);
  if (diagnosticFixture > 0) {
    const target = diagnosticFixture === 35 ? 37 : diagnosticFixture === 60 ? 61 : 600;
    const live = await waitForCampaignDiagnostic(cdp, diagnosticFixture, target, 15_000);
    // Let the egui overlay settle for one rendered frame after the app pauses
    // its pilot. The capture is still bounded and the game is already paused.
    await sleep(180);
    const targetShot = await screenshot(cdp, shotAfter);
    const targetPixels = await analyseScreenshot(cdp, targetShot);
    const boardPixels = await analyseScreenshot(cdp, targetShot, {
      x: 0.04,
      y: 0.10,
      width: 0.92,
      height: 0.58,
    });
    const fixtureState = await cdp.send("Runtime.evaluate", {
      expression: `(() => {
        const root = document.documentElement;
        const canvas = document.querySelector('canvas');
        return {
          title: document.title,
          canvas: canvas ? [canvas.width, canvas.height] : null,
          build: root.getAttribute('data-green-td-build'),
          screen: root.getAttribute('data-green-td-screen'),
          mode: root.getAttribute('data-green-td-mode'),
          encounter: root.getAttribute('data-green-td-encounter'),
          phase: root.getAttribute('data-green-td-phase'),
          speed: Number(root.getAttribute('data-green-td-speed') || 0),
          creeps: Number(root.getAttribute('data-green-td-creeps') || 0),
          diagnostic: root.getAttribute('data-green-td-diagnostic'),
          wasm: performance.getEntriesByType('resource').map(e => e.name)
            .filter(name => name.endsWith('.wasm')),
          text: document.body.innerText.slice(0, 1200),
        };
      })()`,
      returnByValue: true,
    });
    const errors = cdp.events
      .filter((event) =>
        event.method === "Runtime.exceptionThrown" ||
        (event.method === "Log.entryAdded" && event.params.entry.level === "error") ||
        (event.method === "Runtime.consoleAPICalled" && event.params.type === "error"),
      )
      .map((event) => JSON.stringify(event.params));
    const diagnostics = cdp.events
      .filter((event) => event.method === "Runtime.consoleAPICalled" || event.method === "Log.entryAdded")
      .map((event) => JSON.stringify(event.params));
    const saved = await campaignSave(cdp);
    const varied = (pixels) => pixels.range >= 32 && pixels.litRatio >= 0.05 && pixels.activeBands >= 3;
    const fixtureValue = fixtureState.result.value;
    console.log(JSON.stringify({
      diagnostic: { start: diagnosticFixture, target, live, saveSuppressed: !saved.exists },
      state: fixtureValue,
      screenshots: [shotBefore, shotAfter],
      pixels: { menu: menuPixels, target: targetPixels, board: boardPixels },
      persistence: saved,
      errors,
      diagnostics,
      enteredGame: fixtureValue.screen === "playing",
      meaningfulGameplay: varied(targetPixels) && varied(boardPixels),
    }, null, 2));
    failed = errors.length > 0 || !fixtureValue.canvas || fixtureValue.screen !== "playing" ||
      fixtureValue.mode !== "Campaign" || Number(fixtureValue.encounter) !== target ||
      fixtureValue.phase !== "Combat" || fixtureValue.speed !== 100 || fixtureValue.creeps <= 0 ||
      fixtureValue.diagnostic !== `E${diagnosticFixture}->E${target}:reached` ||
      saved.exists || !varied(targetPixels) || !varied(boardPixels);
    cdp.ws.close();
  } else {
  // Campaign opens at readable 10x, then offers only rapid values through the
  // requested 100x max. Freeze first so the speed check cannot accidentally
  // turn a real opening horde into an unobserved simulation skip.
  const tempoAtStart = await runtimeQa(cdp);
  if (tempoAtStart.speed !== 10) {
    throw new Error(`Campaign did not open at 10x: ${JSON.stringify(tempoAtStart)}`);
  }
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown", key: " ", code: "Space", windowsVirtualKeyCode: 32,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp", key: " ", code: "Space", windowsVirtualKeyCode: 32,
  });
  await sleep(140);
  const rapidSteps = [];
  for (const expected of [25, 50, 100]) {
    await cdp.send("Input.dispatchKeyEvent", {
      type: "keyDown", key: "f", code: "KeyF", windowsVirtualKeyCode: 70,
    });
    await cdp.send("Input.dispatchKeyEvent", {
      type: "keyUp", key: "f", code: "KeyF", windowsVirtualKeyCode: 70,
    });
    await sleep(100);
    const state = await runtimeQa(cdp);
    rapidSteps.push(state.speed);
    if (state.speed !== expected) {
      throw new Error(`Campaign rapid tempo missed ${expected}x: ${JSON.stringify({ expected, state })}`);
    }
  }
  // One more press returns to the required 10x default, so the actual opening
  // and subsequent input evidence run at normal Campaign tempo.
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: "f",
    code: "KeyF",
    windowsVirtualKeyCode: 70,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: "f",
    code: "KeyF",
    windowsVirtualKeyCode: 70,
  });
  await sleep(180);
  const resetTempo = await runtimeQa(cdp);
  if (resetTempo.speed !== 10) {
    throw new Error(`Campaign rapid tempo did not return to 10x: ${JSON.stringify(resetTempo)}`);
  }
  // Keyboard shortcuts are useful, but the advertised 100x mode must also be
  // a visible, reachable touch/pointer control.  Press the actual egui tempo
  // button through Chromium and walk the same rapid-only ladder.
  const speedRect = (state) => {
    const values = String(state.speedControl ?? "").split(",").map(Number);
    if (values.length !== 4 || !values.every(Number.isFinite) || values[2] < 20 || values[3] < 20) {
      throw new Error(`visible rapid tempo control is missing: ${JSON.stringify(state)}`);
    }
    return values;
  };
  let liveSpeedRect = speedRect(resetTempo);
  const pointerRapidSteps = [];
  for (const expected of [25, 50, 100, 10]) {
    // The label grows from 10x to 100x, so egui correctly reflows the
    // right-aligned compact strip between presses. Re-read its real rectangle
    // instead of accidentally clicking a stale coordinate on the last step.
    const [speedX, speedY, speedW, speedH] = liveSpeedRect;
    await click(cdp, speedX + speedW * 0.5, speedY + speedH * 0.5);
    await sleep(110);
    const state = await runtimeQa(cdp);
    pointerRapidSteps.push(state.speed);
    if (state.speed !== expected) {
      throw new Error(`visible rapid tempo control missed ${expected}x: ${JSON.stringify({ expected, state })}`);
    }
    liveSpeedRect = speedRect(state);
  }
  // Resume for the real automatic deployment.
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown", key: " ", code: "Space", windowsVirtualKeyCode: 32,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp", key: " ", code: "Space", windowsVirtualKeyCode: 32,
  });
  // Campaign is brisk by default: its authored opening begins on its own
  // after a short real-time build beat. Do not press Enter here; waiting for
  // the live game state proves that automatic deployment rather than a test
  // keystroke started the actual first packet.
  const autoDeployment = await waitForRuntimePhase(cdp, "Combat", 5_000);
  await sleep(450);
  // Freeze the simulation while exercising the real bottom-right icon grid.
  // That makes the exact one-time purchase check deterministic instead of
  // mistaking a concurrent kill reward for a palette-side gold mutation.
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: " ",
    code: "Space",
    windowsVirtualKeyCode: 32,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: " ",
    code: "Space",
    windowsVirtualKeyCode: 32,
  });
  await sleep(180);
  const runtimeBeforeInput = await runtimeQa(cdp);
  const point = (raw, label) => {
    const values = String(raw ?? "").split(",").map(Number);
    if (values.length !== 2 || !values.every(Number.isFinite)) {
      throw new Error(`missing ${label}: ${JSON.stringify(runtimeBeforeInput)}`);
    }
    return values;
  };
  const rect = (raw, label) => {
    const values = String(raw ?? "").split(",").map(Number);
    if (values.length !== 4 || !values.every(Number.isFinite)) {
      throw new Error(`missing ${label}: ${JSON.stringify(runtimeBeforeInput)}`);
    }
    return values;
  };
  const boardRect = rect(runtimeBeforeInput.board, "live board rectangle");
  const paletteRect = rect(runtimeBeforeInput.palette, "bottom-right palette rectangle");
  const cardRects = String(runtimeBeforeInput.cards ?? "")
    .split(";")
    .filter(Boolean)
    .map((raw) => rect(raw, "tower icon rectangle"));
  if (!cardRects.length || !String(runtimeBeforeInput.cardDefs ?? "")) {
    throw new Error(`bottom-right tower icon grid is empty: ${JSON.stringify(runtimeBeforeInput)}`);
  }
  if (VIEWPORT_W / VIEWPORT_H > 1.25 && boardRect[2] <= boardRect[3] * 1.25) {
    throw new Error(`wide viewport still uses a square/letterboxed scene: ${JSON.stringify({ viewport: [VIEWPORT_W, VIEWPORT_H], boardRect })}`);
  }
  // The command grid must be physically below the square world rather than a
  // tall side catalogue or an overlay that steals buildable battlefield space.
  if (paletteRect[1] + 0.5 < boardRect[1] + boardRect[3]) {
    throw new Error(`tower palette is not below the board: ${JSON.stringify({ boardRect, paletteRect })}`);
  }
  const [firstCardX, firstCardY, firstCardW, firstCardH] = cardRects[0];
  await click(cdp, firstCardX + firstCardW * 0.5, firstCardY + firstCardH * 0.5);
  await sleep(220);
  const armed = await runtimeQa(cdp);
  if (!armed.armedTower || armed.armedCost <= 0) {
    throw new Error(`bottom-right tower icon did not arm a real build choice: ${JSON.stringify({ runtimeBeforeInput, armed })}`);
  }
  if (armed.gold !== runtimeBeforeInput.gold) {
    throw new Error(`arming a tower icon spent gold before placement: ${JSON.stringify({ runtimeBeforeInput, armed })}`);
  }
  const armedShot = await screenshot(cdp, shotArmed);
  const armedPixels = await analyseScreenshot(cdp, armedShot);
  // At the exact phone width the compact ellipsis must expose the safety
  // controls as actual buttons.  These clicks use the live egui bounds the
  // app publishes for QA; they are ordinary pointer input, not keyboard
  // substitutes. Menu is left visible (rather than ending this active save)
  // after its hitbox has been verified within the CSS viewport.
  let touchOverflow = null;
  if (VIEWPORT_W <= 600 || commandRects(armed).has("more")) {
    await clickCommand(cdp, armed, "more");
    await sleep(160);
    let overflow = await runtimeQa(cdp);
    const overflowRects = commandRects(overflow);
    for (const name of ["pause", "view", "menu", "cancel", "more"]) {
      const rect = overflowRects.get(name);
      if (!rect || rect[0] < 0 || rect[1] < 0 || rect[0] + rect[2] > VIEWPORT_W + 0.5 ||
          rect[1] + rect[3] > VIEWPORT_H + 0.5) {
        throw new Error(`compact overflow ${name} is not reachable: ${JSON.stringify({ overflow, rect })}`);
      }
    }
    const pausedBefore = overflow.paused;
    await clickCommand(cdp, overflow, "pause");
    await sleep(120);
    overflow = await runtimeQa(cdp);
    if (overflow.paused === pausedBefore) {
      throw new Error(`compact Pause did not toggle live simulation: ${JSON.stringify(overflow)}`);
    }
    await clickCommand(cdp, overflow, "pause");
    await sleep(120);
    overflow = await runtimeQa(cdp);
    await clickCommand(cdp, overflow, "view");
    await sleep(120);
    overflow = await runtimeQa(cdp);
    await clickCommand(cdp, overflow, "cancel");
    await sleep(120);
    const cancelledByTouch = await runtimeQa(cdp);
    if (cancelledByTouch.armedTower) {
      throw new Error(`compact Cancel did not clear repeat-build: ${JSON.stringify(cancelledByTouch)}`);
    }
    touchOverflow = { controls: [...overflowRects.keys()], pausedToggled: true, cancelWorked: true };
    // Restore normal compact controls, then arm the same real palette card
    // for the ordinary placement/pinch path below.
    await clickCommand(cdp, cancelledByTouch, "more");
    await sleep(120);
    await click(cdp, firstCardX + firstCardW * 0.5, firstCardY + firstCardH * 0.5);
    await sleep(160);
    const rearmed = await runtimeQa(cdp);
    if (!rearmed.armedTower) throw new Error(`could not re-arm after compact Cancel: ${JSON.stringify(rearmed)}`);
  }
  // First click a known route point. It is intentionally an invalid build
  // location, so it must keep the ghost armed and leave gold/tower count alone.
  const invalidProbe = point(armed.invalidProbe, "invalid route probe");
  await click(cdp, invalidProbe[0], invalidProbe[1]);
  await sleep(180);
  const afterInvalid = await runtimeQa(cdp);
  if (afterInvalid.towers !== armed.towers || afterInvalid.gold !== armed.gold || afterInvalid.armedTower !== armed.armedTower) {
    throw new Error(`invalid placement changed Campaign state: ${JSON.stringify({ armed, afterInvalid })}`);
  }
  // Then exercise a genuine board placement. The app exposes a read-only legal
  // socket generated from its live camera/board rectangle; this click still
  // travels through ordinary browser pointer and picking code.
  const probe = point(afterInvalid.inputProbe, "live board input probe");
  await click(cdp, probe[0], probe[1]);
  await sleep(300);
  const runtimeAfterInput = await runtimeQa(cdp);
  if (runtimeAfterInput.towers <= runtimeBeforeInput.towers) {
    throw new Error(`bottom-right icon did not build at its live probe: ${JSON.stringify({ runtimeBeforeInput, armed, runtimeAfterInput })}`);
  }
  if (runtimeAfterInput.lastTower !== armed.armedTower || runtimeAfterInput.gold !== armed.gold - armed.armedCost) {
    throw new Error(`tower icon did not build the selected tower exactly once: ${JSON.stringify({ armed, runtimeAfterInput })}`);
  }
  // Repeat-build remains armed until the player explicitly cancels it.
  if (runtimeAfterInput.armedTower !== armed.armedTower) {
    throw new Error(`successful placement unexpectedly cancelled repeat-build: ${JSON.stringify({ armed, runtimeAfterInput })}`);
  }
  // Zoom toward the live grass probe, place through the zoomed camera, then
  // middle-pan and reset. Every assertion reads real world/picking markers;
  // no DOM-only transform is being mistaken for camera support.
  const cameraBeforeZoom = String(runtimeAfterInput.camera ?? "");
  const zoomProbe = point(runtimeAfterInput.inputProbe, "zoomable free-grass probe");
  await wheel(cdp, zoomProbe[0], zoomProbe[1], -260);
  await sleep(220);
  const zoomed = await runtimeQa(cdp);
  if (!zoomed.camera || zoomed.camera === cameraBeforeZoom) {
    throw new Error(`mouse-wheel did not change the live camera: ${JSON.stringify({ runtimeAfterInput, zoomed })}`);
  }
  const zoomedProbe = point(zoomed.inputProbe, "zoomed free-grass probe");
  if (zoomedProbe[0] < boardRect[0] || zoomedProbe[0] > boardRect[0] + boardRect[2] ||
      zoomedProbe[1] < boardRect[1] || zoomedProbe[1] > boardRect[1] + boardRect[3]) {
    throw new Error(`zoomed pick probe fell outside the live scene: ${JSON.stringify({ boardRect, zoomed, zoomedProbe })}`);
  }
  await click(cdp, zoomedProbe[0], zoomedProbe[1]);
  await sleep(220);
  const afterZoomedBuild = await runtimeQa(cdp);
  if (afterZoomedBuild.towers !== runtimeAfterInput.towers + 1 ||
      afterZoomedBuild.gold !== runtimeAfterInput.gold - armed.armedCost) {
    throw new Error(`zoomed free-grass click did not purchase exactly one tower: ${JSON.stringify({ runtimeAfterInput, zoomed, afterZoomedBuild })}`);
  }
  const zoomedShot = await screenshot(cdp, shotZoomed);
  const zoomedPixels = await analyseScreenshot(cdp, zoomedShot);
  // A genuine two-finger pinch must alter the camera but never turn either
  // touch release into a tower placement, even while repeat-build is armed.
  const cameraBeforePinch = String(afterZoomedBuild.camera ?? "");
  await touchPinch(cdp,
    [boardRect[0] + boardRect[2] * 0.50, boardRect[1] + boardRect[3] * 0.50],
    Math.min(34, boardRect[2] * 0.04), Math.min(88, boardRect[2] * 0.10));
  await sleep(220);
  const pinched = await runtimeQa(cdp);
  if (!pinched.camera || pinched.camera === cameraBeforePinch ||
      pinched.towers !== afterZoomedBuild.towers || pinched.gold !== afterZoomedBuild.gold) {
    throw new Error(`touch pinch did not navigate cleanly: ${JSON.stringify({ afterZoomedBuild, pinched })}`);
  }
  const cameraBeforePan = String(pinched.camera ?? "");
  await middleDrag(cdp, [boardRect[0] + boardRect[2] * 0.54, boardRect[1] + boardRect[3] * 0.52],
    [boardRect[0] + boardRect[2] * 0.66, boardRect[1] + boardRect[3] * 0.58]);
  await sleep(180);
  const panned = await runtimeQa(cdp);
  if (!panned.camera || panned.camera === cameraBeforePan || panned.towers !== pinched.towers) {
    throw new Error(`bounded middle-pan changed no camera or altered towers: ${JSON.stringify({ pinched, panned })}`);
  }
  // Desktop exposes a visible reset command. The compact phone palette has no
  // spare command slot, so it uses the documented Escape fallback instead;
  // both paths operate the real camera and retain the placed towers.
  const viewValues = String(panned.view ?? "").split(",").map(Number);
  if (viewValues.length === 4 && viewValues.every(Number.isFinite)) {
    const [viewX, viewY, viewW, viewH] = viewValues;
    await click(cdp, viewX + viewW * 0.5, viewY + viewH * 0.5);
    await sleep(180);
  } else if (commandRects(panned).has("more")) {
    // Compactness is driven by both width and short height. A 844x390
    // landscape viewport therefore has the same real overflow as a phone;
    // open it and use View rather than falsely requiring a desktop button.
    await clickCommand(cdp, panned, "more");
    await sleep(120);
    const overflowForView = await runtimeQa(cdp);
    await clickCommand(cdp, overflowForView, "view");
    await sleep(180);
  } else if (VIEWPORT_W > 600) {
    throw new Error(`missing visible reset-view command: ${JSON.stringify(panned)}`);
  }
  const resetView = await runtimeQa(cdp);
  if (!resetView.camera || resetView.towers !== pinched.towers) {
    throw new Error(`reset overview path changed built towers: ${JSON.stringify({ panned, resetView })}`);
  }
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27,
  });
  await sleep(180);
  const afterCancel = await runtimeQa(cdp);
  if (afterCancel.armedTower) {
    throw new Error(`Escape did not cancel repeat-build: ${JSON.stringify(afterCancel)}`);
  }
  // Resume the actual horde before its captured continuous-play observation.
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown", key: " ", code: "Space", windowsVirtualKeyCode: 32,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp", key: " ", code: "Space", windowsVirtualKeyCode: 32,
  });
  await sleep(180);
  // A capture-only quality switch makes the shadow diagnostic reproducible:
  // one B press from an Ultra auto-selected desktop requests Performance,
  // whose shader has SHADOW_TAPS=0. It does not alter simulation state.
  for (let cycle = 0; cycle < qualityCycles; cycle += 1) {
    await cdp.send("Input.dispatchKeyEvent", {
      type: "keyDown", key: "b", code: "KeyB", windowsVirtualKeyCode: 66,
    });
    await cdp.send("Input.dispatchKeyEvent", {
      type: "keyUp", key: "b", code: "KeyB", windowsVirtualKeyCode: 66,
    });
    await sleep(160);
  }
  if (captureDelayMs > 0) {
    await sleep(captureDelayMs);
  }
  const after = await screenshot(cdp, shotAfter);
  const continuousRuntime = await runtimeQa(cdp);
  if (expectMinEncounter > 0 && Number(continuousRuntime.encounter) < expectMinEncounter) {
    throw new Error(
      `continuous Campaign did not reach encounter ${expectMinEncounter}: ${JSON.stringify(continuousRuntime)}`
    );
  }
  const savedBeforeResume = await campaignSave(cdp);
  let resume = null;
  if (verifyResume) {
    if (!savedBeforeResume.exists || savedBeforeResume.mode !== 1) {
      throw new Error(`Campaign did not write a resumable save: ${JSON.stringify(savedBeforeResume)}`);
    }
    // Reload the same isolated profile. This verifies the browser persistence
    // envelope and the real Continue action rather than a Rust-only round trip.
    await cdp.send("Page.reload", { ignoreCache: true });
    await waitForScreen(cdp, "title", 15_000);
    await sleep(500);
    const resumeMenu = await screenshot(cdp, shotResumeMenu);
    const [resumeX, resumeY, resumeW, resumeH] = await waitForResumeRect(cdp, 3_000);
    await click(cdp, resumeX + resumeW * 0.5, resumeY + resumeH * 0.5);
    await waitForScreen(cdp, "playing", 6_000);
    const resumedLiveBefore = await runtimeQa(cdp);
    // Continue must run the restored simulation, not merely redraw the last
    // localStorage blob. Observe a live interval, then force the new runtime
    // state through the real F7 persistence path before comparing saves.
    await sleep(1_100);
    const resumedLiveAfter = await runtimeQa(cdp);
    await cdp.send("Input.dispatchKeyEvent", {
      type: "keyDown", key: "F7", code: "F7", windowsVirtualKeyCode: 118,
    });
    await cdp.send("Input.dispatchKeyEvent", {
      type: "keyUp", key: "F7", code: "F7", windowsVirtualKeyCode: 118,
    });
    await sleep(400);
    const resumed = await screenshot(cdp, shotResumed);
    const savedAfterResume = await campaignSave(cdp);
    const activeAdvanced = Number(savedAfterResume.activeSeconds) > Number(savedBeforeResume.activeSeconds) + 0.05;
    const spawnAdvanced = Number(savedAfterResume.spawnedBodies) > Number(savedBeforeResume.spawnedBodies)
      || Number(savedAfterResume.deployedPackets) > Number(savedBeforeResume.deployedPackets)
      || Number(savedAfterResume.queuedBodiesLeft) !== Number(savedBeforeResume.queuedBodiesLeft);
    resume = {
      screenshots: [shotResumeMenu, shotResumed],
      before: savedBeforeResume,
      after: savedAfterResume,
      liveBefore: resumedLiveBefore,
      liveAfter: resumedLiveAfter,
      sameSeed: savedBeforeResume.seed === savedAfterResume.seed,
      sameCampaign: savedAfterResume.mode === 1 && savedAfterResume.encounter === savedBeforeResume.encounter,
      sameTowerPositions: JSON.stringify(savedAfterResume.towerPositions) === JSON.stringify(savedBeforeResume.towerPositions),
      resumedCombat: savedAfterResume.phase === 1,
      activeAdvanced,
      spawnAdvanced,
    };
    if (!Array.isArray(savedBeforeResume.towerPositions) ||
        savedBeforeResume.towerPositions.length < 2 ||
        savedBeforeResume.towerPositions.some(tower => !Array.isArray(tower.pos) || tower.pos.length !== 2) ||
        !resume.sameSeed || !resume.sameCampaign || !resume.sameTowerPositions || !resume.resumedCombat ||
        !resume.activeAdvanced || !resume.spawnAdvanced) {
      throw new Error(`Campaign resume changed run identity: ${JSON.stringify(resume)}`);
    }
    // Make the later screenshot-analysis report prove both game frames are
    // real image data, not a stale title or black renderer.
    resume.pixels = {
      menu: await analyseScreenshot(cdp, resumeMenu),
      gameplay: await analyseScreenshot(cdp, resumed),
    };
  }
  const gamePixels = await analyseScreenshot(cdp, after);
  // Keep the renderer honest independently of egui. A bright HUD can make a
  // screenshot look varied even when the custom WebGPU board callback itself
  // is a black rectangle, so sample only the central battlefield as well.
  const boardPixels = await analyseScreenshot(cdp, after, {
    x: 0.04,
    y: 0.10,
    width: 0.92,
    height: 0.58,
  });

  const state = await cdp.send("Runtime.evaluate", {
    expression: `(() => {
      const canvas = document.querySelector('canvas');
      return {
        title: document.title,
        canvas: canvas ? [canvas.width, canvas.height] : null,
        rect: canvas ? [canvas.getBoundingClientRect().x, canvas.getBoundingClientRect().y,
          canvas.getBoundingClientRect().width, canvas.getBoundingClientRect().height] : null,
        dpr: devicePixelRatio,
        build: document.documentElement.getAttribute('data-green-td-build'),
        mode: document.documentElement.getAttribute('data-green-td-mode'),
        encounter: document.documentElement.getAttribute('data-green-td-encounter'),
        phase: document.documentElement.getAttribute('data-green-td-phase'),
        speed: Number(document.documentElement.getAttribute('data-green-td-speed') || 0),
        board: document.documentElement.getAttribute('data-green-td-board-css'),
        inputProbe: document.documentElement.getAttribute('data-green-td-input-probe'),
        viewport: [innerWidth, innerHeight, visualViewport?.width ?? null, visualViewport?.height ?? null],
        wasm: performance.getEntriesByType('resource').map(e => e.name)
          .filter(name => name.endsWith('.wasm')),
        screen: document.documentElement.getAttribute('data-green-td-screen'),
        backend: document.documentElement.getAttribute('data-green-td-backend'),
        pointerQa: window.__tdPointerQa,
        text: document.body.innerText.slice(0, 1200),
      };
    })()`,
    returnByValue: true,
  });

  const errors = cdp.events
    .filter((event) =>
      event.method === "Runtime.exceptionThrown" ||
      (event.method === "Log.entryAdded" && event.params.entry.level === "error") ||
      (event.method === "Runtime.consoleAPICalled" && event.params.type === "error"),
    )
    .map((event) => JSON.stringify(event.params));
  // Keep non-error browser/WebGPU diagnostics in the evidence report. WGPU can
  // reject a shader through a warning callback while leaving the Rust app's
  // outer egui frame alive; treating only console errors as evidence would
  // call an all-black battlefield a successful render.
  const diagnostics = cdp.events
    .filter((event) => event.method === "Runtime.consoleAPICalled" || event.method === "Log.entryAdded")
    .map((event) => JSON.stringify(event.params));

  const visuallyVaried = (pixels) => pixels.range >= 32 &&
    pixels.litRatio >= 0.05 && pixels.activeBands >= 3;
  const meaningfulGameplay = visuallyVaried(gamePixels) && visuallyVaried(boardPixels);
  console.log(JSON.stringify({
    state: state.result.value,
    requestedBackend: forceWebGl2 ? "WebGL2 fallback" : "WebGPU preferred",
    qualityCycles,
    screenshots: [shotBefore, shotArmed, shotZoomed, shotAfter, ...(resume?.screenshots ?? [])],
    pixels: { menu: menuPixels, armed: armedPixels, zoomed: zoomedPixels, gameplay: gamePixels, board: boardPixels },
    persistence: { beforeResume: savedBeforeResume, resume },
    input: { tempoAtStart, rapidSteps, resetTempo, autoDeployment, before: runtimeBeforeInput, after: runtimeAfterInput, pinched, continuousRuntime },
    errors,
    diagnostics,
    enteredGame: state.result.value.screen === "playing",
    meaningfulGameplay,
  }, null, 2));
  failed = errors.length > 0 || !state.result.value.canvas ||
    state.result.value.screen !== "playing" || before === after ||
    menuPixels.range < 32 || !meaningfulGameplay ||
    !savedBeforeResume.exists || savedBeforeResume.mode !== 1 ||
    state.result.value.speed !== 10 ||
    (forceWebGl2 && state.result.value.backend !== "Gl");
  cdp.ws.close();
  }
} finally {
  edge.kill();
  await sleep(600);
  const resolvedProfile = resolve(profile);
  const safePrefix = `${tempRoot.toLowerCase()}\\green-td-edge-`;
  if (resolvedProfile.toLowerCase().startsWith(safePrefix)) {
    try {
      rmSync(resolvedProfile, { recursive: true, force: true });
    } catch {
      // Edge can briefly retain its crash-report database after exit. This is
      // an isolated temp directory and the operating system will reclaim it.
    }
  }
}

process.exitCode = failed ? 1 : 0;

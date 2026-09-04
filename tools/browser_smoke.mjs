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

const EDGE = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
const URL = process.argv[2] ?? "http://127.0.0.1:8080";
const PORT = 9338;
const tempRoot = realpathSync(tmpdir());
const profile = mkdtempSync(join(tempRoot, "green-td-edge-"));
const shotBefore = join(tempRoot, "green-td-edge-menu.png");
const shotAfter = join(tempRoot, "green-td-edge-play.png");

const edge = spawn(
  EDGE,
  [
    "--headless=new",
    "--enable-unsafe-webgpu",
    "--disable-gpu-sandbox",
    `--remote-debugging-port=${PORT}`,
    `--user-data-dir=${profile}`,
    "--window-size=1440,1000",
    "about:blank",
  ],
  { stdio: "ignore", windowsHide: true },
);

const sleep = (ms) => new Promise((resolvePromise) => setTimeout(resolvePromise, ms));

async function endpoint() {
  for (let i = 0; i < 80; i += 1) {
    try {
      const tabs = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      const page = tabs.find((tab) => tab.type === "page");
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
    last = state.result.value;
    if (last.screen === wanted) return;
    await sleep(250);
  }
  throw new Error(
    `Edge did not reach the ${wanted} screen in ${timeoutMs} ms: ${JSON.stringify(last)}`
  );
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
  await cdp.send("Page.navigate", { url: URL });
  // The optimized bundle is large enough that a cold browser can spend more
  // than twelve seconds compiling WASM. Wait for the app's actual title frame,
  // not a guessed timeout that may still be looking at the boot splash.
  await waitForScreen(cdp, "title", 45_000);
  await sleep(2_000);
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

  // The isolated profile has no save, so the first large button below the
  // challenge picker is always Start campaign.
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: 720,
    y: 465,
  });
  await sleep(120);
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: 720,
    y: 465,
    button: "left",
    buttons: 1,
    pointerType: "mouse",
    clickCount: 1,
  });
  await sleep(120);
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: 720,
    y: 465,
    button: "left",
    buttons: 0,
    pointerType: "mouse",
    clickCount: 1,
  });
  await waitForScreen(cdp, "playing", 10_000);
  await sleep(4_000);
  const after = await screenshot(cdp, shotAfter);
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
        screen: document.documentElement.getAttribute('data-green-td-screen'),
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

  const visuallyVaried = (pixels) => pixels.range >= 32 &&
    pixels.litRatio >= 0.05 && pixels.activeBands >= 3;
  const meaningfulGameplay = visuallyVaried(gamePixels) && visuallyVaried(boardPixels);
  console.log(JSON.stringify({
    state: state.result.value,
    screenshots: [shotBefore, shotAfter],
    pixels: { menu: menuPixels, gameplay: gamePixels, board: boardPixels },
    errors,
    enteredGame: state.result.value.screen === "playing",
    meaningfulGameplay,
  }, null, 2));
  failed = errors.length > 0 || !state.result.value.canvas ||
    state.result.value.screen !== "playing" || before === after ||
    menuPixels.range < 32 || !meaningfulGameplay;
  cdp.ws.close();
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

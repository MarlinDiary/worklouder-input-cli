#!/usr/bin/env node
import { createHash } from "node:crypto";
import { access } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import {
  InspectorClient,
  assertEqual,
  exactProcessIds,
  inspectorTarget,
  plistValue,
  sha256File,
  unwrapRemoteResult,
} from "./live-bridge-cdp.mjs";

const APP = "/Applications/input.app";
const EXECUTABLE = `${APP}/Contents/MacOS/input`;
const PLIST = `${APP}/Contents/Info.plist`;
const ASAR = `${APP}/Contents/Resources/app.asar`;
const MAIN_SOURCE_SUFFIX =
  "/Applications/input.app/Contents/Resources/app.asar/dist-electron/main/index.js";
const EXPECTED_VERSION = "0.18.0";
const EXPECTED_ASAR_SHA256 =
  "8e530188bc693ca1b9950bdc0515adfc349a3563e1841fe61ff2d692dc6b2da8";
const EXPECTED_MAIN_SHA256 =
  "7c16191956acb8d0d89c50eb940dc9b757bf6966b19c56499ca2d60d8743154a";
// Input 0.18.0: the recurring device-search RPC call. Pausing here exposes
// the released app's module-local service container without restarting Input.
const CAPTURE_LOCATION = { lineNumber: 40, columnNumber: 51665 };
const PORT = 9229;
const OVERLAY_PATH = fileURLToPath(
  new URL("../companion/input-live-overlay-v3.mjs", import.meta.url),
);

const action = process.argv.includes("--remove") ? "remove" : "install";
if (process.argv.includes("--help")) {
  console.log("usage: install-input-live-bridge.mjs [--remove]");
  process.exit(0);
}

await access(EXECUTABLE);
await access(OVERLAY_PATH);
assertEqual(
  await plistValue(PLIST, "CFBundleShortVersionString"),
  EXPECTED_VERSION,
  "Input version",
);
assertEqual(await sha256File(ASAR), EXPECTED_ASAR_SHA256, "Input app.asar SHA-256");
progress("verified Input 0.18.0 release identity");

const pids = await exactProcessIds(EXECUTABLE);
if (pids.length !== 1) {
  throw new Error(`expected one running Input main process, detected ${pids.length}`);
}
const pid = pids[0];

let target;
let openedInspector = false;
try {
  target = await inspectorTarget(PORT, 500);
} catch {
  process.kill(pid, "SIGUSR1");
  openedInspector = true;
  target = await inspectorTarget(PORT);
}

const client = await InspectorClient.connect(target.webSocketDebuggerUrl);
let closeScheduled = false;
try {
  const actualExecutable = await client.evaluate("process.execPath");
  assertEqual(actualExecutable, EXECUTABLE, "inspector process executable");
  progress(`attached to running Input main process pid=${pid}`);

  if (action === "remove") {
    const result = await client.evaluate(
      `(async()=>{const current=globalThis.__worklouderctlInputBridge;if(!current)return {removed:false};await current.stop();delete globalThis.__worklouderctlInputBridge;delete globalThis.__worklouderctlInputCapture;return {removed:true}})()`,
    );
    closeScheduled = await scheduleInspectorClose(client);
    console.log(JSON.stringify({ provider: "input", action, pid, ...result }, null, 2));
    process.exitCode = 0;
  } else {
    const modulePath = JSON.stringify(OVERLAY_PATH);
    const state = await client.evaluate(
      `(()=>{const require=process.getBuiltinModule("module").createRequire("/tmp/worklouderctl-input-live.cjs");const overlay=require(${modulePath});if(!Number.isInteger(overlay.INPUT_LIVE_OVERLAY_REVISION))throw new Error("Input overlay revision missing");const current=globalThis.__worklouderctlInputBridge;return {targetRevision:overlay.INPUT_LIVE_OVERLAY_REVISION,currentRevision:current?.overlayRevision??null,installed:!!current,captured:!!globalThis.__worklouderctlInputCapture}})()`,
    );
    if (state.installed && state.currentRevision === state.targetRevision) {
      const alreadyInstalled = await client.evaluate(
        `(()=>{const current=globalThis.__worklouderctlInputBridge,capture=globalThis.__worklouderctlInputCapture;return {installed:true,idempotent:true,overlayRevision:current.overlayRevision,version:capture.app.getVersion(),socketPath:current.socketPath,tokenPath:current.tokenPath}})()`,
      );
      closeScheduled = await scheduleInspectorClose(client);
      console.log(
        JSON.stringify({ provider: "input", action, pid, ...alreadyInstalled }, null, 2),
      );
    } else {
      if (state.installed) {
        await client.evaluate(
          `(async()=>{await globalThis.__worklouderctlInputBridge.stop();delete globalThis.__worklouderctlInputBridge;return true})()`,
        );
      }
      if (!state.captured) {
        const capture = await captureReleasedServices(client);
        progress(`captured released service container (${capture.serviceCount} own fields)`);
      }
      const result = await client.evaluate(
        `(async()=>{const capture=globalThis.__worklouderctlInputCapture;if(!capture?.app||!capture?.services)throw new Error("Input service capture missing");const require=process.getBuiltinModule("module").createRequire("/tmp/worklouderctl-input-live.cjs");const overlay=require(${modulePath});const bridge=await overlay.installInputLiveOverlay({app:capture.app,services:capture.services});globalThis.__worklouderctlInputBridge=bridge;return {installed:true,idempotent:false,overlayRevision:bridge.overlayRevision,version:capture.app.getVersion(),socketPath:bridge.socketPath,tokenPath:bridge.tokenPath}})()`,
      );
      progress("installed authenticated bridge without restarting Input");
      closeScheduled = await scheduleInspectorClose(client);
      console.log(JSON.stringify({ provider: "input", action, pid, ...result }, null, 2));
    }
  }
} finally {
  client.close();
  if (openedInspector && !closeScheduled) {
    process.stderr.write(
      "[input-live-bridge] inspector remains open after an interrupted installation\n",
    );
  }
}

async function captureReleasedServices(client) {
  await client.command("Debugger.enable");
  const script = await client.waitForEvent(
    "Debugger.scriptParsed",
    (params) => params.url.endsWith(MAIN_SOURCE_SUFFIX),
    5_000,
  );
  const sourceResult = await client.command("Debugger.getScriptSource", {
    scriptId: script.scriptId,
  });
  assertEqual(
    createHash("sha256").update(sourceResult.scriptSource).digest("hex"),
    EXPECTED_MAIN_SHA256,
    "Input loaded main source SHA-256",
  );
  const possible = await client.command("Debugger.getPossibleBreakpoints", {
    start: {
      scriptId: script.scriptId,
      lineNumber: CAPTURE_LOCATION.lineNumber,
      columnNumber: CAPTURE_LOCATION.columnNumber,
    },
    end: {
      scriptId: script.scriptId,
      lineNumber: CAPTURE_LOCATION.lineNumber,
      columnNumber: CAPTURE_LOCATION.columnNumber + 1,
    },
    restrictToFunction: false,
  });
  const exact = possible.locations.find(
    (location) =>
      location.lineNumber === CAPTURE_LOCATION.lineNumber &&
      location.columnNumber === CAPTURE_LOCATION.columnNumber,
  );
  if (!exact) throw new Error("Input service capture breakpoint is not exact");

  const breakpoint = await client.command("Debugger.setBreakpoint", {
    location: exact,
  });
  let paused = false;
  try {
    const event = await client.waitForEvent(
      "Debugger.paused",
      (params) => params.hitBreakpoints.includes(breakpoint.breakpointId),
      5_000,
    );
    paused = true;
    const frame = event.callFrames[0];
    const evaluated = await client.command("Debugger.evaluateOnCallFrame", {
      callFrameId: frame.callFrameId,
      expression:
        `(()=>{const services=h.get();globalThis.__worklouderctlInputCapture={app:H,services};return {version:H.getVersion(),serviceCount:Object.keys(services).length}})()`,
      returnByValue: true,
      throwOnSideEffect: false,
    });
    const capture = unwrapRemoteResult(evaluated);
    assertEqual(capture.version, EXPECTED_VERSION, "captured Input app version");
    return capture;
  } finally {
    if (paused) await client.command("Debugger.resume");
    await client.command("Debugger.removeBreakpoint", {
      breakpointId: breakpoint.breakpointId,
    });
  }
}

async function scheduleInspectorClose(client) {
  return client.evaluate(
    `(()=>{const inspector=process.getBuiltinModule("inspector");process.once("SIGUSR1",()=>inspector.open(${PORT},"127.0.0.1",false));setTimeout(()=>inspector.close(),250);return true})()`,
  );
}

function progress(message) {
  process.stderr.write(`[input-live-bridge] ${message}\n`);
}

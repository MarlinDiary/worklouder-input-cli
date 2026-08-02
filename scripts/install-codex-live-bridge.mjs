#!/usr/bin/env node
import { access } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import {
  InspectorClient,
  assertEqual,
  exactProcessIds,
  inspectorTarget,
  plistValue,
  sha256File,
} from "./live-bridge-cdp.mjs";

const APP = "/Applications/ChatGPT.app";
const EXECUTABLE = `${APP}/Contents/MacOS/ChatGPT`;
const PLIST = `${APP}/Contents/Info.plist`;
const ASAR = `${APP}/Contents/Resources/app.asar`;
const EXPECTED_VERSION = "26.727.51351";
const EXPECTED_ASAR_SHA256 =
  "a529edd72e10b08931c0d695b5e3e6a0be7f51874610dafc04f578436ab7d74d";
const PORT = 9229;

const action = process.argv.includes("--remove") ? "remove" : "install";
if (process.argv.includes("--help")) {
  console.log("usage: install-codex-live-bridge.mjs [--remove]");
  process.exit(0);
}

await access(EXECUTABLE);
assertEqual(
  await plistValue(PLIST, "CFBundleShortVersionString"),
  EXPECTED_VERSION,
  "Codex version",
);
assertEqual(await sha256File(ASAR), EXPECTED_ASAR_SHA256, "Codex app.asar SHA-256");

let target;
try {
  target = await inspectorTarget(PORT, 500);
} catch {
  const pids = await exactProcessIds(EXECUTABLE);
  if (pids.length !== 1) {
    throw new Error(`expected one running Codex main process, detected ${pids.length}`);
  }
  process.kill(pids[0], "SIGUSR1");
  target = await inspectorTarget(PORT);
}

const client = await InspectorClient.connect(target.webSocketDebuggerUrl);
try {
  assertEqual(await client.evaluate("process.execPath"), EXECUTABLE, "inspector process executable");
  if (action === "remove") {
    const result = await client.evaluate(`(async()=>{const current=globalThis.__worklouderctlCodexBridge;if(!current)return {removed:false};await current.stop();delete globalThis.__worklouderctlCodexBridge;return {removed:true}})()`);
    console.log(JSON.stringify({ provider: "codex", action, ...result }, null, 2));
  } else {
    const modulePath = fileURLToPath(
      new URL("../companion/codex-live-overlay-v2.mjs", import.meta.url),
    );
    const expression = `(async()=>{const require=process.getBuiltinModule("module").createRequire("/tmp/worklouderctl-codex-live.cjs");const {app,BrowserWindow}=require("electron");const overlayPath=${JSON.stringify(modulePath)};const overlay=require(overlayPath);if(!Number.isInteger(overlay.CODEX_LIVE_OVERLAY_REVISION))throw new Error("Codex overlay revision missing");const current=globalThis.__worklouderctlCodexBridge;if(current?.overlayRevision===overlay.CODEX_LIVE_OVERLAY_REVISION)return {installed:true,idempotent:true,overlayRevision:current.overlayRevision,socketPath:current.socketPath,tokenPath:current.tokenPath};if(current){await current.stop();delete globalThis.__worklouderctlCodexBridge}const path=require("node:path");const settingsModule=require(path.join(app.getAppPath(),".vite/build/src-CLstCQVF.js"));const bridge=await overlay.installCodexLiveOverlay({app,BrowserWindow,settingsDefinitions:settingsModule.Li});globalThis.__worklouderctlCodexBridge=bridge;return {installed:true,idempotent:false,overlayRevision:bridge.overlayRevision,version:app.getVersion(),socketPath:bridge.socketPath,tokenPath:bridge.tokenPath}})()`;
    const result = await client.evaluate(expression);
    console.log(JSON.stringify({ provider: "codex", action, ...result }, null, 2));
  }
} finally {
  await client
    .evaluate(
      `(()=>{const inspector=process.getBuiltinModule("inspector");process.once("SIGUSR1",()=>inspector.open(${PORT},"127.0.0.1",false));setTimeout(()=>inspector.close(),250);return true})()`,
    )
    .catch(() => false);
  client.close();
}

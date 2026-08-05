#!/usr/bin/env node
import { access } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import {
  InspectorClient,
  assertEqual,
  exactProcessIds,
  inspectorTargetForProcess,
  plistValue,
  sha256File,
  unwrapRemoteResult,
  waitForInspectorPortRelease,
} from "./live-bridge-cdp.mjs";
import { acquireProviderLock } from "./provider-lock.mjs";

const APP = "/Applications/ChatGPT.app";
const EXECUTABLE = `${APP}/Contents/MacOS/ChatGPT`;
const PLIST = `${APP}/Contents/Info.plist`;
const ASAR = `${APP}/Contents/Resources/app.asar`;
const EXPECTED_VERSION = "26.730.61309";
const EXPECTED_ASAR_SHA256 =
  "9de942a9a058fca20b78d171032e0fe65ccb1063868f175ff7eb4e159efc2c38";
const CODEX_MAIN = `${APP}/Contents/Resources/app.asar/.vite/build/src-Bn_6ASpg.js`;
const DEVICE_SERVICE_MODULE = "./service-D-Jqk1B5.js";
const PORT = 9229;
const PROVIDER_LOCK = `${process.env.HOME}/Library/Application Support/worklouderctl/provider-handoff.lock`;

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

const providerLock = await acquireProviderLock({
  lockPath: PROVIDER_LOCK,
  mode: `install-codex-live-bridge-${action}`,
});
try {
const pids = await exactProcessIds(EXECUTABLE);
if (pids.length !== 1) {
  throw new Error(`expected one running Codex main process, detected ${pids.length}`);
}
const { target } = await inspectorTargetForProcess({
  port: PORT,
  pid: pids[0],
  executable: EXECUTABLE,
});

const client = await InspectorClient.connect(target.webSocketDebuggerUrl);
const objectGroup = "worklouderctl-codex-live-bridge-install";
try {
  assertEqual(await client.evaluate("process.execPath"), EXECUTABLE, "inspector process executable");
  if (action === "remove") {
    const result = await client.evaluate(`(async()=>{const current=globalThis.__worklouderctlCodexBridge;if(!current){delete globalThis.__worklouderctlCodexDeviceServices;return {removed:false}}await current.stop();delete globalThis.__worklouderctlCodexBridge;delete globalThis.__worklouderctlCodexDeviceServices;return {removed:true}})()`);
    console.log(JSON.stringify({ provider: "codex", action, ...result }, null, 2));
  } else {
    const serviceCount = await captureDeviceServices(client, objectGroup);
    if (!Number.isInteger(serviceCount) || serviceCount < 1) {
      throw new Error("Codex device service instances were unavailable");
    }
    const modulePath = fileURLToPath(
      new URL("../companion-codex-v10/codex-live-overlay-v2.mjs", import.meta.url),
    );
    const expression = `(async()=>{const require=process.getBuiltinModule("module").createRequire("/tmp/worklouderctl-codex-live.cjs");const {app,BrowserWindow}=require("electron");const overlayPath=${JSON.stringify(modulePath)};const path=require("node:path");const companionRoot=path.dirname(overlayPath)+path.sep;for(const key of Object.keys(require.cache)){if(key===overlayPath||key.startsWith(companionRoot))delete require.cache[key]}const overlay=require(overlayPath);if(!Number.isInteger(overlay.CODEX_LIVE_OVERLAY_REVISION))throw new Error("Codex overlay revision missing");const current=globalThis.__worklouderctlCodexBridge;if(current?.overlayRevision===overlay.CODEX_LIVE_OVERLAY_REVISION&&current.capabilities?.includes("codex.device.focus.v1")&&current.capabilities?.includes("codex.runtime.status.v1"))return {installed:true,idempotent:true,overlayRevision:current.overlayRevision,socketPath:current.socketPath,tokenPath:current.tokenPath,deviceServiceCount:globalThis.__worklouderctlCodexDeviceServices?.length??0};if(current){await current.stop();delete globalThis.__worklouderctlCodexBridge}const settingsModule=require(path.join(app.getAppPath(),".vite/build/src-Bn_6ASpg.js"));const deviceServiceProvider=()=>globalThis.__worklouderctlCodexDeviceServices;const bridge=await overlay.installCodexLiveOverlay({app,BrowserWindow,settingsDefinitions:settingsModule.Oi,deviceServiceProvider});globalThis.__worklouderctlCodexBridge=bridge;return {installed:true,idempotent:false,overlayRevision:bridge.overlayRevision,version:app.getVersion(),socketPath:bridge.socketPath,tokenPath:bridge.tokenPath,deviceServiceCount:globalThis.__worklouderctlCodexDeviceServices?.length??0}})()`;
    const result = await client.evaluate(expression);
    console.log(JSON.stringify({ provider: "codex", action, ...result }, null, 2));
  }
} finally {
  await client.command("Runtime.releaseObjectGroup", { objectGroup }).catch(() => null);
  const closeScheduled = await client
    .evaluate(
      `(()=>{const inspector=process.getBuiltinModule("inspector");process.once("SIGUSR1",()=>inspector.open(${PORT},"127.0.0.1",false));setTimeout(()=>inspector.close(),250);return true})()`,
    )
    .catch(() => false);
  client.close();
  if (closeScheduled) {
    await waitForInspectorPortRelease(PORT);
  }
}
} finally {
  await providerLock.release();
}

async function captureDeviceServices(client, objectGroup) {
  const evaluated = await client.command("Runtime.evaluate", {
    expression:
      `(()=>{const require=process.getBuiltinModule('module').createRequire(${JSON.stringify(CODEX_MAIN)});` +
      `return require(${JSON.stringify(DEVICE_SERVICE_MODULE)}).CodexMicroService.prototype})()`,
    objectGroup,
  });
  const prototype = evaluated.result.objectId;
  if (prototype == null) throw new Error("Codex service prototype missing");
  const queried = await client.command("Runtime.queryObjects", {
    prototypeObjectId: prototype,
    objectGroup,
  });
  const instances = queried.objects.objectId;
  if (instances == null) throw new Error("Codex service instances missing");
  const called = await client.command("Runtime.callFunctionOn", {
    objectId: instances,
    functionDeclaration:
      "function(){globalThis.__worklouderctlCodexDeviceServices=this;return this.length}",
    returnByValue: true,
    objectGroup,
  });
  return unwrapRemoteResult(called);
}

#!/usr/bin/env node
import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";
import {
  InspectorClient,
  assertEqual,
  exactProcessIds,
  inspectorTargetForProcess,
  terminateExactProcess,
  unwrapRemoteResult,
  waitForInspectorPortRelease,
} from "./live-bridge-cdp.mjs";
import {
  codexOwnsDevice,
  currentOwnerResult,
  inputOwnsDevice,
} from "./provider-state.mjs";
import { acquireProviderLock } from "./provider-lock.mjs";

const INPUT_EXECUTABLE = "/Applications/input.app/Contents/MacOS/input";
const CODEX_EXECUTABLE = "/Applications/ChatGPT.app/Contents/MacOS/ChatGPT";
const CODEX_MAIN =
  "/Applications/ChatGPT.app/Contents/Resources/app.asar/.vite/build/main-dcXtv3U5.js";
const INPUT_INSTALLER = fileURLToPath(
  new URL("./install-input-live-bridge.mjs", import.meta.url),
);
const INPUT_LAUNCH_LABEL = "dev.worklouderctl.input-provider";
const PROVIDER_LOCK = `${process.env.HOME}/Library/Application Support/worklouderctl/provider-handoff.lock`;
const PORT = 9229;
const INPUT_STARTUP_SETTLE_MS = 2_500;
const mode = process.argv[2] ?? "status";

if (process.argv.includes("--help")) {
  console.log(
    "usage: provider-handoff.mjs <status|codex|input|status-codex|status-input|acquire-codex|acquire-input|release-codex|release-input>",
  );
  process.exit(0);
}
const modes = [
  "status",
  "codex",
  "input",
  "status-codex",
  "status-input",
  "acquire-codex",
  "acquire-input",
  "release-codex",
  "release-input",
];
if (!modes.includes(mode)) {
  throw new Error(
    `provider must be one of: ${modes.join(", ")}`,
  );
}

const providerLock = await acquireProviderLock({
  lockPath: PROVIDER_LOCK,
  mode,
});
try {
  console.log(JSON.stringify(await runProviderHandoff(), null, 2));
} finally {
  await providerLock.release();
}

async function runProviderHandoff() {
  let result;
if (mode === "status-input") {
  result = await inputStatus();
} else if (mode === "status-codex") {
  result = await codexAction("status");
} else if (mode === "acquire-input") {
  result = await restartInputProvider();
} else if (mode === "acquire-codex") {
  result = await codexAction("acquire");
} else if (mode === "release-input") {
  const current = await inputStatus();
  result = current.available
    ? await inputAction("release")
    : missingInputAction("release");
} else if (mode === "release-codex") {
  result = await codexAction("release");
} else if (mode === "status") {
  result = {
    action: "status",
    input: await inputStatus(),
    codex: await codexAction("status"),
  };
} else if (mode === "codex") {
  const before = {
    input: await inputStatus(),
    codex: await codexAction("status"),
  };
  if (codexOwnsDevice(before)) {
    result = currentOwnerResult("codex", before);
  } else {
    const released = before.input.available
      ? await inputAction("release")
      : before.input.state.processRunning
        ? await quiesceInputProvider()
        : missingInputAction("release");
    try {
      const acquired = await codexAction("acquire");
      result = {
        action: "handoff",
        provider: "codex",
        idempotent: false,
        before,
        released,
        acquired,
      };
    } catch (error) {
      const rollback = await restartInputProvider().catch((rollbackError) => ({
        error: errorMessage(rollbackError),
      }));
      throw new Error(
        `Codex provider handoff failed; Input rollback=${JSON.stringify(rollback)}; cause=${errorMessage(error)}`,
      );
    }
  }
} else {
  const before = {
    input: await inputStatus(),
    codex: await codexAction("status"),
  };
  if (inputOwnsDevice(before)) {
    result = currentOwnerResult("input", before);
  } else {
    const released = await codexAction("release");
    try {
      const acquired = await restartInputProvider();
      result = {
        action: "handoff",
        provider: "input",
        idempotent: false,
        before,
        released,
        acquired,
      };
    } catch (error) {
      const inputRollback = await quiesceInputProvider().catch(
        (rollbackError) => ({ error: errorMessage(rollbackError) }),
      );
      const codexRollback = await codexAction("acquire").catch(
        (rollbackError) => ({ error: errorMessage(rollbackError) }),
      );
      throw new Error(
        `Input provider handoff failed; Input quiesce=${JSON.stringify(inputRollback)}; ` +
          `Codex rollback=${JSON.stringify(codexRollback)}; cause=${errorMessage(error)}`,
      );
    }
  }
}

  return result;
}

async function inputAction(action) {
  return withInspector(INPUT_EXECUTABLE, async (client) => {
    try {
      // Keep the asynchronous discovery transition rooted in the remote
      // process until Runtime.evaluate finishes awaiting it.
      return await client.evaluate(
        `(globalThis.__worklouderctlHandoffPromise=(async()=>{const capture=globalThis.__worklouderctlInputCapture;if(!capture?.services)throw new Error("Input live bridge capture missing");const services=capture.services,search=services.searchDevicesService,manager=services.devicesCommManager,originalKey="__worklouderctlOriginalStart",suppressedKey="__worklouderctlStartSuppressed";const suppressStart=()=>{if(typeof search[originalKey]!=="function")search[originalKey]=search.start;search.start=function(){search[suppressedKey]=(search[suppressedKey]??0)+1}};const restoreStart=()=>{if(typeof search[originalKey]==="function"){search.start=search[originalKey];delete search[originalKey]}delete search[suppressedKey]};const state=()=>{const devices=manager.getDevices();return {processRunning:true,discoveryStarted:search.started===true,polling:search.pollInterval!=null,startSuppressed:typeof search[originalKey]==="function",suppressedStartCount:search[suppressedKey]??0,deviceCount:devices.length,connectedCount:devices.filter(device=>device.isConnected()).length}};const action=${JSON.stringify(action)},initial=state(),connected=value=>value.connectedCount>0&&value.discoveryStarted&&!value.startSuppressed,released=value=>value.connectedCount===0&&!value.discoveryStarted&&value.startSuppressed;if(action==="status")return {action,available:true,state:initial};if(action==="release"&&released(initial))return {action,available:true,idempotent:true,state:initial};if(action==="await-connected"&&connected(initial))return {action:"acquire",available:true,idempotent:true,restarted:true,state:initial};if(action==="release"){suppressStart();search.dispose();manager.disconnectAllDevices();search.cachedDevices=[];search.cachedBootloaderDevices=[]}else if(action==="await-connected"){restoreStart()}else throw new Error("unsupported Input provider action: "+action);const deadline=Date.now()+20000;while(Date.now()<deadline){const current=state();if((action==="release"&&released(current))||(action==="await-connected"&&connected(current)))return {action:action==="await-connected"?"acquire":action,available:true,idempotent:false,restarted:action==="await-connected",state:current};await new Promise(resolve=>setTimeout(resolve,100))}throw new Error("Input provider transition timed out: "+JSON.stringify(state()))})())`,
        { timeout: 25_000 },
      );
    } finally {
      await client
        .evaluate(`delete globalThis.__worklouderctlHandoffPromise`)
        .catch(() => false);
    }
  });
}

async function inputStatus() {
  const pids = await exactProcessIds(INPUT_EXECUTABLE);
  if (pids.length === 0) return missingInputAction("status");
  if (pids.length !== 1) {
    throw new Error(`expected one running Input process, detected ${pids.length}`);
  }
  try {
    return await inputAction("status");
  } catch (initialError) {
    try {
      const { stdout } = await execFilePromise(process.execPath, [INPUT_INSTALLER]);
      const installation = JSON.parse(stdout);
      if (installation.installed !== true) {
        throw new Error("Input status bridge did not report installed=true");
      }
      return await inputAction("status");
    } catch (installationError) {
      return {
        action: "status",
        available: false,
        error:
          `Input bridge unavailable: ${errorMessage(initialError)}; ` +
          `repair=${errorMessage(installationError)}`,
        state: {
          processRunning: true,
          discoveryStarted: null,
          polling: null,
          startSuppressed: null,
          suppressedStartCount: null,
          deviceCount: null,
          connectedCount: null,
        },
      };
    }
  }
}

function missingInputAction(action) {
  return {
    action,
    available: false,
    idempotent: true,
    state: {
      processRunning: false,
      discoveryStarted: false,
      polling: false,
      startSuppressed: false,
      suppressedStartCount: 0,
      deviceCount: 0,
      connectedCount: 0,
    },
  };
}

async function restartInputProvider() {
  const previousPids = await exactProcessIds(INPUT_EXECUTABLE);
  // Codex/Input close their HID handles asynchronously below the JS lifecycle
  // boundary. A fresh Input process avoids reusing Input 0.18.0's disposed
  // node-hid worker, which can trap inside IOHIDManager on macOS 27.
  await execFilePromise("launchctl", ["remove", INPUT_LAUNCH_LABEL]).catch(
    () => null,
  );
  await sleep(500);
  const forcedPids = await terminateExactProcess(INPUT_EXECUTABLE);
  const terminatedPids = [...new Set([...previousPids, ...forcedPids])];
  await sleep(750);
  await execFilePromise("launchctl", [
    "submit",
    "-l",
    INPUT_LAUNCH_LABEL,
    "--",
    INPUT_EXECUTABLE,
    "--autostart",
  ]);
  const deadline = Date.now() + 15_000;
  let pid = null;
  while (Date.now() < deadline) {
    const pids = await exactProcessIds(INPUT_EXECUTABLE);
    if (pids.length === 1) {
      pid = pids[0];
      break;
    }
    await sleep(100);
  }
  const pids = await exactProcessIds(INPUT_EXECUTABLE);
  if (pid === null || pids.length !== 1 || pids[0] !== pid) {
    throw new Error("fresh Input provider process did not become ready");
  }
  // The Electron executable is visible before its main Node isolate is ready
  // to handle SIGUSR1. Wait for that startup boundary, then prove this is
  // still the same fresh process before asking the installer to attach.
  await sleep(INPUT_STARTUP_SETTLE_MS);
  const settledPids = await exactProcessIds(INPUT_EXECUTABLE);
  if (settledPids.length !== 1 || settledPids[0] !== pid) {
    throw new Error("fresh Input provider process did not remain stable during startup");
  }
  let installation;
  try {
    const { stdout } = await execFilePromise(process.execPath, [INPUT_INSTALLER]);
    installation = JSON.parse(stdout);
    if (installation.installed !== true) {
      throw new Error("fresh Input bridge did not report installed=true");
    }
    const acquired = await inputAction("await-connected");
    return {
      ...acquired,
      restarted: true,
      processId: pid,
      terminatedProcessIds: terminatedPids,
      overlayRevision: installation.overlayRevision,
    };
  } catch (error) {
    await terminateExactProcess(INPUT_EXECUTABLE).catch(() => []);
    await execFilePromise("launchctl", ["remove", INPUT_LAUNCH_LABEL]).catch(
      () => null,
    );
    throw error;
  }
}

async function quiesceInputProvider(timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    const pids = await exactProcessIds(INPUT_EXECUTABLE);
    if (pids.length === 0) {
      await sleep(150);
      continue;
    }
    if (pids.length !== 1) {
      lastError = new Error(
        `expected at most one Input rollback process, detected ${pids.length}`,
      );
      await terminateExactProcess(INPUT_EXECUTABLE);
      await sleep(250);
      continue;
    }
    try {
      const { stdout } = await execFilePromise(process.execPath, [INPUT_INSTALLER]);
      const installation = JSON.parse(stdout);
      if (installation.installed !== true) {
        throw new Error("Input rollback bridge did not report installed=true");
      }
      return {
        ...(await inputAction("release")),
        processId: pids[0],
        overlayRevision: installation.overlayRevision,
      };
    } catch (error) {
      lastError = error;
      await terminateExactProcess(INPUT_EXECUTABLE);
      await sleep(250);
    }
  }
  const remaining = await exactProcessIds(INPUT_EXECUTABLE);
  if (remaining.length > 0) {
    throw new Error(
      `Input rollback did not quiesce: ${errorMessage(lastError)}; ` +
        `remainingPids=${remaining.join(",")}`,
    );
  }
  return { ...missingInputAction("release"), rollbackWaitMs: timeoutMs };
}

async function codexAction(action) {
  return withInspector(CODEX_EXECUTABLE, async (client) => {
    const evaluated = await client.command("Runtime.evaluate", {
      expression: `(()=>{const require=process.getBuiltinModule("module").createRequire(${JSON.stringify(CODEX_MAIN)});return require("./service-4uQDVZZZ.js").CodexMicroService.prototype})()`,
      objectGroup: "worklouderctl-provider-handoff",
    });
    const prototype = evaluated.result.objectId;
    if (!prototype) throw new Error("Codex Micro prototype capture failed");
    const queried = await client.command("Runtime.queryObjects", {
      prototypeObjectId: prototype,
      objectGroup: "worklouderctl-provider-handoff",
    });
    const instances = queried.objects.objectId;
    if (!instances) throw new Error("Codex Micro instance query failed");
    const functionDeclaration = `async function(){const service=this.find(value=>value&&typeof value.start==="function"&&typeof value.stop==="function"&&typeof value.getState==="function");if(!service)throw new Error("Codex Micro service instance missing");const originalKey="__worklouderctlOriginalStart",suppressedKey="__worklouderctlStartSuppressed";const suppressStart=()=>{if(typeof service[originalKey]!=="function")service[originalKey]=service.start;service.start=function(){service[suppressedKey]=(service[suppressedKey]??0)+1}};const restoreStart=()=>{if(typeof service[originalKey]==="function"){service.start=service[originalKey];delete service[originalKey]}delete service[suppressedKey]};const state=()=>({lifecycleState:service.lifecycleState,deviceState:service.getState(),startSuppressed:typeof service[originalKey]==="function",suppressedStartCount:service[suppressedKey]??0,hasComm:service.comm!=null,hasApi:service.api!=null,hasHidSubscription:service.unsubscribeHid!=null,hasJoystickSubscription:service.unsubscribeJoystick!=null});const action=${JSON.stringify(action)},initial=state(),released=value=>value.lifecycleState==="stopped"&&value.startSuppressed&&!value.hasComm&&!value.hasApi,acquired=value=>value.lifecycleState==="started"&&!value.startSuppressed&&value.deviceState.status==="connected"&&value.hasComm&&value.hasApi&&value.hasHidSubscription&&value.hasJoystickSubscription;if(action==="status")return {action,state:initial};if((action==="release"&&released(initial))||(action==="acquire"&&acquired(initial)))return {action,idempotent:true,state:initial};if(action==="release"){suppressStart();await service.stop()}else if(action==="acquire"){restoreStart();service.start()}const deadline=Date.now()+20000;while(Date.now()<deadline){const current=state();if((action==="release"&&released(current))||(action==="acquire"&&acquired(current)))return {action,idempotent:false,state:current};await new Promise(resolve=>setTimeout(resolve,100))}throw new Error("Codex provider transition timed out: "+JSON.stringify(state()))}`;
    const called = await client.command("Runtime.callFunctionOn", {
      objectId: instances,
      functionDeclaration,
      returnByValue: true,
      awaitPromise: true,
      objectGroup: "worklouderctl-provider-handoff",
    });
    const result = unwrapRemoteResult(called);
    await client.command("Runtime.releaseObjectGroup", {
      objectGroup: "worklouderctl-provider-handoff",
    });
    return result;
  });
}

async function withInspector(executable, operation) {
  const pids = await exactProcessIds(executable);
  if (pids.length !== 1) {
    throw new Error(`expected one running process for ${executable}; detected ${pids.length}`);
  }
  const { target } = await inspectorTargetForProcess({
    port: PORT,
    pid: pids[0],
    executable,
  });
  const client = await InspectorClient.connect(target.webSocketDebuggerUrl);
  try {
    assertEqual(await client.evaluate("process.execPath"), executable, "inspector executable");
    return await operation(client);
  } finally {
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
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function execFilePromise(file, args) {
  return new Promise((resolve, reject) => {
    execFile(file, args, { encoding: "utf8" }, (error, stdout, stderr) => {
      if (error) {
        reject(new Error(`${file} failed: ${stderr.trim() || error.message}`));
      } else {
        resolve({ stdout, stderr });
      }
    });
  });
}

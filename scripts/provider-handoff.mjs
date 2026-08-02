#!/usr/bin/env node
import {
  InspectorClient,
  assertEqual,
  exactProcessIds,
  inspectorTarget,
  unwrapRemoteResult,
} from "./live-bridge-cdp.mjs";

const INPUT_EXECUTABLE = "/Applications/input.app/Contents/MacOS/input";
const CODEX_EXECUTABLE = "/Applications/ChatGPT.app/Contents/MacOS/ChatGPT";
const CODEX_MAIN =
  "/Applications/ChatGPT.app/Contents/Resources/app.asar/.vite/build/main-dcXtv3U5.js";
const PORT = 9229;
const mode = process.argv[2] ?? "status";

if (process.argv.includes("--help")) {
  console.log("usage: provider-handoff.mjs <status|codex|input>");
  process.exit(0);
}
if (!["status", "codex", "input"].includes(mode)) {
  throw new Error("provider must be status, codex, or input");
}

let result;
if (mode === "status") {
  result = {
    action: "status",
    input: await inputAction("status"),
    codex: await codexAction("status"),
  };
} else if (mode === "codex") {
  const before = {
    input: await inputAction("status"),
    codex: await codexAction("status"),
  };
  const released = await inputAction("release");
  try {
    const acquired = await codexAction("acquire");
    result = { action: "handoff", provider: "codex", before, released, acquired };
  } catch (error) {
    const rollback = await inputAction("acquire").catch((rollbackError) => ({
      error: errorMessage(rollbackError),
    }));
    throw new Error(
      `Codex provider handoff failed; Input rollback=${JSON.stringify(rollback)}; cause=${errorMessage(error)}`,
    );
  }
} else {
  const before = {
    input: await inputAction("status"),
    codex: await codexAction("status"),
  };
  const released = await codexAction("release");
  try {
    const acquired = await inputAction("acquire");
    result = { action: "handoff", provider: "input", before, released, acquired };
  } catch (error) {
    const rollback = await codexAction("acquire").catch((rollbackError) => ({
      error: errorMessage(rollbackError),
    }));
    throw new Error(
      `Input provider handoff failed; Codex rollback=${JSON.stringify(rollback)}; cause=${errorMessage(error)}`,
    );
  }
}

console.log(JSON.stringify(result, null, 2));

async function inputAction(action) {
  return withInspector(INPUT_EXECUTABLE, async (client) =>
    client.evaluate(
      `(async()=>{const capture=globalThis.__worklouderctlInputCapture;if(!capture?.services)throw new Error("Input live bridge capture missing");const services=capture.services,search=services.searchDevicesService,manager=services.devicesCommManager;const state=()=>{const devices=manager.getDevices();return {discoveryStarted:search.started===true,polling:search.pollInterval!=null,deviceCount:devices.length,connectedCount:devices.filter(device=>device.isConnected()).length}};if(${JSON.stringify(action)}==="release"){search.dispose();manager.disconnectAllDevices();search.cachedDevices=[];search.cachedBootloaderDevices=[]}else if(${JSON.stringify(action)}==="acquire"){search.cachedDevices=[];search.cachedBootloaderDevices=[];search.start()}const deadline=Date.now()+20000;while(Date.now()<deadline){const current=state();if(${JSON.stringify(action)}==="status"||(${JSON.stringify(action)}==="release"&&current.connectedCount===0&&!current.discoveryStarted)||(${JSON.stringify(action)}==="acquire"&&current.connectedCount>0&&current.discoveryStarted))return {action:${JSON.stringify(action)},state:current};await new Promise(resolve=>setTimeout(resolve,100))}throw new Error("Input provider transition timed out: "+JSON.stringify(state()))})()`,
      { timeout: 25_000 },
    ),
  );
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
    const functionDeclaration = `async function(){const service=this.find(value=>value&&typeof value.start==="function"&&typeof value.stop==="function"&&typeof value.getState==="function");if(!service)throw new Error("Codex Micro service instance missing");const state=()=>({lifecycleState:service.lifecycleState,deviceState:service.getState(),hasComm:service.comm!=null,hasApi:service.api!=null,hasHidSubscription:service.unsubscribeHid!=null,hasJoystickSubscription:service.unsubscribeJoystick!=null});const action=${JSON.stringify(action)};if(action==="release")await service.stop();else if(action==="acquire")service.start();const deadline=Date.now()+20000;while(Date.now()<deadline){const current=state();if(action==="status"||(action==="release"&&current.lifecycleState==="stopped"&&!current.hasComm&&!current.hasApi)||(action==="acquire"&&current.lifecycleState==="started"&&current.deviceState.status==="connected"&&current.hasComm&&current.hasApi&&current.hasHidSubscription&&current.hasJoystickSubscription))return {action,state:current};await new Promise(resolve=>setTimeout(resolve,100))}throw new Error("Codex provider transition timed out: "+JSON.stringify(state()))}`;
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
  let target;
  try {
    target = await inspectorTarget(PORT, 300);
  } catch {
    process.kill(pids[0], "SIGUSR1");
    target = await inspectorTarget(PORT);
  }
  const client = await InspectorClient.connect(target.webSocketDebuggerUrl);
  try {
    assertEqual(await client.evaluate("process.execPath"), executable, "inspector executable");
    return await operation(client);
  } finally {
    await client
      .evaluate(
        `(()=>{const inspector=process.getBuiltinModule("inspector");process.once("SIGUSR1",()=>inspector.open(${PORT},"127.0.0.1",false));setTimeout(()=>inspector.close(),250);return true})()`,
      )
      .catch(() => false);
    client.close();
    await new Promise((resolve) => setTimeout(resolve, 350));
  }
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

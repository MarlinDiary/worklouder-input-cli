import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import test from "node:test";
import {
  InspectorClient,
  inspectorTargetForProcess,
  waitForInspectorPortRelease,
} from "./live-bridge-cdp.mjs";

const PORT = 19229;

test("binds the inspector to the requested process and releases the port", async (t) => {
  const child = spawn(
    process.execPath,
    [`--inspect-port=${PORT}`, "-e", "setInterval(()=>{},1000)"],
    { stdio: "ignore" },
  );
  t.after(() => {
    if (child.exitCode === null) child.kill("SIGKILL");
  });
  await sleep(100);

  const { target, openedInspector } = await inspectorTargetForProcess({
    port: PORT,
    pid: child.pid,
    executable: process.execPath,
  });
  assert.equal(openedInspector, true);

  const client = await InspectorClient.connect(target.webSocketDebuggerUrl);
  try {
    assert.deepEqual(await client.evaluate("({pid:process.pid,executable:process.execPath})"), {
      pid: child.pid,
      executable: process.execPath,
    });
    assert.equal(
      await client.evaluate(
        `(()=>{const inspector=process.getBuiltinModule("inspector");setTimeout(()=>inspector.close(),100);return true})()`,
      ),
      true,
    );
  } finally {
    client.close();
  }
  await waitForInspectorPortRelease(PORT);
});

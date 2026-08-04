import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import test from "node:test";
import { startCodexCompanionBridge } from "../companion/codex-main-bridge.mjs";
import {
  CodexBridgeClient,
  CodexBridgeRpcError,
  createFocusForwarder,
  launchAgentProgramArguments,
  relayHealth,
} from "./codex-focus-relay-core.mjs";

const TOKEN = "0123456789abcdef".repeat(4);
const APP = {
  appName: "Fixture",
  process: "dev.worklouderctl.fixture",
  path: "/Applications/Fixture.app",
};

test("persistent Codex bridge client authenticates and forwards focus", async () => {
  const root = await mkdtemp("/tmp/worklouderctl-focus-client-");
  const socketPath = `${root}/bridge.sock`;
  const tokenPath = `${root}/bridge.token`;
  const bridge = await startCodexCompanionBridge({
    adapter: {
      async focusDevice({ app }) {
        return focusResult(app);
      },
    },
    codexVersion: "fixture",
    socketPath,
    tokenPath,
    token: TOKEN,
  });
  const client = new CodexBridgeClient({ socketPath, tokenPath, timeoutMs: 1_000 });
  try {
    const first = await client.call("codex.device.focus", { app: APP, expectLayer: null });
    const second = await client.call("codex.device.focus", { app: APP, expectLayer: null });
    assert.equal(first.operation, "focus");
    assert.equal(second.app.process, APP.process);
    assert.equal(client.nextId, 4);
  } finally {
    client.close();
    await bridge.stop();
  }
});

test("focus forwarder retries device timeouts without reinstalling the bridge", async () => {
  let calls = 0;
  let installs = 0;
  const client = {
    async call() {
      calls += 1;
      if (calls < 3) throw new CodexBridgeRpcError("device timeout", { code: -32008 });
      return focusResult(APP);
    },
    close() {},
  };
  const forwarder = createFocusForwarder({
    socketPath: "fixture",
    tokenPath: "fixture",
    installBridge: async () => { installs += 1; },
    clientFactory: () => client,
    retryDelaysMs: [0, 1, 1],
    sleep: async () => {},
  });
  const forwarded = await forwarder.forward(APP);
  assert.equal(forwarded.retryCount, 2);
  assert.equal(installs, 0);
});

test("focus forwarder reinstalls once after a transport failure", async () => {
  let factories = 0;
  let installs = 0;
  const forwarder = createFocusForwarder({
    socketPath: "fixture",
    tokenPath: "fixture",
    installBridge: async () => { installs += 1; },
    clientFactory: () => {
      factories += 1;
      return {
        async call() {
          if (factories === 1) {
            throw new CodexBridgeRpcError("socket closed", { transport: true });
          }
          return focusResult(APP);
        },
        close() {},
      };
    },
    retryDelaysMs: [0, 1],
    sleep: async () => {},
  });
  const forwarded = await forwarder.forward(APP);
  assert.equal(forwarded.retryCount, 1);
  assert.equal(forwarded.bridgeReinstalled, true);
  assert.equal(installs, 1);
  assert.equal(factories, 2);
});

test("relay health distinguishes process state from functional health", () => {
  assert.equal(relayHealth({ installed: true, running: true, lastEvent: null }).status, "starting");
  assert.equal(relayHealth({
    installed: true,
    running: true,
    lastEvent: { at: "fixture", error: "timeout" },
  }).status, "degraded");
  assert.equal(relayHealth({
    installed: true,
    running: true,
    lastEvent: { at: "fixture", result: {} },
  }).healthy, true);
});

test("LaunchAgent uses a stable PATH lookup for a bare Node command", () => {
  assert.deepEqual(
    launchAgentProgramArguments({ nodeCommand: "node", scriptPath: "/tmp/relay.mjs" }),
    ["/usr/bin/env", "node", "/tmp/relay.mjs", "run"],
  );
  assert.deepEqual(
    launchAgentProgramArguments({
      nodeCommand: "/opt/runtime/bin/node",
      scriptPath: "/tmp/relay.mjs",
    }),
    ["/opt/runtime/bin/node", "/tmp/relay.mjs", "run"],
  );
});

function focusResult(app) {
  return {
    operation: "focus",
    app,
    continuity: {
      sameServiceApi: true,
      sameComm: true,
      sameConnectionAttempt: true,
      lifecycleState: "started",
      deviceState: { status: "connected" },
    },
  };
}

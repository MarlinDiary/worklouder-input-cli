import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mkdtemp, readFile, stat } from "node:fs/promises";
import { createConnection } from "node:net";
import test from "node:test";
import {
  agentKeysRevision,
  createCodexMainAdapter,
  settingsRevision,
} from "./codex-main-adapter.mjs";
import { startCodexCompanionBridge } from "./codex-main-bridge.mjs";
import { installCodexCompanionBridge } from "./codex-main-integration.mjs";

const TOKEN = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

test("Codex bridge authenticates, gates mutations, and dispatches snapshots", async () => {
  const root = await mkdtemp("/tmp/wlb-codex-bridge-");
  const bridge = await startCodexCompanionBridge({
    adapter: {
      async snapshotSettings() {
        return { marker: "settings-from-codex-session" };
      },
    },
    codexVersion: "26.727.51351-test",
    socketPath: root + "/bridge.sock",
    tokenPath: root + "/bridge.token",
    token: TOKEN,
  });
  try {
    assert.equal((await stat(bridge.socketPath)).mode & 0o777, 0o600);
    assert.equal((await stat(bridge.tokenPath)).mode & 0o777, 0o600);
    assert.equal(await readFile(bridge.tokenPath, "utf8"), TOKEN);
    const client = await connect(bridge.socketPath);
    const hello = await client.request("bridge.hello", {
      protocolVersion: 1,
      token: TOKEN,
      client: { name: "test", version: "1" },
    });
    assert.equal(hello.result.codexVersion, "26.727.51351-test");
    assert.ok(hello.result.capabilities.includes("codex.settings.snapshot.v1"));
    assert.ok(!hello.result.capabilities.includes("codex.settings.apply.v1"));
    const snapshot = await client.request("codex.settings.snapshot", {});
    assert.equal(snapshot.result.marker, "settings-from-codex-session");
    const mutation = await client.request("codex.settings.apply", mutationParams({}));
    assert.equal(mutation.error.code, -32003);
    client.close();
  } finally {
    await bridge.stop();
  }
});

test("Codex bridge rejects a mismatched token before dispatch", async () => {
  const root = await mkdtemp("/tmp/wlb-codex-auth-");
  let calls = 0;
  const bridge = await startCodexCompanionBridge({
    adapter: { async snapshotSettings() { calls += 1; return {}; } },
    codexVersion: "test",
    socketPath: root + "/bridge.sock",
    tokenPath: root + "/bridge.token",
    token: TOKEN,
  });
  try {
    const client = await connect(bridge.socketPath);
    const response = await client.request("bridge.hello", {
      protocolVersion: 1,
      token: "f".repeat(64),
      client: { name: "test", version: "1" },
    });
    assert.equal(response.error.code, -32001);
    assert.equal(calls, 0);
    client.close();
  } finally {
    await bridge.stop();
  }
});

test("Codex adapter snapshots settings and validates all Agent Key assignment types", async () => {
  const state = testState();
  state.assignments = {
    AG00: { type: "command", commandId: "composer.togglePlanMode" },
    AG01: { type: "skill", skillName: "Review", skillPath: "/tmp/review/SKILL.md" },
    AG02: { hostId: "host", threadKey: "thread", title: "Task" },
    AG03: { keycapId: "GIT" },
  };
  const adapter = testAdapter(state);
  const settings = await adapter.snapshotSettings({});
  assert.equal(settings.settingsRevision, settingsRevision(state.settings));
  assert.match(settings.sourceSha256, /^[0-9a-f]{64}$/);
  const keys = await adapter.snapshotAgentKeys({});
  assert.deepEqual(Object.keys(keys.assignments), ["AG00", "AG01", "AG02", "AG03", "AG04", "AG05"]);
  assert.equal(keys.assignments.AG04, null);
  assert.equal(keys.globalStateRevision, agentKeysRevision(keys.assignments));
});

test("Codex adapter applies, replays, rejects stale CAS, and restores", async () => {
  const state = testState();
  const adapter = testAdapter(state);
  const baseline = await adapter.snapshotSettings({});
  const candidateSettings = { ...baseline.settings, "codex-micro-agent-source": "custom" };
  const candidateEffective = { ...baseline.effectiveSettings, "codex-micro-agent-source": "custom" };
  const candidateRevision = settingsRevision(candidateSettings);
  const apply = await adapter.applySettings(mutationParams({
    expectedSourceSha256: baseline.sourceSha256,
    expectedSettingsRevision: baseline.settingsRevision,
    targetSettingsRevision: candidateRevision,
    settings: candidateSettings,
    effectiveSettings: candidateEffective,
    idempotencyKey: "apply-1",
  }));
  assert.equal(apply.changed, true);
  assert.equal(apply.afterSettingsRevision, candidateRevision);
  const replay = await adapter.applySettings(mutationParams({
    expectedSourceSha256: baseline.sourceSha256,
    expectedSettingsRevision: baseline.settingsRevision,
    targetSettingsRevision: candidateRevision,
    settings: candidateSettings,
    effectiveSettings: candidateEffective,
    idempotencyKey: "apply-1",
  }));
  assert.equal(replay.idempotentReplay, true);
  await assert.rejects(
    adapter.applySettings(mutationParams({
      expectedSourceSha256: baseline.sourceSha256,
      expectedSettingsRevision: baseline.settingsRevision,
      targetSettingsRevision: candidateRevision,
      settings: candidateSettings,
      effectiveSettings: candidateEffective,
      idempotencyKey: "stale",
    })),
    (error) => error.code === -32005,
  );
  const live = await adapter.snapshotSettings({});
  const restore = await adapter.restoreSettings(mutationParams({
    expectedSourceSha256: live.sourceSha256,
    expectedSettingsRevision: live.settingsRevision,
    targetSettingsRevision: baseline.settingsRevision,
    settings: baseline.settings,
    effectiveSettings: baseline.effectiveSettings,
    idempotencyKey: "restore-1",
  }));
  assert.equal(restore.afterSettingsRevision, baseline.settingsRevision);
});

test("Codex adapter rolls back after exact readback failure", async () => {
  const state = testState();
  const adapter = testAdapter(state, { corruptOnce: true });
  const baseline = await adapter.snapshotSettings({});
  const settings = { ...baseline.settings, "codex-micro-lighting-brightness": 42 };
  await assert.rejects(
    adapter.applySettings(mutationParams({
      expectedSourceSha256: baseline.sourceSha256,
      expectedSettingsRevision: baseline.settingsRevision,
      targetSettingsRevision: settingsRevision(settings),
      settings,
      effectiveSettings: { ...baseline.effectiveSettings, "codex-micro-lighting-brightness": 42 },
      idempotencyKey: "corrupt-readback",
    })),
    (error) => error.code === -32008 && error.data.rollbackPerformed === true,
  );
  const restored = await adapter.snapshotSettings({});
  assert.equal(restored.settingsRevision, baseline.settingsRevision);
});

test("Codex integration uses the Codex userData boundary and lifecycle", async () => {
  const root = await mkdtemp("/tmp/wlb-codex-integration-");
  const app = new EventEmitter();
  app.getPath = (name) => { assert.equal(name, "userData"); return root; };
  app.getVersion = () => "26.727.51351-test";
  const state = testState();
  const bridge = await installCodexCompanionBridge({
    app,
    request: makeRequest(state),
  });
  try {
    assert.equal(bridge.socketPath, root + "/worklouderctl-codex-bridge-v1.sock");
    assert.ok(bridge.capabilities.includes("codex.settings.snapshot.v1"));
    assert.ok(!bridge.capabilities.includes("codex.settings.apply.v1"));
  } finally {
    await bridge.stop();
  }
});

function testState() {
  const settings = {
    "codex-micro-agent-source": "recent",
    "codex-micro-single-tap-agent-keys": false,
    "codex-micro-lighting-brightness": 100,
  };
  return {
    settings,
    effectiveSettings: structuredClone(settings),
    definitions: {
      "codex-micro-agent-source": { type: "string", default: "recent" },
    },
    source: Buffer.from(JSON.stringify(settings)),
    assignments: null,
  };
}

function makeRequest(state) {
  return async (method, params) => {
    if (method === "settings-read") {
      return {
        filePath: "/fixture/config.toml",
        settings: structuredClone(state.settings),
        effectiveSettings: structuredClone(state.effectiveSettings),
        definitions: structuredClone(state.definitions),
      };
    }
    if (method === "get-global-state") {
      assert.equal(params.key, "codex-micro-custom-agent-assignments");
      return { value: structuredClone(state.assignments) };
    }
    throw new Error("unexpected request: " + method);
  };
}

function testAdapter(state, { corruptOnce = false } = {}) {
  let corrupt = corruptOnce;
  return createCodexMainAdapter({
    request: makeRequest(state),
    readSettingsSource: async () => state.source,
    settingsReplacer: {
      async replaceSettings({ settings, operation }) {
        state.settings = structuredClone(settings);
        state.effectiveSettings = structuredClone(settings);
        state.source = Buffer.from(JSON.stringify(settings));
        if (corrupt && operation !== "automatic-rollback") {
          corrupt = false;
          state.effectiveSettings["codex-micro-lighting-brightness"] = 99;
        }
      },
    },
  });
}

function mutationParams(overrides) {
  return {
    expectedSourceSha256: "0".repeat(64),
    expectedSettingsRevision: "0".repeat(64),
    targetSettingsRevision: "0".repeat(64),
    idempotencyKey: "test",
    settings: {},
    effectiveSettings: {},
    ...overrides,
  };
}

async function connect(socketPath) {
  const socket = createConnection(socketPath);
  socket.setEncoding("utf8");
  await new Promise((resolve, reject) => {
    socket.once("connect", resolve);
    socket.once("error", reject);
  });
  let nextId = 1;
  let buffer = "";
  const pending = new Map();
  socket.on("data", (chunk) => {
    buffer += chunk;
    while (buffer.includes("\n")) {
      const newline = buffer.indexOf("\n");
      const message = JSON.parse(buffer.slice(0, newline));
      buffer = buffer.slice(newline + 1);
      pending.get(message.id)?.(message);
      pending.delete(message.id);
    }
  });
  return {
    request(method, params) {
      const id = nextId++;
      return new Promise((resolve) => {
        pending.set(id, resolve);
        socket.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
      });
    },
    close() { socket.destroy(); },
  };
}

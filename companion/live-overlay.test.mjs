import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import {
  codexRendererRequestExpression,
  createCodexMicroRequest,
  createCodexNativeRequest,
  normalizeCodexMicroDefinitions,
} from "./codex-live-overlay.mjs";
import { createInputConfigurationWriter } from "./input-live-overlay.mjs";
import { installInputLiveOverlay } from "./input-live-overlay.mjs";

test("Codex live request uses the official renderer message bridge", async () => {
  const calls = [];
  const hidden = browserWindow({
    visible: false,
    execute: async () => {
      throw new Error("not a Codex window");
    },
  });
  const visible = browserWindow({
    visible: true,
    execute: async (expression, userGesture) => {
      calls.push({ expression, userGesture });
      return { status: 200, body: { settings: { marker: true } } };
    },
  });
  const request = createCodexNativeRequest({
    BrowserWindow: { getAllWindows: () => [hidden, visible] },
  });
  assert.deepEqual(await request("settings-read", {}), {
    settings: { marker: true },
  });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].userGesture, true);
  assert.match(calls[0].expression, /sendMessageFromView/);
  assert.match(calls[0].expression, /vscode:\/\/codex\//);
});

test("Codex renderer expression carries only the requested method and params", () => {
  const expression = codexRendererRequestExpression({
    method: "set-global-state",
    params: { key: "state-key", value: { AG00: null } },
    requestTimeoutMs: 15_000,
  });
  assert.match(expression, /set-global-state/);
  assert.match(expression, /state-key/);
  assert.match(expression, /fetch-response/);
  assert.doesNotMatch(expression, /ipcRenderer/);
});

test("Codex live overlay selects hidden Micro settings from released definitions", async () => {
  const definitions = normalizeCodexMicroDefinitions(codexDefinitions());
  const calls = [];
  const request = createCodexMicroRequest({
    definitions,
    async nativeRequest(method) {
      calls.push(method);
      if (method === "settings-read") {
        return { filePath: "/fixture/config.toml", settings: { visible: true } };
      }
      if (method === "get-settings") {
        return {
          configuredValues: {
            visible: true,
            "codex-micro-lighting-brightness": 75,
          },
          values: Object.fromEntries(
            Object.entries(definitions).map(([key, definition]) => [
              key,
              definition.default,
            ]),
          ),
        };
      }
      throw new Error("unexpected method");
    },
  });
  const snapshot = await request("settings-read", {});
  assert.deepEqual(calls, ["settings-read", "get-settings"]);
  assert.deepEqual(snapshot.settings, {
    "codex-micro-lighting-brightness": 75,
  });
  assert.equal(Object.keys(snapshot.effectiveSettings).length, 5);
  assert.equal(Object.keys(snapshot.definitions).length, 5);
});

test("Input live writer deletes stale files, skips unchanged bytes, writes dependency order, and refreshes cache", async () => {
  const smart = Buffer.from('{"smartActions":{}}');
  const other = Buffer.from("other-new");
  const keymap = Buffer.from('{"version":1}');
  const operations = [];
  const device = {
    rpcService: {
      async getFileList() {
        return [
          listing("keymap.json", Buffer.from("old")),
          listing("smart_actions.json", smart),
          listing("stale.json", Buffer.from("stale")),
          listing("other.json", Buffer.from("other-old")),
        ];
      },
      async deleteFile(path) {
        operations.push(["delete", path]);
        return true;
      },
      async writeFileChunked(path, bytes) {
        operations.push(["write", path, Buffer.from(bytes).toString()]);
        return true;
      },
    },
  };
  const writer = createInputConfigurationWriter({
    deviceFileService: {
      async fetchDeviceFiles(actual) {
        assert.equal(actual, device);
        operations.push(["refresh"]);
      },
    },
  });
  await writer.replaceConfiguration({
    device,
    files: [
      { relativePath: "keymap.json", bytes: keymap },
      { relativePath: "other.json", bytes: other },
      { relativePath: "smart_actions.json", bytes: smart },
    ],
  });
  assert.deepEqual(operations, [
    ["delete", "stale.json"],
    ["write", "other.json", "other-new"],
    ["write", "keymap.json", '{"version":1}'],
    ["refresh"],
  ]);
});

test("Input live writer stops before cache refresh on a provider write failure", async () => {
  let refreshed = false;
  const writer = createInputConfigurationWriter({
    deviceFileService: {
      async fetchDeviceFiles() {
        refreshed = true;
      },
    },
  });
  await assert.rejects(
    writer.replaceConfiguration({
      device: {
        rpcService: {
          async getFileList() { return []; },
          async deleteFile() { return true; },
          async writeFileChunked() { return false; },
        },
      },
      files: [{ relativePath: "keymap.json", bytes: Buffer.from("target") }],
    }),
    /failed to write keymap\.json/,
  );
  assert.equal(refreshed, false);
});

test("Input live overlay preserves Input service-container prototype getters", async () => {
  const root = await import("node:fs/promises").then(({ mkdtemp }) =>
    mkdtemp("/tmp/wlb-input-live-getters-"),
  );
  const lifecycle = new Map();
  const device = {
    id: "getter-device",
    info: { devicePid: 33632, deviceType: "codex_micro" },
    isConnected: () => true,
    rpcService: {},
  };
  class InputServices {
    get devicesCommManager() {
      return { getDevices: () => [device] };
    }
    get deviceFileService() {
      return { fetchDeviceFiles: async () => {} };
    }
  }
  const app = {
    getVersion: () => "0.18.0",
    getPath: () => root,
    once: (event, listener) => lifecycle.set(event, listener),
    removeListener: (event) => lifecycle.delete(event),
  };
  const bridge = await installInputLiveOverlay({
    app,
    services: new InputServices(),
    socketPath: root + "/bridge.sock",
    tokenPath: root + "/bridge.token",
  });
  try {
    const { stat } = await import("node:fs/promises");
    assert.equal((await stat(bridge.socketPath)).isSocket(), true);
    assert.equal((await stat(bridge.tokenPath)).isFile(), true);
  } finally {
    await bridge.stop();
  }
});

function browserWindow({ visible, execute }) {
  return {
    isDestroyed: () => false,
    isVisible: () => visible,
    webContents: { executeJavaScript: execute },
  };
}

function codexDefinitions() {
  const values = [
    ["codex-micro-agent-source", "recent", { type: "string", enum: ["pinned", "recent", "priority", "custom"] }],
    ["codex-micro-single-tap-agent-keys", false, { type: "boolean" }],
    ["codex-micro-layout", { version: 1 }, { type: "object" }],
    ["codex-micro-lighting-brightness", 100, { type: "integer", minimum: 0, maximum: 100 }],
    ["codex-micro-lighting-auto-off", "3-minutes", { type: "string", enum: ["off", "3-minutes"] }],
  ];
  return values.map(([key, defaultValue, schema]) => ({
    key,
    agentAccess: "hidden",
    default: defaultValue,
    schema: { toJSONSchema: () => schema },
  }));
}

function listing(name, bytes) {
  return {
    name,
    checksum: createHash("sha1").update(bytes).digest("hex"),
  };
}

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import { mkdtemp, readFile, stat } from "node:fs/promises";
import { createConnection } from "node:net";
import test from "node:test";
import { createInputMainAdapter } from "./input-main-adapter.mjs";
import { startInputCompanionBridge } from "./input-main-bridge.mjs";
import { installInputCompanionBridge } from "./input-main-integration.mjs";

const TOKEN =
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

test("bridge authenticates and dispatches through the adapter", async () => {
  const root = await mkdtemp("/tmp/wlb-node-");
  const socketPath = root + "/bridge.sock";
  const tokenPath = root + "/bridge.token";
  const bridge = await startInputCompanionBridge({
    adapter: {
      async getDeviceStatus() {
        return { marker: "status-from-input-session" };
      },
    },
    inputVersion: "0.18.0-test",
    bridgeVersion: "0.1.0-test",
    socketPath,
    tokenPath,
    token: TOKEN,
  });

  try {
    assert.equal((await stat(socketPath)).mode & 0o777, 0o600);
    assert.equal((await stat(tokenPath)).mode & 0o777, 0o600);
    assert.equal(await readFile(tokenPath, "utf8"), TOKEN);

    const client = await connect(socketPath);
    const hello = await client.request("bridge.hello", {
      protocolVersion: 1,
      token: TOKEN,
      client: { name: "test", version: "1" },
    });
    assert.equal(hello.result.protocolVersion, 1);
    assert.equal(hello.result.inputVersion, "0.18.0-test");
    assert.ok(hello.result.capabilities.includes("device.status.v1"));

    const status = await client.request("device.status", { deviceId: null });
    assert.deepEqual(status.result, {
      marker: "status-from-input-session",
    });
    client.close();
  } finally {
    await bridge.stop();
  }
});

test("bridge rejects a mismatched token before adapter dispatch", async () => {
  const root = await mkdtemp("/tmp/wlb-auth-");
  const socketPath = root + "/bridge.sock";
  const tokenPath = root + "/bridge.token";
  let calls = 0;
  const bridge = await startInputCompanionBridge({
    adapter: {
      async getDeviceStatus() {
        calls += 1;
        return {};
      },
    },
    inputVersion: "0.18.0-test",
    socketPath,
    tokenPath,
    token: TOKEN,
  });

  try {
    const client = await connect(socketPath);
    const response = await client.request("bridge.hello", {
      protocolVersion: 1,
      token: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
      client: { name: "test", version: "1" },
    });
    assert.equal(response.error.code, -32001);
    assert.equal(calls, 0);
    client.close();
  } finally {
    await bridge.stop();
  }
});

test("Input adapter maps the existing connected session", async () => {
  const keymap = Buffer.from('{"version":1}');
  let discoveryCalls = 0;
  const device = {
    id: "device-1",
    info: {
      devicePid: 33632,
      deviceType: "codex_micro",
      layoutType: "universal",
      connectionType: 1,
      isUsbConnection: false,
    },
    isConnected: () => true,
    rpcService: {
      async getFirmwareVersion() {
        return "v0.6.0";
      },
      async getDeviceStatus() {
        return { selectedProfileIndex: 0, selectedLayerIndex: 2 };
      },
      async getFileList() {
        return [
          {
            name: "keymap.json",
            size: keymap.length,
            checksum: createHash("sha1").update(keymap).digest("hex"),
          },
        ];
      },
      async readFileChunked(path) {
        assert.equal(path, "keymap.json");
        return keymap;
      },
    },
  };
  const adapter = createInputMainAdapter({
    devicesCommManager: {
      getDevices() {
        discoveryCalls += 1;
        return [device];
      },
    },
    deviceKitVersion: "0.1.29",
  });

  const status = await adapter.getDeviceStatus({ deviceId: null });
  const files = await adapter.listFiles({
    deviceId: null,
    path: null,
    recursive: true,
  });
  const read = await adapter.readFile({
    deviceId: null,
    path: "keymap.json",
  });

  assert.equal(discoveryCalls, 3);
  assert.equal(status.status.firmwareVersion, "v0.6.0");
  assert.equal(status.status.selectedLayerIndex, 2);
  assert.equal(files.files[0].relativePath, "keymap.json");
  assert.equal(Buffer.from(read.dataBase64, "base64").toString(), keymap.toString());
});

test("Input adapter snapshots and validates a compare-and-swap revision", async () => {
  const fileBytes = new Map([
    ["smart_actions.json", Buffer.from('{"smartActions":{}}')],
    ["keymap.json", Buffer.from('{"version":1,"layers":[]}')],
  ]);
  const device = {
    id: "device-config",
    info: {
      devicePid: 33632,
      deviceType: "codex_micro",
      layoutType: "universal",
      connectionType: 1,
      isUsbConnection: false,
    },
    isConnected: () => true,
    rpcService: {
      async getFirmwareVersion() {
        return "v0.6.0";
      },
      async getDeviceStatus() {
        return { selectedProfileIndex: 0, selectedLayerIndex: 2 };
      },
      async getFileList() {
        return [...fileBytes].map(([name, bytes]) => ({
          name,
          size: bytes.length,
          checksum: createHash("sha1").update(bytes).digest("hex"),
        }));
      },
      async readFileChunked(path) {
        return fileBytes.get(path);
      },
    },
  };
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [device] },
    deviceKitVersion: "0.1.29",
  });

  const snapshot = await adapter.snapshotConfig({ deviceId: "device-config" });
  assert.equal(snapshot.kind, "worklouder-input-config-snapshot");
  assert.equal(snapshot.deviceId, "device-config");
  assert.deepEqual(
    snapshot.files.map((file) => file.relativePath),
    ["keymap.json", "smart_actions.json"],
  );
  assert.match(snapshot.revision, /^[0-9a-f]{64}$/);
  const validation = await adapter.validateConfig({
    deviceId: "device-config",
    snapshot,
    expectedRevision: snapshot.revision,
  });
  assert.equal(validation.valid, true);
  assert.equal(validation.revision, snapshot.revision);
  assert.equal(validation.liveRevision, snapshot.revision);
  assert.equal(validation.fileCount, 2);

  const tampered = structuredClone(snapshot);
  tampered.files[0].dataBase64 = Buffer.from("tampered").toString("base64");
  await assert.rejects(
    adapter.validateConfig({ deviceId: "device-config", snapshot: tampered }),
    (error) => error.code === -32602,
  );
  await assert.rejects(
    adapter.validateConfig({
      deviceId: "device-config",
      snapshot,
      expectedRevision: "f".repeat(64),
    }),
    (error) => error.code === -32005,
  );
});

test("one-call Input integration owns discovery and lifecycle paths", async () => {
  const root = await mkdtemp("/tmp/wlb-integration-");
  class FixtureApp extends EventEmitter {
    getPath(name) {
      assert.equal(name, "userData");
      return root;
    }

    getVersion() {
      return "0.18.0-integration";
    }
  }
  const device = {
    id: "device-1",
    info: {
      devicePid: 33632,
      deviceType: "codex_micro",
      layoutType: "universal",
      connectionType: 1,
      isUsbConnection: false,
    },
    isConnected: () => true,
    rpcService: {
      async getFirmwareVersion() {
        return "v0.6.0";
      },
      async getDeviceStatus() {
        return { selectedProfileIndex: 0, selectedLayerIndex: 2 };
      },
      async getFileList() {
        return [];
      },
      async readFileChunked() {
        return Buffer.alloc(0);
      },
    },
  };
  const app = new FixtureApp();
  const integration = await installInputCompanionBridge({
    app,
    services: {
      devicesCommManager: {
        getDevices: () => [device],
      },
    },
    deviceKitVersion: "0.1.29-integration",
    bridgeVersion: "0.1.0-integration",
  });

  assert.equal(integration.inputVersion, "0.18.0-integration");
  assert.equal(
    integration.socketPath,
    root + "/worklouderctl-bridge-v1.sock",
  );
  assert.equal(
    integration.tokenPath,
    root + "/worklouderctl-bridge-v1.token",
  );
  assert.equal((await stat(integration.socketPath)).mode & 0o777, 0o600);
  assert.equal((await stat(integration.tokenPath)).mode & 0o777, 0o600);
  assert.ok(integration.capabilities.includes("device.config.snapshot.v1"));
  assert.ok(integration.capabilities.includes("device.config.validate.v1"));
  assert.equal(app.listenerCount("before-quit"), 1);
  await integration.stop();
  assert.equal(app.listenerCount("before-quit"), 0);
  await assert.rejects(stat(integration.socketPath), { code: "ENOENT" });
});

async function connect(socketPath) {
  const socket = createConnection(socketPath);
  socket.setEncoding("utf8");
  await new Promise((resolve, reject) => {
    socket.once("connect", resolve);
    socket.once("error", reject);
  });
  let nextId = 1;
  let buffer = "";
  const queued = [];
  socket.on("data", (chunk) => {
    buffer += chunk;
    while (buffer.includes("\n")) {
      const index = buffer.indexOf("\n");
      const line = buffer.slice(0, index);
      buffer = buffer.slice(index + 1);
      queued.shift()?.(JSON.parse(line));
    }
  });
  return {
    request(method, params) {
      const id = nextId;
      nextId += 1;
      return new Promise((resolve) => {
        queued.push(resolve);
        socket.write(
          JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n",
        );
      });
    },
    close() {
      socket.end();
    },
  };
}

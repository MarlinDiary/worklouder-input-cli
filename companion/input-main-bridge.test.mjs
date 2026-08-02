import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, stat } from "node:fs/promises";
import { createConnection } from "node:net";
import test from "node:test";
import { createInputMainAdapter } from "./input-main-adapter.mjs";
import { startInputCompanionBridge } from "./input-main-bridge.mjs";

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

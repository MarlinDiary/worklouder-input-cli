import { createHash } from "node:crypto";
import { startInputCompanionBridge } from "./input-main-bridge.mjs";

const [socketPath, tokenPath] = process.argv.slice(2);
if (!socketPath || !tokenPath) {
  throw new Error("usage: node fixture-server.mjs SOCKET TOKEN");
}

const files = new Map([
  [
    "keymap.json",
    Buffer.from(
      JSON.stringify({
        version: 1,
        activeProfileId: 0,
        profiles: [{ id: 0, name: "Bridge Fixture", layers: [] }],
      }),
    ),
  ],
  [
    "smart_actions.json",
    Buffer.from(
      JSON.stringify({ version: 1, smartActions: {}, smartActionGroups: [] }),
    ),
  ],
]);

const common = {
  deviceKitVersion: "0.1.29-fixture",
  device: {
    devicePid: "33632",
    deviceType: "codex_micro",
    layoutType: "universal",
    connectionType: "hid",
    isUsbConnection: false,
  },
  status: {
    firmwareVersion: "v0.6.0-fixture",
    selectedProfileIndex: 0,
    selectedLayerIndex: 2,
    batteryPercentage: null,
    isCharging: null,
  },
  warnings: [],
};

const adapter = {
  async listDevices() {
    return {
      deviceKitVersion: common.deviceKitVersion,
      devices: [{ id: "fixture-device", connected: true, device: common.device }],
    };
  },
  async getDeviceStatus() {
    return common;
  },
  async listFiles() {
    return {
      ...common,
      files: [...files].map(([relativePath, bytes]) => ({
        relativePath,
        size: bytes.length,
        deviceChecksumSha1: createHash("sha1").update(bytes).digest("hex"),
      })),
    };
  },
  async readFile({ path }) {
    const bytes = files.get(path);
    if (!bytes) {
      throw new Error("fixture file not found: " + path);
    }
    return {
      relativePath: path,
      size: bytes.length,
      deviceChecksumSha1: createHash("sha1").update(bytes).digest("hex"),
      dataBase64: bytes.toString("base64"),
    };
  },
};

const bridge = await startInputCompanionBridge({
  adapter,
  inputVersion: "0.18.0-fixture",
  bridgeVersion: "0.1.0-fixture",
  socketPath,
  tokenPath,
  token: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
});

process.stdout.write(
  JSON.stringify({
    ready: true,
    socketPath,
    tokenPath,
    capabilities: bridge.capabilities,
  }) + "\n",
);

let stopping = false;
const stop = async () => {
  if (stopping) {
    return;
  }
  stopping = true;
  await bridge.stop();
  process.exit(0);
};
process.on("SIGINT", () => void stop());
process.on("SIGTERM", () => void stop());

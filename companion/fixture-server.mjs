import { createHash } from "node:crypto";
import { createInputMainAdapter } from "./input-main-adapter.mjs";
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

const device = {
  id: "fixture-device",
  info: {
    devicePid: "33632",
    deviceType: "codex_micro",
    layoutType: "universal",
    connectionType: 1,
    isUsbConnection: false,
  },
  isConnected: () => true,
  rpcService: {
    async getFirmwareVersion() {
      return "v0.6.0-fixture";
    },
    async getDeviceStatus() {
      return { selectedProfileIndex: 0, selectedLayerIndex: 2 };
    },
    async getFileList() {
      return [...files].map(([name, bytes]) => ({
        name,
        size: bytes.length,
        checksum: createHash("sha1").update(bytes).digest("hex"),
      }));
    },
    async readFileChunked(path) {
      const bytes = files.get(path);
      if (!bytes) {
        throw new Error("fixture file not found: " + path);
      }
      return bytes;
    },
  },
};

const adapter = createInputMainAdapter({
  devicesCommManager: { getDevices: () => [device] },
  deviceKitVersion: "0.1.29-fixture",
});

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

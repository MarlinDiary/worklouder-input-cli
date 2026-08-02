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
        fixtureExtension: { preserved: true },
        macros: [
          {
            id: 3,
            name: "Fixture Action",
            color: null,
            actions: [{ act: 1, delay: 0, kc: "KC_C" }],
          },
          {
            id: 4,
            name: "Dependent Action",
            color: null,
            actions: [{ act: 1, delay: 0, kc: "KA_A3" }],
          },
          {
            id: 10,
            name: "Two Digit Action",
            color: null,
            actions: [{ act: 1, delay: 0, kc: "KC_E" }],
          },
        ],
        macrosGroups: [
          {
            id: 0,
            name: "Primary",
            tags: ["fixture"],
            color: null,
            actionIds: [3, 4],
          },
          {
            id: 1,
            name: "Single",
            tags: [],
            color: null,
            actionIds: [3],
          },
        ],
        multiActions: [
          {
            id: 1,
            name: "Fixture Multi",
            color: null,
            kcOnTap: "KA_A3",
            kcOnHold: "KC_NONE",
            kcOnDoubleTap: "KC_NONE",
            kcOnTapHold: "KC_NONE",
            tt: 250,
          },
          {
            id: 2,
            name: "Dependent Multi",
            color: "#123456",
            icon: "icon-fixture",
            kcOnTap: "KC_NONE",
            kcOnHold: "KA_M1",
            kcOnDoubleTap: "KC_NONE",
            kcOnTapHold: "KC_NONE",
            tt: 300,
          },
        ],
        multiActionsGroups: [
          {
            id: 0,
            name: "Multi",
            tags: [],
            color: null,
            actionIds: [1, 2],
          },
          {
            id: 4,
            name: "Shared",
            tags: ["fixture"],
            color: "#ABCDEF",
            actionIds: [1],
          },
        ],
        profiles: [
          {
            id: 0,
            name: "Fixture Default",
            macrosUsed: [10, 3],
            multiActionsUsed: [1],
            layers: [
              {
                id: 0,
                name: "Base",
                color: 0x112233,
                layout: {
                  keymap: [["KC_A", "KC_B"], ["KA_A10"]],
                  encoders: [["KC_LEFT", "KC_RGHT", "KC_MUTE"]],
                  joystick: {
                    type: "RADIAL",
                    sectors: [
                      { a1: 0.0, a2: 1.5, k: "KA_A3" },
                      { a1: 1.5, a2: 3.0, k: "KA_M1" },
                    ],
                  },
                },
              },
              {
                id: 1,
                name: "Tools",
                color: 0x445566,
                layout: {
                  keymap: [["KI_LM2", "KC_NONE"]],
                  encoders: [],
                  joystick: { type: "VENDOR", sectors: [] },
                },
              },
            ],
          },
          {
            id: 7,
            name: "Fixture Alternate",
            layers: [{ id: 9, name: "Other", color: 0x778899, layout: {} }],
          },
        ],
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
  configurationWriter: {
    async replaceConfiguration({ files: replacement }) {
      files.clear();
      for (const file of replacement) {
        files.set(file.relativePath, Buffer.from(file.bytes));
      }
    },
  },
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

#!/usr/bin/env node
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";
import { createCodexMainAdapter } from "./codex-main-adapter.mjs";
import { startCodexCompanionBridge } from "./codex-main-bridge.mjs";

const [socketPath, tokenPath] = process.argv.slice(2);
if (!socketPath || !tokenPath) {
  process.stderr.write("usage: codex-fixture-server.mjs SOCKET TOKEN_FILE\n");
  process.exit(2);
}
const here = dirname(fileURLToPath(import.meta.url));
const contract = JSON.parse(await readFile(resolve(here, "../spec/codex-settings-26.730.61309.json"), "utf8"));
const sourcePath = join(dirname(socketPath), "codex-fixture-config.toml");
let settings = Object.fromEntries(Object.entries(contract.definitions).map(([key, definition]) => [key, structuredClone(definition.default)]));
let effectiveSettings = structuredClone(settings);
let assignments = {
  AG00: { type: "command", commandId: "fixture.command" },
  AG01: null,
  AG02: null,
  AG03: null,
  AG04: null,
  AG05: null,
};
let failSettingsWrites = Number.parseInt(
  process.env.WORKLOUDERCTL_FIXTURE_FAIL_CODEX_SETTINGS_WRITES ?? "0",
  10,
);
if (!Number.isSafeInteger(failSettingsWrites) || failSettingsWrites < 0) {
  throw new Error("WORKLOUDERCTL_FIXTURE_FAIL_CODEX_SETTINGS_WRITES must be a non-negative integer");
}
const fixtureDeviceStatus = { selectedProfileIndex: 0, selectedLayerIndex: 0 };
const fixtureServiceApi = {
  api: {
    async getDeviceStatus() {
      return structuredClone(fixtureDeviceStatus);
    },
    async sendFocusApp() {},
  },
};
const fixtureService = {
  api: fixtureServiceApi,
  comm: {},
  connectionAttemptId: "fixture-connection",
  lifecycleState: "connected",
  getState() {
    return { status: "connected" };
  },
  unsubscribeHid() {},
  unsubscribeJoystick() {},
};

await persist();
const request = async (method, params) => {
  if (method === "settings-read") {
    return {
      filePath: sourcePath,
      settings: structuredClone(settings),
      effectiveSettings: structuredClone(effectiveSettings),
      definitions: structuredClone(contract.definitions),
    };
  }
  if (method === "get-global-state") {
    if (params.key !== "codex-micro-custom-agent-assignments") throw new Error("unknown fixture state key");
    return { value: structuredClone(assignments) };
  }
  if (method === "set-global-state") {
    if (params.key !== "codex-micro-custom-agent-assignments") throw new Error("unknown fixture state key");
    assignments = structuredClone(params.value);
    return { success: true };
  }
  throw new Error("unsupported fixture request: " + method);
};
const adapter = createCodexMainAdapter({
  request,
  deviceServiceProvider: () => [fixtureService],
  settingsReplacer: {
    async replaceSettings(request) {
      if (failSettingsWrites > 0) {
        failSettingsWrites -= 1;
        throw new Error("injected Codex settings write failure");
      }
      settings = structuredClone(request.settings);
      effectiveSettings = Object.fromEntries(Object.entries(contract.definitions).map(([key, definition]) => [
        key,
        structuredClone(Object.hasOwn(settings, key) ? settings[key] : definition.default),
      ]));
      await persist();
    },
  },
  agentKeysWriter: {
    async replaceAssignments({ key, assignments }) {
      const result = await request("set-global-state", { key, value: assignments });
      if (result.success !== true) throw new Error("fixture global-state write failed");
    },
  },
});
const bridge = await startCodexCompanionBridge({
  adapter,
  codexVersion: contract.appVersion,
  bridgeVersion: "0.1.0-fixture",
  socketPath,
  tokenPath,
  token: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
});
process.stdout.write(JSON.stringify({ ready: true, socketPath, tokenPath, sourcePath }) + "\n");

let stopping = false;
const stop = async () => {
  if (stopping) return;
  stopping = true;
  await bridge.stop();
  process.exit(0);
};
process.on("SIGINT", stop);
process.on("SIGTERM", stop);

async function persist() {
  await writeFile(sourcePath, JSON.stringify(canonicalJson({ desktop: settings }), null, 2) + "\n", { mode: 0o600 });
}

function canonicalJson(value) {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonicalJson(value[key])]));
}

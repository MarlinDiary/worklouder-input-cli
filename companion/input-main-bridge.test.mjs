import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import { mkdtemp, readFile, stat } from "node:fs/promises";
import { createConnection } from "node:net";
import test from "node:test";
import {
  createInputMainAdapter,
  hostSettingsRevision,
  presetCatalogRevision,
} from "./input-main-adapter.mjs";
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
  assert.equal(
    Buffer.from(read.dataBase64, "base64").toString(),
    keymap.toString(),
  );
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

test("Input adapter applies, replays, rejects stale CAS, and restores", async () => {
  const baselineBytes = new Map([
    ["keymap.json", Buffer.from('{"version":1,"layer":"baseline"}')],
    ["smart_actions.json", Buffer.from('{"smartActions":{}}')],
  ]);
  const files = cloneFileMap(baselineBytes);
  const writerCalls = [];
  const device = configDevice("device-transaction", files);
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [device] },
    deviceKitVersion: "0.1.29",
    configurationWriter: {
      async replaceConfiguration(request) {
        writerCalls.push(request.operation);
        replaceFileMap(files, request.files);
      },
    },
  });
  const baseline = await adapter.snapshotConfig({
    deviceId: "device-transaction",
  });
  files.set("keymap.json", Buffer.from('{"version":1,"layer":"candidate"}'));
  const candidate = await adapter.snapshotConfig({
    deviceId: "device-transaction",
  });
  replaceFileMap(
    files,
    [...baselineBytes].map(([relativePath, bytes]) => ({
      relativePath,
      bytes,
    })),
  );

  const apply = await adapter.applyConfig({
    deviceId: "device-transaction",
    expectedRevision: baseline.revision,
    idempotencyKey: "apply-transaction-1",
    config: candidate,
  });
  assert.equal(apply.changed, true);
  assert.equal(apply.idempotentReplay, false);
  assert.equal(apply.beforeRevision, baseline.revision);
  assert.equal(apply.afterRevision, candidate.revision);
  assert.deepEqual(writerCalls, ["apply"]);

  const replay = await adapter.applyConfig({
    deviceId: "device-transaction",
    expectedRevision: baseline.revision,
    idempotencyKey: "apply-transaction-1",
    config: candidate,
  });
  assert.equal(replay.idempotentReplay, true);
  assert.deepEqual(writerCalls, ["apply"]);

  await assert.rejects(
    adapter.applyConfig({
      deviceId: "device-transaction",
      expectedRevision: baseline.revision,
      idempotencyKey: "apply-transaction-1",
      config: baseline,
    }),
    (error) => error.code === -32602,
  );

  await assert.rejects(
    adapter.applyConfig({
      deviceId: "device-transaction",
      expectedRevision: baseline.revision,
      idempotencyKey: "apply-stale-revision",
      config: candidate,
    }),
    (error) => error.code === -32005,
  );
  const restore = await adapter.restoreConfig({
    deviceId: "device-transaction",
    expectedRevision: candidate.revision,
    idempotencyKey: "restore-transaction-1",
    snapshot: baseline,
  });
  assert.equal(restore.changed, true);
  assert.equal(restore.afterRevision, baseline.revision);
  assert.deepEqual(writerCalls, ["apply", "restore"]);
  const restored = await adapter.snapshotConfig({
    deviceId: "device-transaction",
  });
  assert.equal(restored.revision, baseline.revision);
  const noOp = await adapter.restoreConfig({
    deviceId: "device-transaction",
    expectedRevision: baseline.revision,
    idempotencyKey: "restore-no-op",
    snapshot: baseline,
  });
  assert.equal(noOp.changed, false);
  assert.deepEqual(writerCalls, ["apply", "restore"]);
});

test("Input adapter automatically restores the pre-mutation snapshot", async () => {
  const files = new Map([
    ["keymap.json", Buffer.from('{"version":1,"layer":"baseline"}')],
    ["smart_actions.json", Buffer.from('{"smartActions":{}}')],
  ]);
  const device = configDevice("device-rollback", files);
  const operations = [];
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [device] },
    deviceKitVersion: "0.1.29",
    configurationWriter: {
      async replaceConfiguration(request) {
        operations.push(request.operation);
        if (request.operation === "automatic-rollback") {
          replaceFileMap(files, request.files);
        } else {
          files.set("keymap.json", Buffer.from("corrupt-readback"));
        }
      },
    },
  });
  const baseline = await adapter.snapshotConfig({
    deviceId: "device-rollback",
  });
  files.set("keymap.json", Buffer.from('{"version":1,"layer":"target"}'));
  const candidate = await adapter.snapshotConfig({
    deviceId: "device-rollback",
  });
  replaceFileMap(
    files,
    baseline.files.map((file) => ({
      relativePath: file.relativePath,
      bytes: Buffer.from(file.dataBase64, "base64"),
    })),
  );

  await assert.rejects(
    adapter.applyConfig({
      deviceId: "device-rollback",
      expectedRevision: baseline.revision,
      idempotencyKey: "apply-auto-rollback",
      config: candidate,
    }),
    (error) => error.code === -32008 && error.data?.rollbackPerformed === true,
  );
  assert.deepEqual(operations, ["apply", "automatic-rollback"]);
  const restored = await adapter.snapshotConfig({
    deviceId: "device-rollback",
  });
  assert.equal(restored.revision, baseline.revision);
});

test("Input adapter applies, replays, rejects stale CAS, and restores host settings", async () => {
  let settings = {
    showedAnalyticsPopUp: true,
    analyticsConsented: false,
    smartActionCmdEnabled: false,
  };
  const replacements = [];
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [] },
    deviceKitVersion: "0.1.29",
    hostSettingsAuthority: {
      async readSettings() {
        return { ...settings };
      },
      async replaceSettings(candidate) {
        replacements.push({ ...candidate });
        settings = { ...candidate };
      },
    },
  });
  const baseline = await adapter.snapshotHostSettings();
  assert.equal(baseline.kind, "worklouder-input-host-settings");
  assert.equal(baseline.settings.smartActionCmdEnabled, false);
  assert.match(baseline.revision, /^[0-9a-f]{64}$/);

  const candidate = structuredClone(baseline);
  candidate.settings.smartActionCmdEnabled = true;
  candidate.revision = hostSettingsRevision(candidate.settings);
  const apply = await adapter.applyHostSettings({
    expectedRevision: baseline.revision,
    idempotencyKey: "host-settings-apply-1",
    settings: candidate,
  });
  assert.equal(apply.changed, true);
  assert.equal(apply.idempotentReplay, false);
  assert.equal(apply.afterRevision, candidate.revision);
  assert.deepEqual(replacements, [candidate.settings]);
  assert.equal(settings.showedAnalyticsPopUp, true);
  assert.equal(settings.analyticsConsented, false);

  const replay = await adapter.applyHostSettings({
    expectedRevision: baseline.revision,
    idempotencyKey: "host-settings-apply-1",
    settings: candidate,
  });
  assert.equal(replay.idempotentReplay, true);
  assert.equal(replacements.length, 1);

  await assert.rejects(
    adapter.applyHostSettings({
      expectedRevision: baseline.revision,
      idempotencyKey: "host-settings-stale",
      settings: candidate,
    }),
    (error) => error.code === -32005,
  );
  const restore = await adapter.restoreHostSettings({
    expectedRevision: candidate.revision,
    idempotencyKey: "host-settings-restore-1",
    snapshot: baseline,
  });
  assert.equal(restore.changed, true);
  assert.equal(restore.afterRevision, baseline.revision);
  assert.equal(settings.smartActionCmdEnabled, false);
  assert.equal(replacements.length, 2);

  const noOp = await adapter.restoreHostSettings({
    expectedRevision: baseline.revision,
    idempotencyKey: "host-settings-restore-no-op",
    snapshot: baseline,
  });
  assert.equal(noOp.changed, false);
  assert.equal(replacements.length, 2);

  const tampered = structuredClone(baseline);
  tampered.settings.analyticsConsented = true;
  await assert.rejects(
    adapter.applyHostSettings({
      expectedRevision: baseline.revision,
      idempotencyKey: "host-settings-tampered",
      settings: tampered,
    }),
    (error) => error.code === -32602,
  );
});

test("Input adapter automatically restores host settings after failed readback", async () => {
  const baselineSettings = {
    showedAnalyticsPopUp: false,
    analyticsConsented: true,
    smartActionCmdEnabled: false,
  };
  let settings = { ...baselineSettings };
  let replacementCount = 0;
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [] },
    deviceKitVersion: "0.1.29",
    hostSettingsAuthority: {
      async readSettings() {
        return { ...settings };
      },
      async replaceSettings(candidate) {
        replacementCount += 1;
        settings =
          replacementCount === 1
            ? { ...candidate, analyticsConsented: false }
            : { ...candidate };
      },
    },
  });
  const baseline = await adapter.snapshotHostSettings();
  const candidate = structuredClone(baseline);
  candidate.settings.smartActionCmdEnabled = true;
  candidate.revision = hostSettingsRevision(candidate.settings);

  await assert.rejects(
    adapter.applyHostSettings({
      expectedRevision: baseline.revision,
      idempotencyKey: "host-settings-auto-rollback",
      settings: candidate,
    }),
    (error) => error.code === -32008 && error.data?.rollbackPerformed === true,
  );
  assert.equal(replacementCount, 2);
  assert.deepEqual(settings, baselineSettings);
  const restored = await adapter.snapshotHostSettings();
  assert.equal(restored.revision, baseline.revision);
});

test("Input adapter snapshots a complete preset catalog deterministically", async () => {
  const presets = [
    {
      name: "Fixture",
      id: 9002,
      tags: ["design"],
      layer: { name: "Fixture layer", id: 4 },
    },
  ];
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [] },
    deviceKitVersion: "0.1.29",
    presetCatalogAuthority: {
      async listPresets() {
        return presets;
      },
    },
  });
  const snapshot = await adapter.snapshotPresets();
  assert.equal(snapshot.kind, "worklouder-input-preset-catalog");
  assert.equal(snapshot.revision, presetCatalogRevision(snapshot.presets));
  assert.deepEqual(snapshot.presets, presets);

  presets[0].name = "Changed after snapshot";
  assert.equal(snapshot.presets[0].name, "Fixture");
});

test("Input adapter exposes AppSense focus forwarding and device layer state", async () => {
  const device = configDevice("appsense-device", new Map());
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [device] },
    deviceKitVersion: "0.1.29",
    appsenseRuntimeAuthority: {
      async readState() {
        return {
          collecting: true,
          deviceIds: ["appsense-device", "appsense-device"],
          focusedApp: {
            appName: "Fixture App",
            process: "com.example.fixture",
          },
          lastForwardedApp: {
            appName: "Fixture App",
            process: "com.example.fixture",
          },
        };
      },
    },
  });
  const runtime = await adapter.getAppSenseRuntime({ deviceId: null });
  assert.equal(runtime.kind, "worklouder-input-appsense-runtime");
  assert.equal(runtime.status.selectedProfileIndex, 0);
  assert.equal(runtime.status.selectedLayerIndex, 2);
  assert.equal(runtime.runtime.collecting, true);
  assert.equal(runtime.runtime.selectedDeviceRegistered, true);
  assert.deepEqual(runtime.runtime.deviceIds, ["appsense-device"]);
  assert.equal(runtime.runtime.focusedApp.process, "com.example.fixture");

  const invalid = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [device] },
    deviceKitVersion: "0.1.29",
    appsenseRuntimeAuthority: { readState: async () => ({ collecting: true }) },
  });
  await assert.rejects(
    invalid.getAppSenseRuntime({ deviceId: null }),
    (error) => error.code === -32008,
  );
});

test("Input adapter normalizes Tier 4 permission, firmware, and sanitized log state", async () => {
  const device = configDevice(
    "operations-device",
    new Map([["keymap.json", Buffer.from('{"version":1}')]]),
  );
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [device] },
    deviceKitVersion: "0.1.29",
    permissionsAuthority: {
      readStatus: async () => ({
        platform: "darwin",
        requiredPermission: "input-monitoring",
        granted: true,
        checkedDevicePaths: [],
      }),
    },
    firmwareAuthority: {
      readStatus: async () => ({
        updateAvailable: true,
        release: {
          version: "v0.7.0",
          fetchedAt: 1234,
          changeLog: "Fixture",
          downloadUrl: "https://example.test/firmware.bin",
        },
      }),
    },
    logsAuthority: {
      readLogs: async () => [
        {
          time: "2026-08-03T00:00:00.000Z",
          level: "INFO",
          message:
            "path=/Users/alice/Library token=secret alice@example.test",
        },
      ],
    },
  });

  const permissions = await adapter.getPermissionsStatus({ deviceId: null });
  assert.equal(permissions.permission.requiredPermission, "input-monitoring");
  assert.equal(permissions.permission.granted, true);

  const firmware = await adapter.getFirmwareStatus({ deviceId: null });
  assert.equal(firmware.status.firmwareVersion, "v0.6.0");
  assert.equal(firmware.update.release.version, "v0.7.0");

  const plan = await adapter.getFirmwarePlan({ deviceId: null });
  assert.equal(plan.kind, "worklouder-input-firmware-plan");
  assert.equal(plan.currentFirmwareVersion, "v0.6.0");
  assert.equal(plan.targetRelease.version, "v0.7.0");
  assert.equal(plan.configFileCount, 1);
  assert.equal(plan.ready, false);
  assert.deepEqual(plan.blockers, ["usb-required"]);
  assert.match(plan.revision, /^[0-9a-f]{64}$/);
  assert.equal(plan.phases.length, 7);

  const logs = await adapter.snapshotLogs({ maxEntries: 10 });
  assert.equal(logs.sanitized, true);
  assert.equal(logs.entries[0].level, "info");
  assert.equal(
    logs.entries[0].message,
    "path=$HOME/Library token=<REDACTED> <REDACTED_EMAIL>",
  );
  assert.equal(logs.redactionCount, 3);
});

test("Input adapter delegates firmware update and verifies config-preserving postflight", async () => {
  const files = new Map([["keymap.json", Buffer.from('{"version":1}')]]);
  const device = configDevice("firmware-device", files);
  device.info.isUsbConnection = true;
  let firmwareVersion = "v0.6.0";
  device.rpcService.getFirmwareVersion = async () => firmwareVersion;
  const completedPhases = [
    "backup-configuration",
    "download-input-selected-release",
    "enter-bootloader",
    "flash-with-input-device-programmer",
    "reconnect-original-device",
    "restore-changed-configuration",
    "verify-firmware-and-configuration",
  ];
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [device] },
    deviceKitVersion: "0.1.29",
    firmwareAuthority: {
      readStatus: async () => ({
        updateAvailable: firmwareVersion === "v0.6.0",
        release:
          firmwareVersion === "v0.6.0"
            ? {
                version: "v0.7.0",
                fetchedAt: 1234,
                changeLog: "Fixture",
                downloadUrl: "https://example.test/firmware.bin",
              }
            : null,
      }),
    },
    firmwareOperationsAuthority: {
      async updateFirmware({ release, configurationSnapshot }) {
        assert.equal(release.version, "v0.7.0");
        assert.equal(configurationSnapshot.files.length, 1);
        firmwareVersion = release.version;
        return {
          targetVersion: release.version,
          configurationRestored: true,
          completedPhases,
        };
      },
    },
  });

  const plan = await adapter.getFirmwarePlan({ deviceId: "firmware-device" });
  assert.equal(plan.ready, true);
  const request = {
    deviceId: plan.deviceId,
    expectedRevision: plan.configRevision,
    expectedPlanRevision: plan.revision,
    idempotencyKey: "firmware-update-1",
    plan,
  };
  const updated = await adapter.updateFirmware(request);
  assert.equal(updated.afterFirmwareVersion, "v0.7.0");
  assert.equal(updated.afterConfigRevision, plan.configRevision);
  assert.equal(updated.configurationRestored, true);
  assert.equal(updated.recoveryRequired, false);
  assert.equal(updated.providerOutcome, "completed");
  assert.equal(updated.phases.length, 7);
  const replay = await adapter.updateFirmware(request);
  assert.equal(replay.idempotentReplay, true);
  assert.equal(replay.afterFirmwareVersion, "v0.7.0");
});

test("Input adapter builds a versioned default candidate without mutating configuration", async () => {
  const files = new Map([
    ["keymap.json", Buffer.from('{"layout":"custom"}')],
    ["smart_actions.json", Buffer.from('{"smartActions":{"1":{}}}')],
  ]);
  const device = configDevice("reset-device", files);
  let authorityCalls = 0;
  const adapter = createInputMainAdapter({
    devicesCommManager: { getDevices: () => [device] },
    deviceKitVersion: "0.1.29",
    inputVersion: "0.18.0",
    resetAuthority: {
      async buildDefaultConfiguration({ device: selected, currentConfiguration }) {
        authorityCalls += 1;
        assert.equal(selected, device);
        assert.equal(currentConfiguration.deviceId, "reset-device");
        return {
          layoutVersion: "codex_micro/universal/input-0.18.0/v1",
          files: [
            { relativePath: "keymap.json", bytes: Buffer.from('{"layout":"default"}') },
            { relativePath: "smart_actions.json", bytes: Buffer.from('{"smartActions":{}}') },
          ],
        };
      },
    },
  });

  const bundle = await adapter.getResetPlan({ deviceId: "reset-device" });
  assert.equal(bundle.kind, "worklouder-input-reset-plan-bundle");
  assert.equal(bundle.plan.inputAppVersion, "0.18.0");
  assert.equal(bundle.plan.device.layoutType, "universal");
  assert.equal(bundle.plan.sourceRevision.length, 64);
  assert.equal(bundle.plan.candidateRevision, bundle.candidate.revision);
  assert.notEqual(bundle.plan.sourceRevision, bundle.plan.candidateRevision);
  assert.equal(bundle.plan.candidateFileCount, 2);
  assert.equal(authorityCalls, 1);
  assert.equal(files.get("keymap.json").toString(), '{"layout":"custom"}');
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
  const appSettings = {
    showedAnalyticsPopUp: true,
    analyticsConsented: false,
    smartActionCmdEnabled: false,
    toDTO() {
      return {
        showedAnalyticsPopUp: this.showedAnalyticsPopUp,
        analyticsConsented: this.analyticsConsented,
        smartActionCmdEnabled: this.smartActionCmdEnabled,
      };
    },
  };
  const integration = await installInputCompanionBridge({
    app,
    services: {
      devicesCommManager: {
        getDevices: () => [device],
      },
      applicationService: {
        getAppSettings: () => appSettings,
        saveAppSettings: () => {},
      },
      presetCatalogAuthority: {
        listPresets: () => [],
      },
      appsenseRuntimeAuthority: {
        readState: () => ({
          collecting: true,
          deviceIds: ["device-1"],
          focusedApp: { appName: "Fixture", process: "fixture.bundle" },
          lastForwardedApp: {
            appName: "Fixture",
            process: "fixture.bundle",
          },
        }),
      },
      permissionsAuthority: {
        readStatus: () => ({
          platform: "darwin",
          requiredPermission: "input-monitoring",
          granted: true,
          checkedDevicePaths: [],
        }),
      },
      firmwareAuthority: {
        readStatus: () => ({ updateAvailable: false, release: null }),
      },
      logsAuthority: {
        readLogs: () => [],
      },
    },
    deviceKitVersion: "0.1.29-integration",
    bridgeVersion: "0.1.0-integration",
  });

  assert.equal(integration.inputVersion, "0.18.0-integration");
  assert.equal(integration.socketPath, root + "/worklouderctl-bridge-v1.sock");
  assert.equal(integration.tokenPath, root + "/worklouderctl-bridge-v1.token");
  assert.equal((await stat(integration.socketPath)).mode & 0o777, 0o600);
  assert.equal((await stat(integration.tokenPath)).mode & 0o777, 0o600);
  assert.ok(integration.capabilities.includes("device.config.snapshot.v1"));
  assert.ok(integration.capabilities.includes("device.config.validate.v1"));
  assert.ok(!integration.capabilities.includes("device.config.apply.v1"));
  assert.ok(!integration.capabilities.includes("device.config.restore.v1"));
  assert.ok(
    integration.capabilities.includes("input.host-settings.snapshot.v1"),
  );
  assert.ok(integration.capabilities.includes("input.host-settings.apply.v1"));
  assert.ok(
    integration.capabilities.includes("input.host-settings.restore.v1"),
  );
  assert.ok(integration.capabilities.includes("input.presets.snapshot.v1"));
  assert.ok(integration.capabilities.includes("input.appsense.runtime.v1"));
  assert.ok(integration.capabilities.includes("input.permissions.status.v1"));
  assert.ok(integration.capabilities.includes("input.firmware.status.v1"));
  assert.ok(integration.capabilities.includes("input.firmware.plan.v1"));
  assert.ok(!integration.capabilities.includes("input.firmware.update.v1"));
  assert.ok(!integration.capabilities.includes("input.reset.plan.v1"));
  assert.ok(integration.capabilities.includes("input.logs.snapshot.v1"));
  assert.equal(app.listenerCount("before-quit"), 1);
  await integration.stop();
  assert.equal(app.listenerCount("before-quit"), 0);
  await assert.rejects(stat(integration.socketPath), { code: "ENOENT" });
});

function configDevice(id, files) {
  return {
    id,
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
        return [...files].map(([name, bytes]) => ({
          name,
          size: bytes.length,
          checksum: createHash("sha1").update(bytes).digest("hex"),
        }));
      },
      async readFileChunked(path) {
        return files.get(path);
      },
    },
  };
}

function cloneFileMap(files) {
  return new Map([...files].map(([path, bytes]) => [path, Buffer.from(bytes)]));
}

function replaceFileMap(target, files) {
  target.clear();
  for (const file of files) {
    target.set(file.relativePath, Buffer.from(file.bytes));
  }
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

import { createHash } from "node:crypto";
import { BridgeError } from "./input-main-bridge.mjs";

export const CONFIG_SNAPSHOT_SCHEMA_VERSION = 1;
export const CONFIG_SNAPSHOT_KIND = "worklouder-input-config-snapshot";
export const CONFIG_REVISION_ALGORITHM =
  "sha256:path-u32be-path-bytes-size-u64be-content-v1";
export const HOST_SETTINGS_SCHEMA_VERSION = 1;
export const HOST_SETTINGS_KIND = "worklouder-input-host-settings";
export const HOST_SETTINGS_REVISION_ALGORITHM =
  "sha256:input-host-settings-three-booleans-v1";
export const PRESET_CATALOG_SCHEMA_VERSION = 1;
export const PRESET_CATALOG_KIND = "worklouder-input-preset-catalog";
export const PRESET_CATALOG_REVISION_ALGORITHM =
  "sha256:recursive-key-sorted-presets-json-v1";
export const FIRMWARE_PLAN_SCHEMA_VERSION = 1;
export const FIRMWARE_PLAN_KIND = "worklouder-input-firmware-plan";
export const FIRMWARE_PLAN_REVISION_ALGORITHM =
  "sha256:recursive-key-sorted-firmware-plan-body-v1";

const MAX_CONFIG_FILES = 4096;
const MAX_CONFIG_FILE_BYTES = 16 * 1024 * 1024;
const MAX_CONFIG_TOTAL_BYTES = 32 * 1024 * 1024;
const MAX_PRESETS = 1024;
const MAX_PRESET_CATALOG_BYTES = 32 * 1024 * 1024;
const MAX_LOG_ENTRIES = 5000;
const MAX_LOG_MESSAGE_BYTES = 8192;

export function createInputMainAdapter({
  devicesCommManager,
  deviceKitVersion,
  configurationWriter,
  hostSettingsAuthority,
  presetCatalogAuthority,
  appsenseRuntimeAuthority,
  permissionsAuthority,
  firmwareAuthority,
  logsAuthority,
}) {
  if (
    !devicesCommManager ||
    typeof devicesCommManager.getDevices !== "function"
  ) {
    throw new TypeError("devicesCommManager.getDevices is required");
  }
  if (typeof deviceKitVersion !== "string" || deviceKitVersion.length === 0) {
    throw new TypeError("deviceKitVersion is required");
  }
  if (
    configurationWriter !== undefined &&
    (!configurationWriter ||
      typeof configurationWriter.replaceConfiguration !== "function")
  ) {
    throw new TypeError(
      "configurationWriter.replaceConfiguration must be a function",
    );
  }
  if (
    hostSettingsAuthority !== undefined &&
    (!hostSettingsAuthority ||
      typeof hostSettingsAuthority.readSettings !== "function" ||
      typeof hostSettingsAuthority.replaceSettings !== "function")
  ) {
    throw new TypeError(
      "hostSettingsAuthority must provide readSettings and replaceSettings",
    );
  }
  if (
    presetCatalogAuthority !== undefined &&
    (!presetCatalogAuthority ||
      typeof presetCatalogAuthority.listPresets !== "function")
  ) {
    throw new TypeError("presetCatalogAuthority.listPresets must be a function");
  }
  if (
    appsenseRuntimeAuthority !== undefined &&
    (!appsenseRuntimeAuthority ||
      typeof appsenseRuntimeAuthority.readState !== "function")
  ) {
    throw new TypeError("appsenseRuntimeAuthority.readState must be a function");
  }
  if (
    permissionsAuthority !== undefined &&
    (!permissionsAuthority ||
      typeof permissionsAuthority.readStatus !== "function")
  ) {
    throw new TypeError("permissionsAuthority.readStatus must be a function");
  }
  if (
    firmwareAuthority !== undefined &&
    (!firmwareAuthority || typeof firmwareAuthority.readStatus !== "function")
  ) {
    throw new TypeError("firmwareAuthority.readStatus must be a function");
  }
  if (
    logsAuthority !== undefined &&
    (!logsAuthority || typeof logsAuthority.readLogs !== "function")
  ) {
    throw new TypeError("logsAuthority.readLogs must be a function");
  }
  const idempotencyCache = new Map();
  const hostSettingsIdempotencyCache = new Map();

  const selectDevice = (deviceId) => {
    const devices = devicesCommManager
      .getDevices()
      .filter((device) => device.isConnected());
    if (deviceId !== null && deviceId !== undefined) {
      const selected = devices.find(
        (device) => String(device.id) === String(deviceId),
      );
      if (!selected) {
        throw new BridgeError(-32004, "device not found", { deviceId });
      }
      return selected;
    }
    const codexMicro = devices.filter(
      (device) => String(device.info.deviceType) === "codex_micro",
    );
    if (codexMicro.length !== 1) {
      throw new BridgeError(
        -32004,
        "expected exactly one connected Codex Micro, found " +
          codexMicro.length,
      );
    }
    return codexMicro[0];
  };

  const common = async (device) => {
    const [firmwareVersion, status] = await Promise.all([
      device.rpcService.getFirmwareVersion(),
      device.rpcService.getDeviceStatus(),
    ]);
    if (status.firmwareVersion && status.firmwareVersion !== firmwareVersion) {
      throw new BridgeError(
        -32008,
        "sys.version and device.status firmware versions differed",
      );
    }
    return {
      deviceKitVersion,
      device: publicDevice(device.info),
      status: {
        firmwareVersion: status.firmwareVersion ?? firmwareVersion,
        selectedProfileIndex: optionalNumber(
          status.selectedProfileIndex ?? status.selected_profile_index,
        ),
        selectedLayerIndex: optionalNumber(
          status.selectedLayerIndex ?? status.selected_layer_index,
        ),
        batteryPercentage: optionalNumber(
          status.batteryPercentage ?? status.battery_percentage,
        ),
        isCharging:
          status.isCharging === undefined
            ? (status.is_charging ?? null)
            : status.isCharging,
      },
      warnings: [],
    };
  };

  const captureConfigSnapshot = async (device) => {
    const [status, firstListing] = await Promise.all([
      common(device),
      device.rpcService.getFileList({ recursive: true }),
    ]);
    const listed = normalizeFileList(firstListing);
    if (listed.length === 0) {
      throw new BridgeError(-32008, "device configuration contained no files");
    }
    const files = [];
    let totalBytes = 0;
    for (const file of listed) {
      const bytes = await device.rpcService.readFileChunked(file.relativePath);
      if (!Buffer.isBuffer(bytes)) {
        throw new BridgeError(-32008, "device returned no file bytes");
      }
      if (bytes.length !== file.size) {
        throw new BridgeError(
          -32006,
          "device configuration changed during snapshot",
        );
      }
      const sha1 = createHash("sha1").update(bytes).digest("hex");
      if (sha1 !== file.deviceChecksumSha1) {
        throw new BridgeError(
          -32006,
          "device configuration changed during snapshot",
        );
      }
      totalBytes += bytes.length;
      if (
        bytes.length > MAX_CONFIG_FILE_BYTES ||
        totalBytes > MAX_CONFIG_TOTAL_BYTES
      ) {
        throw new BridgeError(
          -32008,
          "device configuration exceeded snapshot limits",
        );
      }
      files.push({
        ...file,
        sha256: createHash("sha256").update(bytes).digest("hex"),
        dataBase64: bytes.toString("base64"),
      });
    }
    const secondListing = normalizeFileList(
      await device.rpcService.getFileList({ recursive: true }),
    );
    if (listingIdentity(listed) !== listingIdentity(secondListing)) {
      throw new BridgeError(
        -32006,
        "device configuration changed during snapshot",
      );
    }
    return {
      schemaVersion: CONFIG_SNAPSHOT_SCHEMA_VERSION,
      kind: CONFIG_SNAPSHOT_KIND,
      revisionAlgorithm: CONFIG_REVISION_ALGORITHM,
      revision: configRevision(files),
      deviceId: String(device.id),
      ...status,
      files,
    };
  };

  const runMutation = async ({
    operation,
    deviceId,
    expectedRevision,
    idempotencyKey,
    candidate,
  }) => {
    const target = validateConfigSnapshot(candidate);
    const expected = safeSha256(expectedRevision, "expected revision");
    const device = selectDevice(deviceId);
    if (String(device.id) !== target.deviceId) {
      throw new BridgeError(
        -32602,
        "configuration deviceId did not match request",
      );
    }
    const requestDigest = mutationRequestDigest({
      operation,
      deviceId: String(device.id),
      expectedRevision: expected,
      targetRevision: target.revision,
    });
    const cached = idempotencyCache.get(idempotencyKey);
    if (cached) {
      if (cached.requestDigest !== requestDigest) {
        throw new BridgeError(
          -32602,
          "idempotency key was reused with a different mutation",
        );
      }
      return { ...cached.result, idempotentReplay: true };
    }

    const before = await captureConfigSnapshot(device);
    if (before.revision !== expected) {
      throw new BridgeError(-32005, "device revision conflict", {
        expectedRevision: expected,
        liveRevision: before.revision,
      });
    }
    if (before.revision === target.revision) {
      const result = mutationResult({
        operation,
        idempotencyKey,
        deviceId: String(device.id),
        beforeRevision: before.revision,
        afterRevision: before.revision,
        target,
        changed: false,
      });
      cacheMutation(idempotencyCache, idempotencyKey, requestDigest, result);
      return result;
    }

    const backup = validateConfigSnapshot(before);
    try {
      await replaceConfiguration(configurationWriter, {
        device,
        files: target.files,
        operation,
        targetRevision: target.revision,
      });
      const after = await captureConfigSnapshot(device);
      if (after.revision !== target.revision) {
        throw new Error(
          `configuration readback revision ${after.revision} did not match ${target.revision}`,
        );
      }
      const result = mutationResult({
        operation,
        idempotencyKey,
        deviceId: String(device.id),
        beforeRevision: before.revision,
        afterRevision: after.revision,
        target,
        changed: true,
      });
      cacheMutation(idempotencyCache, idempotencyKey, requestDigest, result);
      return result;
    } catch (mutationError) {
      let rollbackRevision = null;
      try {
        await replaceConfiguration(configurationWriter, {
          device,
          files: backup.files,
          operation: "automatic-rollback",
          targetRevision: backup.revision,
        });
        const restored = await captureConfigSnapshot(device);
        rollbackRevision = restored.revision;
        if (rollbackRevision !== backup.revision) {
          throw new Error(
            `rollback readback revision ${rollbackRevision} did not match ${backup.revision}`,
          );
        }
      } catch (rollbackError) {
        throw new BridgeError(
          -32008,
          "configuration mutation and rollback failed",
          {
            operation,
            beforeRevision: before.revision,
            targetRevision: target.revision,
            rollbackRevision,
            mutationError: errorMessage(mutationError),
            rollbackError: errorMessage(rollbackError),
          },
        );
      }
      throw new BridgeError(
        -32008,
        "configuration mutation failed and was rolled back",
        {
          operation,
          beforeRevision: before.revision,
          targetRevision: target.revision,
          rollbackRevision,
          rollbackPerformed: true,
          mutationError: errorMessage(mutationError),
        },
      );
    }
  };

  const captureHostSettings = async () => {
    const settings = normalizeHostSettings(
      await hostSettingsAuthority.readSettings(),
    );
    return hostSettingsSnapshot(settings);
  };

  const runHostSettingsMutation = async ({
    operation,
    expectedRevision,
    idempotencyKey,
    candidate,
  }) => {
    const target = validateHostSettingsSnapshot(candidate);
    const expected = safeSha256(expectedRevision, "expected revision");
    const requestDigest = mutationRequestDigest({
      operation: `host-settings-${operation}`,
      deviceId: "input-host-settings",
      expectedRevision: expected,
      targetRevision: target.revision,
    });
    const cached = hostSettingsIdempotencyCache.get(idempotencyKey);
    if (cached) {
      if (cached.requestDigest !== requestDigest) {
        throw new BridgeError(
          -32602,
          "idempotency key was reused with a different mutation",
        );
      }
      return { ...cached.result, idempotentReplay: true };
    }

    const before = await captureHostSettings();
    if (before.revision !== expected) {
      throw new BridgeError(-32005, "host settings revision conflict", {
        expectedRevision: expected,
        liveRevision: before.revision,
      });
    }
    if (before.revision === target.revision) {
      const result = hostSettingsMutationResult({
        operation,
        idempotencyKey,
        beforeRevision: before.revision,
        afterRevision: before.revision,
        targetRevision: target.revision,
        changed: false,
      });
      cacheMutation(
        hostSettingsIdempotencyCache,
        idempotencyKey,
        requestDigest,
        result,
      );
      return result;
    }

    try {
      await hostSettingsAuthority.replaceSettings({ ...target.settings });
      const after = await captureHostSettings();
      if (after.revision !== target.revision) {
        throw new Error(
          `host settings readback revision ${after.revision} did not match ${target.revision}`,
        );
      }
      const result = hostSettingsMutationResult({
        operation,
        idempotencyKey,
        beforeRevision: before.revision,
        afterRevision: after.revision,
        targetRevision: target.revision,
        changed: true,
      });
      cacheMutation(
        hostSettingsIdempotencyCache,
        idempotencyKey,
        requestDigest,
        result,
      );
      return result;
    } catch (mutationError) {
      let rollbackRevision = null;
      try {
        await hostSettingsAuthority.replaceSettings({ ...before.settings });
        const restored = await captureHostSettings();
        rollbackRevision = restored.revision;
        if (rollbackRevision !== before.revision) {
          throw new Error(
            `host settings rollback revision ${rollbackRevision} did not match ${before.revision}`,
          );
        }
      } catch (rollbackError) {
        throw new BridgeError(
          -32008,
          "host settings mutation and rollback failed",
          {
            operation,
            beforeRevision: before.revision,
            targetRevision: target.revision,
            rollbackRevision,
            mutationError: errorMessage(mutationError),
            rollbackError: errorMessage(rollbackError),
          },
        );
      }
      throw new BridgeError(
        -32008,
        "host settings mutation failed and was rolled back",
        {
          operation,
          beforeRevision: before.revision,
          targetRevision: target.revision,
          rollbackRevision,
          rollbackPerformed: true,
          mutationError: errorMessage(mutationError),
        },
      );
    }
  };

  const adapter = {
    async listDevices() {
      return {
        deviceKitVersion,
        devices: devicesCommManager.getDevices().map((device) => ({
          id: String(device.id),
          connected: Boolean(device.isConnected()),
          device: publicDevice(device.info),
        })),
      };
    },

    async getDeviceStatus({ deviceId = null }) {
      const device = selectDevice(deviceId);
      return common(device);
    },

    async listFiles({ deviceId = null, path, recursive = false }) {
      const device = selectDevice(deviceId);
      const [snapshot, files] = await Promise.all([
        common(device),
        device.rpcService.getFileList({
          path: path ?? undefined,
          recursive: Boolean(recursive),
        }),
      ]);
      if (!Array.isArray(files)) {
        throw new BridgeError(-32008, "device returned a non-array file list");
      }
      return {
        ...snapshot,
        files: files.map((file) => ({
          relativePath: safeRelativePath(file.name),
          size: safeSize(file.size),
          deviceChecksumSha1: safeSha1(file.checksum),
        })),
      };
    },

    async readFile({ deviceId = null, path }) {
      const device = selectDevice(deviceId);
      const relativePath = safeRelativePath(path);
      const bytes = await device.rpcService.readFileChunked(relativePath);
      if (!Buffer.isBuffer(bytes)) {
        throw new BridgeError(-32008, "device returned no file bytes");
      }
      return {
        relativePath,
        size: bytes.length,
        deviceChecksumSha1: createHash("sha1").update(bytes).digest("hex"),
        dataBase64: bytes.toString("base64"),
      };
    },

    async snapshotConfig({ deviceId = null }) {
      return captureConfigSnapshot(selectDevice(deviceId));
    },

    async validateConfig({
      deviceId = null,
      snapshot,
      expectedRevision = null,
    }) {
      const validation = validateConfigSnapshot(snapshot);
      if (
        deviceId !== null &&
        deviceId !== undefined &&
        String(deviceId) !== validation.deviceId
      ) {
        throw new BridgeError(
          -32602,
          "snapshot deviceId did not match request",
        );
      }
      let liveRevision = null;
      if (expectedRevision !== null && expectedRevision !== undefined) {
        const expected = safeSha256(expectedRevision, "expected revision");
        const live = await captureConfigSnapshot(selectDevice(deviceId));
        liveRevision = live.revision;
        if (
          live.deviceId !== validation.deviceId ||
          liveRevision !== expected
        ) {
          throw new BridgeError(-32005, "device revision conflict", {
            expectedRevision: expected,
            liveRevision,
            snapshotDeviceId: validation.deviceId,
            liveDeviceId: live.deviceId,
          });
        }
      }
      return {
        schemaVersion: CONFIG_SNAPSHOT_SCHEMA_VERSION,
        kind: "worklouder-input-config-validation",
        valid: true,
        revision: validation.revision,
        liveRevision,
        fileCount: validation.fileCount,
        totalBytes: validation.totalBytes,
      };
    },
  };
  if (configurationWriter) {
    adapter.applyConfig = async ({
      deviceId,
      expectedRevision,
      idempotencyKey,
      config,
    }) =>
      runMutation({
        operation: "apply",
        deviceId,
        expectedRevision,
        idempotencyKey,
        candidate: config,
      });
    adapter.restoreConfig = async ({
      deviceId,
      expectedRevision,
      idempotencyKey,
      snapshot,
    }) =>
      runMutation({
        operation: "restore",
        deviceId,
        expectedRevision,
        idempotencyKey,
        candidate: snapshot,
      });
  }
  if (hostSettingsAuthority) {
    adapter.snapshotHostSettings = async () => captureHostSettings();
    adapter.applyHostSettings = async ({
      expectedRevision,
      idempotencyKey,
      settings,
    }) =>
      runHostSettingsMutation({
        operation: "apply",
        expectedRevision,
        idempotencyKey,
        candidate: settings,
      });
    adapter.restoreHostSettings = async ({
      expectedRevision,
      idempotencyKey,
      snapshot,
    }) =>
      runHostSettingsMutation({
        operation: "restore",
        expectedRevision,
        idempotencyKey,
        candidate: snapshot,
      });
  }
  if (presetCatalogAuthority) {
    adapter.snapshotPresets = async () =>
      presetCatalogSnapshot(await presetCatalogAuthority.listPresets());
  }
  if (appsenseRuntimeAuthority) {
    adapter.getAppSenseRuntime = async ({ deviceId = null }) => {
      const device = selectDevice(deviceId);
      const [runtime, status] = await Promise.all([
        appsenseRuntimeAuthority.readState({ device }),
        common(device),
      ]);
      return {
        schemaVersion: 1,
        kind: "worklouder-input-appsense-runtime",
        ...status,
        runtime: normalizeAppSenseRuntime(runtime, String(device.id)),
      };
    };
  }
  if (permissionsAuthority) {
    adapter.getPermissionsStatus = async ({ deviceId = null }) => {
      const device = selectDevice(deviceId);
      const status = normalizePermissionsStatus(
        await permissionsAuthority.readStatus({ device }),
      );
      return {
        schemaVersion: 1,
        kind: "worklouder-input-permissions-status",
        deviceKitVersion,
        device: publicDevice(device.info),
        permission: status,
      };
    };
  }
  if (firmwareAuthority) {
    adapter.getFirmwareStatus = async ({ deviceId = null }) => {
      const device = selectDevice(deviceId);
      const [deviceStatus, update] = await Promise.all([
        common(device),
        firmwareAuthority.readStatus({ device }),
      ]);
      return {
        schemaVersion: 1,
        kind: "worklouder-input-firmware-status",
        ...deviceStatus,
        update: normalizeFirmwareStatus(update),
      };
    };
    adapter.getFirmwarePlan = async ({ deviceId = null }) => {
      const device = selectDevice(deviceId);
      const [deviceStatus, update, config] = await Promise.all([
        common(device),
        firmwareAuthority.readStatus({ device }),
        captureConfigSnapshot(device),
      ]);
      return firmwarePlan({
        deviceId: String(device.id),
        deviceStatus,
        update: normalizeFirmwareStatus(update),
        config,
      });
    };
  }
  if (logsAuthority) {
    adapter.snapshotLogs = async ({ maxEntries = MAX_LOG_ENTRIES }) => {
      const limit = safeInteger(maxEntries, "maxEntries", 1, MAX_LOG_ENTRIES);
      const source = await logsAuthority.readLogs({ maxEntries: limit });
      return normalizeLogsSnapshot(source, limit);
    };
  }
  return adapter;
}

function normalizePermissionsStatus(status) {
  if (!status || typeof status !== "object" || Array.isArray(status)) {
    throw new BridgeError(-32008, "Input permission status was invalid");
  }
  const platform = safeBoundedString(status.platform, "permission platform", 32);
  const requiredPermission = safeBoundedString(
    status.requiredPermission,
    "required permission",
    64,
  );
  if (![
    "input-monitoring",
    "hid-read-write",
    "none",
  ].includes(requiredPermission)) {
    throw new BridgeError(-32008, "Input required permission was unknown");
  }
  if (typeof status.granted !== "boolean") {
    throw new BridgeError(-32008, "Input permission grant was invalid");
  }
  const checkedDevicePaths = Array.isArray(status.checkedDevicePaths)
    ? [...new Set(status.checkedDevicePaths.map((path) =>
      safeBoundedString(path, "checked device path", 4096),
    ))].sort()
    : [];
  if (checkedDevicePaths.length > 256) {
    throw new BridgeError(-32008, "Input permission path list was too large");
  }
  return { platform, requiredPermission, granted: status.granted, checkedDevicePaths };
}

function normalizeFirmwareStatus(status) {
  if (!status || typeof status !== "object" || Array.isArray(status)) {
    throw new BridgeError(-32008, "Input firmware status was invalid");
  }
  if (
    status.updateAvailable !== null &&
    typeof status.updateAvailable !== "boolean"
  ) {
    throw new BridgeError(-32008, "Input firmware update availability was invalid");
  }
  let release = null;
  if (status.release !== null && status.release !== undefined) {
    if (typeof status.release !== "object" || Array.isArray(status.release)) {
      throw new BridgeError(-32008, "Input firmware release was invalid");
    }
    release = {
      version: safeBoundedString(status.release.version, "firmware version", 128),
      fetchedAt: safeInteger(
        status.release.fetchedAt,
        "firmware fetchedAt",
        0,
        Number.MAX_SAFE_INTEGER,
      ),
      downloadUrl: safeHttpUrl(status.release.downloadUrl, "firmware download URL"),
      changeLog: optionalBoundedString(
        status.release.changeLog,
        "firmware change log",
        1024 * 1024,
      ),
    };
  }
  if (status.updateAvailable === true && release === null) {
    throw new BridgeError(-32008, "Input reported an update without release metadata");
  }
  return { updateAvailable: status.updateAvailable, release };
}

function firmwarePlan({ deviceId, deviceStatus, update, config }) {
  const blockers = [];
  if (update.updateAvailable === null) {
    blockers.push("update-availability-unknown");
  } else if (update.updateAvailable === false) {
    blockers.push("no-update-available");
  }
  if (update.release === null) {
    blockers.push("release-unavailable");
  }
  if (!deviceStatus.device.isUsbConnection) {
    blockers.push("usb-required");
  }
  const body = {
    deviceId,
    deviceKitVersion: deviceStatus.deviceKitVersion,
    device: deviceStatus.device,
    currentFirmwareVersion: safeBoundedString(
      deviceStatus.status.firmwareVersion,
      "current firmware version",
      128,
    ),
    targetRelease: update.release,
    configRevision: safeSha256(config.revision, "configuration revision"),
    configFileCount: safeInteger(
      config.files.length,
      "configuration file count",
      1,
      MAX_CONFIG_FILES,
    ),
    ready: blockers.length === 0,
    blockers,
    phases: [
      "backup-configuration",
      "download-input-selected-release",
      "enter-bootloader",
      "flash-with-input-device-programmer",
      "reconnect-original-device",
      "restore-changed-configuration",
      "verify-firmware-and-configuration",
    ],
  };
  return {
    schemaVersion: FIRMWARE_PLAN_SCHEMA_VERSION,
    kind: FIRMWARE_PLAN_KIND,
    revisionAlgorithm: FIRMWARE_PLAN_REVISION_ALGORITHM,
    revision: firmwarePlanRevision(body),
    ...body,
  };
}

export function firmwarePlanRevision(body) {
  return createHash("sha256")
    .update("worklouder-input-firmware-plan-revision-v1\0", "utf8")
    .update(canonicalJson(body), "utf8")
    .digest("hex");
}

function normalizeLogsSnapshot(source, limit) {
  if (!Array.isArray(source)) {
    throw new BridgeError(-32008, "Input logs were not an array");
  }
  const selected = source.slice(-limit);
  let redactionCount = 0;
  const entries = selected.map((entry) => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
      throw new BridgeError(-32008, "Input log entry was invalid");
    }
    const message = redactLogMessage(
      safeBoundedString(entry.message, "log message", MAX_LOG_MESSAGE_BYTES),
    );
    redactionCount += message.redactionCount;
    return {
      time: safeBoundedString(entry.time, "log timestamp", 128),
      level: safeBoundedString(entry.level, "log level", 32).toLowerCase(),
      message: message.value,
    };
  });
  return {
    schemaVersion: 1,
    kind: "worklouder-input-log-snapshot",
    sanitized: true,
    sourceEntryCount: source.length,
    truncated: source.length > selected.length,
    redactionCount,
    entries,
  };
}

function redactLogMessage(value) {
  let redactionCount = 0;
  const replace = (pattern, replacement) => {
    value = value.replace(pattern, (...args) => {
      redactionCount += 1;
      return typeof replacement === "function" ? replacement(...args) : replacement;
    });
  };
  replace(/\/Users\/[^/\s"']+/g, "$HOME");
  replace(/[A-Za-z]:\\Users\\[^\\\s"']+/g, "$HOME");
  replace(/\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi, "<REDACTED_EMAIL>");
  replace(/\bBearer\s+[^\s,;]+/gi, "Bearer <REDACTED>");
  replace(
    /\b(authorization|token|api[_-]?key|password|secret|device[_-]?id|serial)(\s*[:=]\s*)([^\s,;]+)/gi,
    (_match, name, separator) => `${name}${separator}<REDACTED>`,
  );
  replace(
    /([?&](?:token|api[_-]?key|password|secret)=)[^&#\s]+/gi,
    "$1<REDACTED>",
  );
  replace(
    /\b[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b/gi,
    "<REDACTED_UUID>",
  );
  return { value, redactionCount };
}

function safeBoundedString(value, label, maxBytes) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    Buffer.byteLength(value, "utf8") > maxBytes ||
    value.includes("\0")
  ) {
    throw new BridgeError(-32008, `${label} was invalid`);
  }
  return value;
}

function optionalBoundedString(value, label, maxBytes) {
  if (value === null || value === undefined) {
    return null;
  }
  if (
    typeof value !== "string" ||
    Buffer.byteLength(value, "utf8") > maxBytes ||
    value.includes("\0")
  ) {
    throw new BridgeError(-32008, `${label} was invalid`);
  }
  return value;
}

function safeInteger(value, label, minimum, maximum) {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new BridgeError(-32602, `${label} was invalid`);
  }
  return value;
}

function safeHttpUrl(value, label) {
  const url = safeBoundedString(value, label, 8192);
  let parsed;
  try {
    parsed = new URL(url);
  } catch {
    throw new BridgeError(-32008, `${label} was invalid`);
  }
  if (!["https:", "http:"].includes(parsed.protocol)) {
    throw new BridgeError(-32008, `${label} used an unsupported protocol`);
  }
  return parsed.toString();
}

function normalizeAppSenseRuntime(runtime, selectedDeviceId) {
  if (!runtime || typeof runtime !== "object" || Array.isArray(runtime)) {
    throw new BridgeError(-32008, "AppSense runtime state was invalid");
  }
  if (typeof runtime.collecting !== "boolean") {
    throw new BridgeError(-32008, "AppSense collecting state was invalid");
  }
  if (
    !Array.isArray(runtime.deviceIds) ||
    runtime.deviceIds.length > 256 ||
    runtime.deviceIds.some(
      (id) => typeof id !== "string" || id.length === 0 || id.length > 256,
    )
  ) {
    throw new BridgeError(-32008, "AppSense device IDs were invalid");
  }
  const deviceIds = [...new Set(runtime.deviceIds)].sort();
  return {
    collecting: runtime.collecting,
    selectedDeviceRegistered: deviceIds.includes(selectedDeviceId),
    deviceIds,
    focusedApp: normalizeFocusedApp(runtime.focusedApp),
    lastForwardedApp: normalizeFocusedApp(runtime.lastForwardedApp),
  };
}

function normalizeFocusedApp(app) {
  if (app === null || app === undefined) return null;
  if (!app || typeof app !== "object" || Array.isArray(app)) {
    throw new BridgeError(-32008, "AppSense focused application was invalid");
  }
  const normalized = {};
  for (const field of ["appName", "process", "path"]) {
    const value = app[field];
    if (value !== undefined && value !== null) {
      if (typeof value !== "string" || value.length > 4096 || value.includes("\0")) {
        throw new BridgeError(
          -32008,
          `AppSense focused application ${field} was invalid`,
        );
      }
      normalized[field] = value;
    }
  }
  if (!normalized.appName && !normalized.process && !normalized.path) {
    throw new BridgeError(-32008, "AppSense focused application was empty");
  }
  return normalized;
}

async function replaceConfiguration(
  configurationWriter,
  { device, files, operation, targetRevision },
) {
  await configurationWriter.replaceConfiguration({
    device,
    operation,
    targetRevision,
    files: files.map((file) => ({
      relativePath: file.relativePath,
      bytes: Buffer.from(file.bytes),
    })),
  });
}

function mutationResult({
  operation,
  idempotencyKey,
  deviceId,
  beforeRevision,
  afterRevision,
  target,
  changed,
}) {
  return {
    schemaVersion: 1,
    kind: "worklouder-input-config-mutation",
    operation,
    idempotencyKey,
    idempotentReplay: false,
    changed,
    rollbackPerformed: false,
    deviceId,
    beforeRevision,
    afterRevision,
    targetRevision: target.revision,
    fileCount: target.fileCount,
    totalBytes: target.totalBytes,
  };
}

function hostSettingsSnapshot(settings) {
  const normalized = normalizeHostSettings(settings);
  return {
    schemaVersion: HOST_SETTINGS_SCHEMA_VERSION,
    kind: HOST_SETTINGS_KIND,
    revisionAlgorithm: HOST_SETTINGS_REVISION_ALGORITHM,
    revision: hostSettingsRevision(normalized),
    settings: normalized,
  };
}

function validateHostSettingsSnapshot(snapshot) {
  if (!snapshot || typeof snapshot !== "object" || Array.isArray(snapshot)) {
    throw new BridgeError(-32602, "host settings snapshot must be an object");
  }
  if (
    snapshot.schemaVersion !== HOST_SETTINGS_SCHEMA_VERSION ||
    snapshot.kind !== HOST_SETTINGS_KIND ||
    snapshot.revisionAlgorithm !== HOST_SETTINGS_REVISION_ALGORITHM
  ) {
    throw new BridgeError(-32602, "host settings snapshot header was invalid");
  }
  const settings = normalizeHostSettings(snapshot.settings, -32602);
  const revision = hostSettingsRevision(settings);
  if (safeSha256(snapshot.revision, "host settings revision") !== revision) {
    throw new BridgeError(
      -32602,
      "host settings revision did not match content",
    );
  }
  return { revision, settings };
}

function normalizeHostSettings(settings, code = -32008) {
  if (!settings || typeof settings !== "object" || Array.isArray(settings)) {
    throw new BridgeError(code, "Input returned invalid host settings");
  }
  const normalized = {};
  for (const field of [
    "showedAnalyticsPopUp",
    "analyticsConsented",
    "smartActionCmdEnabled",
  ]) {
    if (typeof settings[field] !== "boolean") {
      throw new BridgeError(code, `host settings ${field} was not boolean`);
    }
    normalized[field] = settings[field];
  }
  return normalized;
}

export function hostSettingsRevision(settings) {
  return createHash("sha256")
    .update("worklouder-input-host-settings-revision-v1\0", "utf8")
    .update(
      Buffer.from([
        settings.showedAnalyticsPopUp ? 1 : 0,
        settings.analyticsConsented ? 1 : 0,
        settings.smartActionCmdEnabled ? 1 : 0,
      ]),
    )
    .digest("hex");
}

function presetCatalogSnapshot(presets) {
  const normalized = normalizePresetCatalog(presets);
  return {
    schemaVersion: PRESET_CATALOG_SCHEMA_VERSION,
    kind: PRESET_CATALOG_KIND,
    revisionAlgorithm: PRESET_CATALOG_REVISION_ALGORITHM,
    revision: presetCatalogRevision(normalized),
    presets: normalized,
  };
}

function normalizePresetCatalog(presets) {
  if (!Array.isArray(presets) || presets.length > MAX_PRESETS) {
    throw new BridgeError(-32008, "Input returned an invalid preset catalog");
  }
  for (const preset of presets) {
    if (!preset || typeof preset !== "object" || Array.isArray(preset)) {
      throw new BridgeError(-32008, "Input returned an invalid preset entry");
    }
  }
  let normalized;
  try {
    normalized = JSON.parse(JSON.stringify(presets));
  } catch (error) {
    throw new BridgeError(-32008, "Input preset catalog was not JSON", {
      error: errorMessage(error),
    });
  }
  const bytes = Buffer.byteLength(canonicalJson(normalized), "utf8");
  if (bytes > MAX_PRESET_CATALOG_BYTES) {
    throw new BridgeError(-32008, "Input preset catalog exceeded size limits");
  }
  return normalized;
}

export function presetCatalogRevision(presets) {
  return createHash("sha256")
    .update("worklouder-input-preset-catalog-revision-v1\0", "utf8")
    .update(canonicalJson(presets), "utf8")
    .digest("hex");
}

function canonicalJson(value) {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(",")}]`;
  }
  if (value !== null && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function hostSettingsMutationResult({
  operation,
  idempotencyKey,
  beforeRevision,
  afterRevision,
  targetRevision,
  changed,
}) {
  return {
    schemaVersion: 1,
    kind: "worklouder-input-host-settings-mutation",
    operation,
    idempotencyKey,
    idempotentReplay: false,
    changed,
    rollbackPerformed: false,
    beforeRevision,
    afterRevision,
    targetRevision,
  };
}

function mutationRequestDigest({
  operation,
  deviceId,
  expectedRevision,
  targetRevision,
}) {
  return createHash("sha256")
    .update(operation, "utf8")
    .update("\0")
    .update(deviceId, "utf8")
    .update("\0")
    .update(expectedRevision, "utf8")
    .update("\0")
    .update(targetRevision, "utf8")
    .digest("hex");
}

function cacheMutation(cache, idempotencyKey, requestDigest, result) {
  cache.set(idempotencyKey, { requestDigest, result });
  if (cache.size > 1024) {
    cache.delete(cache.keys().next().value);
  }
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function normalizeFileList(files) {
  if (!Array.isArray(files)) {
    throw new BridgeError(-32008, "device returned a non-array file list");
  }
  if (files.length > MAX_CONFIG_FILES) {
    throw new BridgeError(-32008, "device configuration had too many files");
  }
  const normalized = files.map((file) => ({
    relativePath: safeRelativePath(file.name),
    size: safeSize(file.size),
    deviceChecksumSha1: safeSha1(file.checksum),
  }));
  normalized.sort((left, right) =>
    comparePaths(left.relativePath, right.relativePath),
  );
  for (let index = 1; index < normalized.length; index += 1) {
    if (normalized[index - 1].relativePath === normalized[index].relativePath) {
      throw new BridgeError(-32008, "device returned duplicate file paths");
    }
  }
  return normalized;
}

function listingIdentity(files) {
  return JSON.stringify(
    files.map((file) => [
      file.relativePath,
      file.size,
      file.deviceChecksumSha1,
    ]),
  );
}

function validateConfigSnapshot(snapshot) {
  if (!snapshot || typeof snapshot !== "object" || Array.isArray(snapshot)) {
    throw new BridgeError(-32602, "snapshot must be an object");
  }
  if (
    snapshot.schemaVersion !== CONFIG_SNAPSHOT_SCHEMA_VERSION ||
    snapshot.kind !== CONFIG_SNAPSHOT_KIND ||
    snapshot.revisionAlgorithm !== CONFIG_REVISION_ALGORITHM ||
    typeof snapshot.deviceId !== "string" ||
    snapshot.deviceId.length === 0 ||
    !Array.isArray(snapshot.files) ||
    snapshot.files.length === 0 ||
    snapshot.files.length > MAX_CONFIG_FILES
  ) {
    throw new BridgeError(-32602, "snapshot header or file list was invalid");
  }
  const seen = new Set();
  let totalBytes = 0;
  const files = snapshot.files.map((file) => {
    if (!file || typeof file !== "object" || Array.isArray(file)) {
      throw new BridgeError(-32602, "snapshot file entry was invalid");
    }
    const relativePath = safeRelativePath(file.relativePath, -32602);
    if (seen.has(relativePath)) {
      throw new BridgeError(-32602, "snapshot contained duplicate file paths");
    }
    seen.add(relativePath);
    const bytes = decodeCanonicalBase64(file.dataBase64);
    const size = safeSize(file.size, -32602);
    totalBytes += bytes.length;
    if (
      size !== bytes.length ||
      bytes.length > MAX_CONFIG_FILE_BYTES ||
      totalBytes > MAX_CONFIG_TOTAL_BYTES
    ) {
      throw new BridgeError(-32602, "snapshot file size was invalid");
    }
    const deviceChecksumSha1 = safeSha1(file.deviceChecksumSha1, -32602);
    if (createHash("sha1").update(bytes).digest("hex") !== deviceChecksumSha1) {
      throw new BridgeError(
        -32602,
        "snapshot file SHA-1 did not match content",
      );
    }
    const sha256 = safeSha256(file.sha256, "snapshot file SHA-256");
    if (createHash("sha256").update(bytes).digest("hex") !== sha256) {
      throw new BridgeError(
        -32602,
        "snapshot file SHA-256 did not match content",
      );
    }
    return { relativePath, bytes };
  });
  const revision = configRevision(files);
  if (safeSha256(snapshot.revision, "snapshot revision") !== revision) {
    throw new BridgeError(-32602, "snapshot revision did not match content");
  }
  return {
    deviceId: snapshot.deviceId,
    revision,
    fileCount: files.length,
    totalBytes,
    files,
  };
}

function configRevision(files) {
  const hash = createHash("sha256");
  hash.update("worklouder-input-config-revision-v1\0", "utf8");
  const sorted = [...files].sort((left, right) =>
    comparePaths(left.relativePath, right.relativePath),
  );
  for (const file of sorted) {
    const path = Buffer.from(file.relativePath, "utf8");
    const pathLength = Buffer.alloc(4);
    pathLength.writeUInt32BE(path.length);
    const content = file.bytes ?? decodeCanonicalBase64(file.dataBase64);
    const contentLength = Buffer.alloc(8);
    contentLength.writeBigUInt64BE(BigInt(content.length));
    hash.update(pathLength);
    hash.update(path);
    hash.update(contentLength);
    hash.update(content);
  }
  return hash.digest("hex");
}

function decodeCanonicalBase64(value) {
  if (
    typeof value !== "string" ||
    value.length % 4 !== 0 ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(
      value,
    )
  ) {
    throw new BridgeError(
      -32602,
      "snapshot file content was not canonical base64",
    );
  }
  const bytes = Buffer.from(value, "base64");
  if (bytes.toString("base64") !== value) {
    throw new BridgeError(
      -32602,
      "snapshot file content was not canonical base64",
    );
  }
  return bytes;
}

function publicDevice(device) {
  const connectionType =
    device.connectionType === 0
      ? "serial"
      : device.connectionType === 1
        ? "hid"
        : String(device.connectionType);
  return {
    devicePid: String(device.devicePid),
    deviceType: String(device.deviceType),
    layoutType: String(device.layoutType),
    connectionType,
    isUsbConnection: Boolean(device.isUsbConnection),
  };
}

function optionalNumber(value) {
  if (value === null || value === undefined) {
    return null;
  }
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < 0) {
    throw new BridgeError(-32008, "device returned an invalid numeric status");
  }
  return number;
}

function safeSize(value, code = -32008) {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < 0) {
    throw new BridgeError(code, "device returned an invalid file size");
  }
  return number;
}

function safeSha1(value, code = -32008) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/i.test(value)) {
    throw new BridgeError(code, "device returned an invalid SHA-1");
  }
  return value.toLowerCase();
}

function safeSha256(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/i.test(value)) {
    throw new BridgeError(-32602, label + " was invalid");
  }
  return value.toLowerCase();
}

function safeRelativePath(value, code = -32008) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    Buffer.byteLength(value, "utf8") > 4096 ||
    value.includes("\0") ||
    value.startsWith("/") ||
    value.startsWith("\\")
  ) {
    throw new BridgeError(code, "device returned an invalid file path");
  }
  const portable = value.replaceAll("\\", "/");
  const parts = portable.split("/");
  if (parts.some((part) => part === "" || part === "." || part === "..")) {
    throw new BridgeError(code, "device returned an unsafe file path");
  }
  return parts.join("/");
}

function comparePaths(left, right) {
  return Buffer.compare(Buffer.from(left, "utf8"), Buffer.from(right, "utf8"));
}

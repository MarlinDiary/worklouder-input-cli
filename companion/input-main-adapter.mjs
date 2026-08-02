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
export const RESET_PLAN_SCHEMA_VERSION = 1;
export const RESET_PLAN_KIND = "worklouder-input-reset-plan";
export const RESET_PLAN_REVISION_ALGORITHM =
  "sha256:recursive-key-sorted-reset-plan-body-v1";
export const RECOVERY_PLAN_SCHEMA_VERSION = 1;
export const RECOVERY_PLAN_KIND = "worklouder-input-recovery-plan";
export const RECOVERY_PLAN_REVISION_ALGORITHM =
  "sha256:recursive-key-sorted-recovery-plan-body-v1";

const MAX_CONFIG_FILES = 4096;
const MAX_CONFIG_FILE_BYTES = 16 * 1024 * 1024;
const MAX_CONFIG_TOTAL_BYTES = 32 * 1024 * 1024;
const MAX_PRESETS = 1024;
const MAX_PRESET_CATALOG_BYTES = 32 * 1024 * 1024;
const MAX_LOG_ENTRIES = 5000;
const MAX_LOG_MESSAGE_BYTES = 8192;
const FIRMWARE_PHASES = [
  "backup-configuration",
  "download-input-selected-release",
  "enter-bootloader",
  "flash-with-input-device-programmer",
  "reconnect-original-device",
  "restore-changed-configuration",
  "verify-firmware-and-configuration",
];
const RECOVERY_PHASES = [
  "detect-input-bootloader",
  "validate-input-selected-release",
  "recover-with-input-device-programmer",
  "reconnect-original-device",
  "restore-exact-configuration",
  "verify-firmware-and-configuration",
];
const RECOVERY_PROVIDER_PHASES = RECOVERY_PHASES.slice(0, 4);

export function createInputMainAdapter({
  devicesCommManager,
  deviceKitVersion,
  inputVersion,
  configurationWriter,
  hostSettingsAuthority,
  presetCatalogAuthority,
  appsenseRuntimeAuthority,
  permissionsAuthority,
  firmwareAuthority,
  firmwareOperationsAuthority,
  resetAuthority,
  recoveryAuthority,
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
    firmwareOperationsAuthority !== undefined &&
    (!firmwareOperationsAuthority ||
      typeof firmwareOperationsAuthority.updateFirmware !== "function")
  ) {
    throw new TypeError(
      "firmwareOperationsAuthority.updateFirmware must be a function",
    );
  }
  if (firmwareOperationsAuthority && !firmwareAuthority) {
    throw new TypeError(
      "firmwareOperationsAuthority requires firmwareAuthority.readStatus",
    );
  }
  if (
    resetAuthority !== undefined &&
    (!resetAuthority ||
      typeof resetAuthority.buildDefaultConfiguration !== "function")
  ) {
    throw new TypeError(
      "resetAuthority.buildDefaultConfiguration must be a function",
    );
  }
  if (
    resetAuthority &&
    (typeof inputVersion !== "string" || inputVersion.length === 0)
  ) {
    throw new TypeError("inputVersion is required with resetAuthority");
  }
  if (
    recoveryAuthority !== undefined &&
    (!recoveryAuthority ||
      typeof recoveryAuthority.readStatus !== "function" ||
      typeof recoveryAuthority.recoverFirmware !== "function")
  ) {
    throw new TypeError(
      "recoveryAuthority must provide readStatus and recoverFirmware",
    );
  }
  if (
    recoveryAuthority &&
    (typeof inputVersion !== "string" || inputVersion.length === 0)
  ) {
    throw new TypeError("inputVersion is required with recoveryAuthority");
  }
  if (
    logsAuthority !== undefined &&
    (!logsAuthority || typeof logsAuthority.readLogs !== "function")
  ) {
    throw new TypeError("logsAuthority.readLogs must be a function");
  }
  const idempotencyCache = new Map();
  const hostSettingsIdempotencyCache = new Map();
  const firmwareIdempotencyCache = new Map();
  const recoveryIdempotencyCache = new Map();

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

  const captureFirmwarePlan = async (device) => {
    const [config, update] = await Promise.all([
      captureConfigSnapshot(device),
      firmwareAuthority.readStatus({ device }),
    ]);
    return {
      config,
      plan: firmwarePlan({
        deviceId: String(device.id),
        deviceStatus: {
          deviceKitVersion: config.deviceKitVersion,
          device: config.device,
          status: config.status,
        },
        update: normalizeFirmwareStatus(update),
        config,
      }),
    };
  };

  const captureResetPlan = async (device) => {
    const current = await captureConfigSnapshot(device);
    const generated = await resetAuthority.buildDefaultConfiguration({
      device,
      currentConfiguration: current,
    });
    if (!generated || typeof generated !== "object" || Array.isArray(generated)) {
      throw new BridgeError(-32008, "Input reset authority returned no candidate");
    }
    const layoutVersion = safeBoundedString(
      generated.layoutVersion,
      "reset layout version",
      128,
    );
    const candidate = defaultConfigSnapshot(current, generated.files);
    const body = {
      inputAppVersion: inputVersion,
      deviceId: current.deviceId,
      deviceKitVersion: current.deviceKitVersion,
      device: current.device,
      firmwareVersion: safeBoundedString(
        current.status.firmwareVersion,
        "reset firmware version",
        128,
      ),
      layoutVersion,
      strategy: "input-default-layout",
      sourceRevision: current.revision,
      candidateRevision: candidate.revision,
      candidateFileCount: candidate.files.length,
    };
    const plan = {
      schemaVersion: RESET_PLAN_SCHEMA_VERSION,
      kind: RESET_PLAN_KIND,
      revisionAlgorithm: RESET_PLAN_REVISION_ALGORITHM,
      revision: resetPlanRevision(body),
      ...body,
    };
    return {
      schemaVersion: 1,
      kind: "worklouder-input-reset-plan-bundle",
      plan,
      candidate,
    };
  };

  const captureRecoveryPlan = async (suppliedConfiguration) => {
    const configuration = validateConfigSnapshot(suppliedConfiguration);
    const device = normalizeFirmwarePlanDevice(suppliedConfiguration.device);
    const deviceKitVersion = safeBoundedString(
      suppliedConfiguration.deviceKitVersion,
      "recovery device kit version",
      128,
    );
    const previousFirmwareVersion = safeBoundedString(
      suppliedConfiguration.status?.firmwareVersion,
      "recovery previous firmware version",
      128,
    );
    const status = normalizeRecoveryStatus(
      await recoveryAuthority.readStatus({
        configurationSnapshot: suppliedConfiguration,
      }),
    );
    const blockers = [];
    if (!status.bootloaderDetected) {
      blockers.push("input-bootloader-not-detected");
    }
    if (!status.targetRelease) {
      blockers.push("input-selected-release-unavailable");
    }
    const body = {
      inputAppVersion: inputVersion,
      deviceId: configuration.deviceId,
      deviceKitVersion,
      device,
      previousFirmwareVersion,
      targetRelease: status.targetRelease,
      bootloader: status.bootloader,
      configurationRevision: configuration.revision,
      configurationFileCount: configuration.files.length,
      phases: [...RECOVERY_PHASES],
      blockers,
      ready: blockers.length === 0,
    };
    return {
      schemaVersion: RECOVERY_PLAN_SCHEMA_VERSION,
      kind: RECOVERY_PLAN_KIND,
      revisionAlgorithm: RECOVERY_PLAN_REVISION_ALGORITHM,
      revision: recoveryPlanRevision(body),
      ...body,
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

  const runFirmwareUpdate = async ({
    deviceId,
    expectedRevision,
    expectedPlanRevision,
    idempotencyKey,
    plan: suppliedPlan,
  }) => {
    const plan = normalizeFirmwarePlan(suppliedPlan);
    const expected = safeSha256(expectedRevision, "expected revision");
    const expectedPlan = safeSha256(
      expectedPlanRevision,
      "expected firmware plan revision",
    );
    if (plan.revision !== expectedPlan || plan.configRevision !== expected) {
      throw new BridgeError(-32602, "firmware plan expectations differed");
    }
    if (String(deviceId) !== plan.deviceId) {
      throw new BridgeError(-32602, "firmware plan deviceId differed");
    }
    const targetVersion = plan.targetRelease?.version ?? "";
    const requestDigest = mutationRequestDigest({
      operation: "firmware-update",
      deviceId: plan.deviceId,
      expectedRevision: expected,
      targetRevision: plan.revision,
    });
    const cached = firmwareIdempotencyCache.get(idempotencyKey);
    if (cached) {
      if (cached.requestDigest !== requestDigest) {
        throw new BridgeError(
          -32602,
          "idempotency key was reused with a different firmware update",
        );
      }
      return { ...cached.result, idempotentReplay: true };
    }

    const device = selectDevice(deviceId);
    const live = await captureFirmwarePlan(device);
    if (live.plan.revision !== expectedPlan) {
      throw new BridgeError(-32005, "firmware plan revision conflict", {
        expectedPlanRevision: expectedPlan,
        livePlanRevision: live.plan.revision,
      });
    }
    if (!live.plan.ready || targetVersion.length === 0) {
      throw new BridgeError(-32007, "firmware plan was not ready", {
        blockers: live.plan.blockers,
      });
    }
    if (live.config.revision !== expected) {
      throw new BridgeError(-32005, "firmware configuration revision conflict", {
        expectedRevision: expected,
        liveRevision: live.config.revision,
      });
    }

    let providerOutcome = "completed";
    try {
      const providerResult = await firmwareOperationsAuthority.updateFirmware({
        device,
        operation: "update",
        plan: live.plan,
        release: live.plan.targetRelease,
        configurationSnapshot: live.config,
      });
      validateFirmwareOperationResult(providerResult, targetVersion);
    } catch (providerError) {
      providerOutcome = "postflight-confirmed";
      try {
        const observed = await captureConfigSnapshot(selectDevice(deviceId));
        if (
          observed.status.firmwareVersion !== targetVersion ||
          observed.revision !== expected
        ) {
          throw new Error("postflight did not match the update target");
        }
      } catch (postflightError) {
        throw new BridgeError(-32008, "firmware update required recovery", {
          recoveryRequired: true,
          deviceId: plan.deviceId,
          planRevision: plan.revision,
          beforeFirmwareVersion: plan.currentFirmwareVersion,
          targetFirmwareVersion: targetVersion,
          beforeConfigRevision: expected,
          providerError: errorMessage(providerError),
          postflightError: errorMessage(postflightError),
        });
      }
    }

    const after = await captureConfigSnapshot(selectDevice(deviceId));
    if (
      after.status.firmwareVersion !== targetVersion ||
      after.revision !== expected
    ) {
      throw new BridgeError(-32008, "firmware update postflight failed", {
        recoveryRequired: true,
        targetFirmwareVersion: targetVersion,
        afterFirmwareVersion: after.status.firmwareVersion ?? null,
        beforeConfigRevision: expected,
        afterConfigRevision: after.revision,
      });
    }
    const result = firmwareMutationResult({
      idempotencyKey,
      plan,
      targetVersion,
      after,
      providerOutcome,
    });
    cacheMutation(firmwareIdempotencyCache, idempotencyKey, requestDigest, result);
    return result;
  };

  const runFirmwareRecovery = async ({
    expectedPlanRevision,
    idempotencyKey,
    plan: suppliedPlan,
    configuration: suppliedConfiguration,
  }) => {
    const plan = normalizeRecoveryPlan(suppliedPlan);
    const configuration = validateConfigSnapshot(suppliedConfiguration);
    const expectedPlan = safeSha256(
      expectedPlanRevision,
      "expected recovery plan revision",
    );
    if (
      plan.revision !== expectedPlan ||
      plan.inputAppVersion !== inputVersion ||
      plan.configurationRevision !== configuration.revision ||
      plan.deviceId !== configuration.deviceId
    ) {
      throw new BridgeError(-32602, "recovery plan expectations differed");
    }
    const targetVersion = plan.targetRelease?.version ?? "";
    const requestDigest = mutationRequestDigest({
      operation: "firmware-recovery",
      deviceId: plan.deviceId,
        expectedRevision: configuration.revision,
      targetRevision: plan.revision,
    });
    const cached = recoveryIdempotencyCache.get(idempotencyKey);
    if (cached) {
      if (cached.requestDigest !== requestDigest) {
        throw new BridgeError(
          -32602,
          "idempotency key was reused with a different firmware recovery",
        );
      }
      return { ...cached.result, idempotentReplay: true };
    }

    const livePlan = await captureRecoveryPlan(suppliedConfiguration);
    if (livePlan.revision !== expectedPlan) {
      throw new BridgeError(-32005, "recovery plan revision conflict", {
        expectedPlanRevision: expectedPlan,
        livePlanRevision: livePlan.revision,
      });
    }
    if (!livePlan.ready || targetVersion.length === 0) {
      throw new BridgeError(-32007, "firmware recovery was not ready", {
        blockers: livePlan.blockers,
      });
    }

    let providerOutcome = "completed";
    try {
      const providerResult = await recoveryAuthority.recoverFirmware({
        operation: "recover",
        plan: livePlan,
        release: livePlan.targetRelease,
        bootloader: livePlan.bootloader,
      });
      validateRecoveryOperationResult(providerResult, targetVersion);
    } catch (providerError) {
      providerOutcome = "postflight-confirmed";
      try {
        const observed = await captureConfigSnapshot(selectDevice(plan.deviceId));
        if (observed.status.firmwareVersion !== targetVersion) {
          throw new Error("postflight did not find recovered target firmware");
        }
      } catch (postflightError) {
        throw new BridgeError(-32008, "firmware recovery required intervention", {
          recoveryRequired: true,
          deviceId: plan.deviceId,
          planRevision: plan.revision,
          targetFirmwareVersion: targetVersion,
          configurationRevision: configuration.revision,
          providerError: errorMessage(providerError),
          postflightError: errorMessage(postflightError),
        });
      }
    }

    const recovered = await captureConfigSnapshot(selectDevice(plan.deviceId));
    if (recovered.status.firmwareVersion !== targetVersion) {
      throw new BridgeError(-32008, "firmware recovery postflight failed", {
        recoveryRequired: true,
        targetFirmwareVersion: targetVersion,
        afterFirmwareVersion: recovered.status.firmwareVersion ?? null,
      });
    }
    let restore;
    try {
      restore = await runMutation({
        operation: "recovery-restore",
        deviceId: plan.deviceId,
        expectedRevision: recovered.revision,
        idempotencyKey: idempotencyKey + ":configuration",
        candidate: suppliedConfiguration,
      });
    } catch (restoreError) {
      throw new BridgeError(-32008, "firmware recovered but configuration restore failed", {
        recoveryRequired: true,
        deviceId: plan.deviceId,
        planRevision: plan.revision,
        targetFirmwareVersion: targetVersion,
        recoveredConfigRevision: recovered.revision,
        targetConfigRevision: configuration.revision,
        restoreError: errorMessage(restoreError),
      });
    }
    const after = await captureConfigSnapshot(selectDevice(plan.deviceId));
    if (
      after.status.firmwareVersion !== targetVersion ||
      after.revision !== configuration.revision
    ) {
      throw new BridgeError(-32008, "recovery configuration postflight failed", {
        recoveryRequired: true,
        targetFirmwareVersion: targetVersion,
        afterFirmwareVersion: after.status.firmwareVersion ?? null,
        targetConfigRevision: configuration.revision,
        afterConfigRevision: after.revision,
      });
    }
    const result = {
      schemaVersion: 1,
      kind: "worklouder-input-recovery-mutation",
      operation: "recover",
      idempotencyKey,
      idempotentReplay: false,
      changed: true,
      providerOutcome,
      recoveryRequired: false,
      planRevision: plan.revision,
      deviceId: plan.deviceId,
      beforeFirmwareVersion: plan.previousFirmwareVersion,
      afterFirmwareVersion: after.status.firmwareVersion,
      targetFirmwareVersion: targetVersion,
      beforeConfigRevision: configuration.revision,
      recoveredConfigRevision: recovered.revision,
      afterConfigRevision: after.revision,
      configurationRestored: restore.afterRevision === configuration.revision,
      phases: RECOVERY_PHASES.map((name) => ({ name, status: "completed" })),
    };
    cacheMutation(recoveryIdempotencyCache, idempotencyKey, requestDigest, result);
    return result;
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
      return (await captureFirmwarePlan(device)).plan;
    };
  }
  if (firmwareOperationsAuthority) {
    adapter.updateFirmware = runFirmwareUpdate;
  }
  if (resetAuthority) {
    adapter.getResetPlan = async ({ deviceId = null }) =>
      captureResetPlan(selectDevice(deviceId));
  }
  if (recoveryAuthority) {
    adapter.getRecoveryPlan = async ({ configuration }) =>
      captureRecoveryPlan(configuration);
    adapter.recoverFirmware = runFirmwareRecovery;
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

function defaultConfigSnapshot(current, suppliedFiles) {
  if (
    !Array.isArray(suppliedFiles) ||
    suppliedFiles.length === 0 ||
    suppliedFiles.length > MAX_CONFIG_FILES
  ) {
    throw new BridgeError(-32008, "Input reset authority returned invalid files");
  }
  const seen = new Set();
  let totalBytes = 0;
  const files = suppliedFiles.map((file) => {
    if (!file || typeof file !== "object" || Array.isArray(file)) {
      throw new BridgeError(-32008, "Input reset file was invalid");
    }
    const relativePath = safeRelativePath(file.relativePath);
    if (seen.has(relativePath)) {
      throw new BridgeError(-32008, "Input reset files contained duplicate paths");
    }
    seen.add(relativePath);
    const bytes = Buffer.isBuffer(file.bytes)
      ? Buffer.from(file.bytes)
      : file.bytes instanceof Uint8Array
        ? Buffer.from(file.bytes)
        : null;
    if (!bytes) {
      throw new BridgeError(-32008, "Input reset file contained no bytes");
    }
    totalBytes += bytes.length;
    if (
      bytes.length > MAX_CONFIG_FILE_BYTES ||
      totalBytes > MAX_CONFIG_TOTAL_BYTES
    ) {
      throw new BridgeError(-32008, "Input reset files exceeded snapshot limits");
    }
    return {
      relativePath,
      size: bytes.length,
      deviceChecksumSha1: createHash("sha1").update(bytes).digest("hex"),
      sha256: createHash("sha256").update(bytes).digest("hex"),
      dataBase64: bytes.toString("base64"),
      bytes,
    };
  });
  files.sort((left, right) => comparePaths(left.relativePath, right.relativePath));
  const serializable = files.map(({ bytes: _bytes, ...file }) => file);
  const candidate = {
    ...current,
    revision: configRevision(files),
    files: serializable,
  };
  validateConfigSnapshot(candidate);
  return candidate;
}

export function resetPlanRevision(body) {
  return createHash("sha256")
    .update("worklouder-input-reset-plan-revision-v1\0", "utf8")
    .update(canonicalJson(body), "utf8")
    .digest("hex");
}

function normalizeRecoveryStatus(status) {
  if (!status || typeof status !== "object" || Array.isArray(status)) {
    throw new BridgeError(-32008, "Input recovery status was invalid");
  }
  exactKeys(
    status,
    ["bootloaderDetected", "bootloader", "targetRelease"],
    "recovery status",
  );
  if (typeof status.bootloaderDetected !== "boolean") {
    throw new BridgeError(-32008, "Input bootloader detection was invalid");
  }
  let bootloader = null;
  if (status.bootloader !== null && status.bootloader !== undefined) {
    if (typeof status.bootloader !== "object" || Array.isArray(status.bootloader)) {
      throw new BridgeError(-32008, "Input bootloader identity was invalid");
    }
    exactKeys(
      status.bootloader,
      ["transport", "identifier", "deviceType"],
      "bootloader identity",
    );
    bootloader = {
      transport: safeBoundedString(
        status.bootloader.transport,
        "bootloader transport",
        64,
      ),
      identifier: safeBoundedString(
        status.bootloader.identifier,
        "bootloader identifier",
        512,
      ),
      deviceType: safeBoundedString(
        status.bootloader.deviceType,
        "bootloader device type",
        128,
      ),
    };
  }
  if (status.bootloaderDetected !== (bootloader !== null)) {
    throw new BridgeError(-32008, "Input bootloader identity was inconsistent");
  }
  const targetRelease = normalizeFirmwareStatus({
    updateAvailable: status.targetRelease === null ? false : true,
    release: status.targetRelease,
  }).release;
  return {
    bootloaderDetected: status.bootloaderDetected,
    bootloader,
    targetRelease,
  };
}

export function recoveryPlanRevision(body) {
  return createHash("sha256")
    .update("worklouder-input-recovery-plan-revision-v1\0", "utf8")
    .update(canonicalJson(body), "utf8")
    .digest("hex");
}

function normalizeRecoveryPlan(plan) {
  if (!plan || typeof plan !== "object" || Array.isArray(plan)) {
    throw new BridgeError(-32602, "recovery plan must be an object");
  }
  exactKeys(
    plan,
    [
      "schemaVersion",
      "kind",
      "revisionAlgorithm",
      "revision",
      "inputAppVersion",
      "deviceId",
      "deviceKitVersion",
      "device",
      "previousFirmwareVersion",
      "targetRelease",
      "bootloader",
      "configurationRevision",
      "configurationFileCount",
      "phases",
      "blockers",
      "ready",
    ],
    "recovery plan",
  );
  if (
    plan.schemaVersion !== RECOVERY_PLAN_SCHEMA_VERSION ||
    plan.kind !== RECOVERY_PLAN_KIND ||
    plan.revisionAlgorithm !== RECOVERY_PLAN_REVISION_ALGORITHM
  ) {
    throw new BridgeError(-32602, "recovery plan header was invalid");
  }
  const status = normalizeRecoveryStatus({
    bootloaderDetected: plan.bootloader !== null,
    bootloader: plan.bootloader,
    targetRelease: plan.targetRelease,
  });
  const blockers = normalizeRecoveryBlockers(plan.blockers);
  if (
    typeof plan.ready !== "boolean" ||
    plan.ready !== (blockers.length === 0) ||
    (status.bootloader === null) !==
      blockers.includes("input-bootloader-not-detected") ||
    (status.targetRelease === null) !==
      blockers.includes("input-selected-release-unavailable") ||
    !Array.isArray(plan.phases) ||
    canonicalJson(plan.phases) !== canonicalJson(RECOVERY_PHASES)
  ) {
    throw new BridgeError(-32602, "recovery plan readiness was inconsistent");
  }
  const body = {
    inputAppVersion: safeBoundedString(
      plan.inputAppVersion,
      "recovery Input version",
      128,
    ),
    deviceId: safeBoundedString(plan.deviceId, "recovery deviceId", 512),
    deviceKitVersion: safeBoundedString(
      plan.deviceKitVersion,
      "recovery device kit version",
      128,
    ),
    device: normalizeFirmwarePlanDevice(plan.device),
    previousFirmwareVersion: safeBoundedString(
      plan.previousFirmwareVersion,
      "recovery previous firmware version",
      128,
    ),
    targetRelease: status.targetRelease,
    bootloader: status.bootloader,
    configurationRevision: safeSha256(
      plan.configurationRevision,
      "recovery configuration revision",
    ),
    configurationFileCount: safeInteger(
      plan.configurationFileCount,
      "recovery configuration file count",
      1,
      MAX_CONFIG_FILES,
    ),
    phases: [...RECOVERY_PHASES],
    blockers,
    ready: plan.ready,
  };
  const revision = safeSha256(plan.revision, "recovery plan revision");
  if (revision !== recoveryPlanRevision(body)) {
    throw new BridgeError(-32602, "recovery plan revision did not match content");
  }
  return {
    schemaVersion: RECOVERY_PLAN_SCHEMA_VERSION,
    kind: RECOVERY_PLAN_KIND,
    revisionAlgorithm: RECOVERY_PLAN_REVISION_ALGORITHM,
    revision,
    ...body,
  };
}

function normalizeRecoveryBlockers(blockers) {
  const known = new Set([
    "input-bootloader-not-detected",
    "input-selected-release-unavailable",
  ]);
  if (
    !Array.isArray(blockers) ||
    blockers.length > known.size ||
    new Set(blockers).size !== blockers.length ||
    blockers.some((blocker) => !known.has(blocker))
  ) {
    throw new BridgeError(-32602, "recovery blockers were invalid");
  }
  return [...blockers];
}

function validateRecoveryOperationResult(result, targetVersion) {
  if (!result || typeof result !== "object" || Array.isArray(result)) {
    throw new Error("firmware recovery authority returned no result");
  }
  if (
    result.targetVersion !== targetVersion ||
    !Array.isArray(result.completedPhases) ||
    canonicalJson(result.completedPhases) !==
      canonicalJson(RECOVERY_PROVIDER_PHASES)
  ) {
    throw new Error("firmware recovery authority returned an invalid result");
  }
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
    phases: [...FIRMWARE_PHASES],
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

function normalizeFirmwarePlan(plan) {
  if (!plan || typeof plan !== "object" || Array.isArray(plan)) {
    throw new BridgeError(-32602, "firmware plan must be an object");
  }
  exactKeys(
    plan,
    [
      "schemaVersion",
      "kind",
      "revisionAlgorithm",
      "revision",
      "deviceId",
      "deviceKitVersion",
      "device",
      "currentFirmwareVersion",
      "targetRelease",
      "configRevision",
      "configFileCount",
      "ready",
      "blockers",
      "phases",
    ],
    "firmware plan",
  );
  if (
    plan.schemaVersion !== FIRMWARE_PLAN_SCHEMA_VERSION ||
    plan.kind !== FIRMWARE_PLAN_KIND ||
    plan.revisionAlgorithm !== FIRMWARE_PLAN_REVISION_ALGORITHM
  ) {
    throw new BridgeError(-32602, "firmware plan header was invalid");
  }
  const device = normalizeFirmwarePlanDevice(plan.device);
  const targetRelease = normalizeFirmwareStatus({
    updateAvailable: plan.targetRelease === null ? false : true,
    release: plan.targetRelease,
  }).release;
  const blockers = normalizeFirmwareBlockers(plan.blockers);
  if (
    typeof plan.ready !== "boolean" ||
    plan.ready !== (blockers.length === 0) ||
    (targetRelease === null) !== blockers.includes("release-unavailable") ||
    (!device.isUsbConnection) !== blockers.includes("usb-required") ||
    (blockers.includes("update-availability-unknown") &&
      blockers.includes("no-update-available"))
  ) {
    throw new BridgeError(-32602, "firmware plan blockers were inconsistent");
  }
  if (
    !Array.isArray(plan.phases) ||
    canonicalJson(plan.phases) !== canonicalJson(FIRMWARE_PHASES)
  ) {
    throw new BridgeError(-32602, "firmware plan phases were invalid");
  }
  const body = {
    deviceId: safeBoundedString(plan.deviceId, "firmware plan deviceId", 512),
    deviceKitVersion: safeBoundedString(
      plan.deviceKitVersion,
      "firmware plan device kit version",
      128,
    ),
    device,
    currentFirmwareVersion: safeBoundedString(
      plan.currentFirmwareVersion,
      "current firmware version",
      128,
    ),
    targetRelease,
    configRevision: safeSha256(plan.configRevision, "configuration revision"),
    configFileCount: safeInteger(
      plan.configFileCount,
      "configuration file count",
      1,
      MAX_CONFIG_FILES,
    ),
    ready: plan.ready,
    blockers,
    phases: [...FIRMWARE_PHASES],
  };
  const revision = safeSha256(plan.revision, "firmware plan revision");
  if (revision !== firmwarePlanRevision(body)) {
    throw new BridgeError(-32602, "firmware plan revision did not match content");
  }
  return {
    schemaVersion: FIRMWARE_PLAN_SCHEMA_VERSION,
    kind: FIRMWARE_PLAN_KIND,
    revisionAlgorithm: FIRMWARE_PLAN_REVISION_ALGORITHM,
    revision,
    ...body,
  };
}

function normalizeFirmwarePlanDevice(device) {
  if (!device || typeof device !== "object" || Array.isArray(device)) {
    throw new BridgeError(-32602, "firmware plan device was invalid");
  }
  exactKeys(
    device,
    [
      "devicePid",
      "deviceType",
      "layoutType",
      "connectionType",
      "isUsbConnection",
    ],
    "firmware plan device",
  );
  if (typeof device.isUsbConnection !== "boolean") {
    throw new BridgeError(-32602, "firmware plan USB state was invalid");
  }
  return {
    devicePid: safeBoundedString(device.devicePid, "device PID", 128),
    deviceType: safeBoundedString(device.deviceType, "device type", 128),
    layoutType: safeBoundedString(device.layoutType, "layout type", 128),
    connectionType: safeBoundedString(
      device.connectionType,
      "connection type",
      128,
    ),
    isUsbConnection: device.isUsbConnection,
  };
}

function normalizeFirmwareBlockers(blockers) {
  const known = new Set([
    "update-availability-unknown",
    "no-update-available",
    "release-unavailable",
    "usb-required",
  ]);
  if (
    !Array.isArray(blockers) ||
    blockers.length > known.size ||
    new Set(blockers).size !== blockers.length ||
    blockers.some((blocker) => !known.has(blocker))
  ) {
    throw new BridgeError(-32602, "firmware plan blockers were invalid");
  }
  return [...blockers];
}

function validateFirmwareOperationResult(result, targetVersion) {
  if (!result || typeof result !== "object" || Array.isArray(result)) {
    throw new Error("firmware update authority returned no result");
  }
  if (
    result.targetVersion !== targetVersion ||
    result.configurationRestored !== true ||
    !Array.isArray(result.completedPhases) ||
    canonicalJson(result.completedPhases) !== canonicalJson(FIRMWARE_PHASES)
  ) {
    throw new Error("firmware update authority returned an invalid result");
  }
}

function firmwareMutationResult({
  idempotencyKey,
  plan,
  targetVersion,
  after,
  providerOutcome,
}) {
  return {
    schemaVersion: 1,
    kind: "worklouder-input-firmware-mutation",
    operation: "update",
    idempotencyKey,
    idempotentReplay: false,
    changed: plan.currentFirmwareVersion !== targetVersion,
    deviceId: plan.deviceId,
    planRevision: plan.revision,
    beforeFirmwareVersion: plan.currentFirmwareVersion,
    afterFirmwareVersion: after.status.firmwareVersion,
    targetFirmwareVersion: targetVersion,
    beforeConfigRevision: plan.configRevision,
    afterConfigRevision: after.revision,
    configurationRestored: true,
    providerOutcome,
    recoveryRequired: false,
    phases: FIRMWARE_PHASES.map((name) => ({ name, status: "completed" })),
  };
}

function exactKeys(value, expected, label) {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (canonicalJson(actual) !== canonicalJson(wanted)) {
    throw new BridgeError(-32602, `${label} fields were invalid`);
  }
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

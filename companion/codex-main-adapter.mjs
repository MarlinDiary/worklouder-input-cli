import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { CodexBridgeError } from "./codex-main-bridge.mjs";

export const CODEX_SETTINGS_SNAPSHOT_SCHEMA_VERSION = 1;
export const CODEX_SETTINGS_SNAPSHOT_KIND = "worklouderctl-codex-settings-snapshot";
export const CODEX_SETTINGS_REVISION_ALGORITHM = "codex-settings-revision-v1";
export const CODEX_AGENT_KEYS_SNAPSHOT_KIND = "worklouderctl-codex-agent-keys-snapshot";
export const CODEX_AGENT_KEYS_MUTATION_KIND = "worklouderctl-codex-agent-keys-mutation";
export const CODEX_AGENT_KEYS_STATE_KEY = "codex-micro-custom-agent-assignments";
export const CODEX_AGENT_KEY_SLOTS = Object.freeze([
  "AG00", "AG01", "AG02", "AG03", "AG04", "AG05",
]);

const SETTINGS_PREFIX = "worklouder-codex-settings-revision-v1\0";
const AGENT_KEYS_PREFIX = "worklouder-codex-agent-keys-revision-v1\0";
const MAX_IDEMPOTENCY_ENTRIES = 256;

export function createCodexMainAdapter({
  request,
  settingsReplacer,
  agentKeysWriter,
  deviceServiceProvider,
  readSettingsSource = (path) => readFile(path),
}) {
  if (typeof request !== "function") throw new TypeError("request is required");
  if (
    settingsReplacer !== undefined &&
    (!settingsReplacer || typeof settingsReplacer.replaceSettings !== "function")
  ) {
    throw new TypeError("settingsReplacer.replaceSettings is required");
  }
  if (typeof readSettingsSource !== "function") {
    throw new TypeError("readSettingsSource must be a function");
  }
  if (
    agentKeysWriter !== undefined &&
    (!agentKeysWriter || typeof agentKeysWriter.replaceAssignments !== "function")
  ) {
    throw new TypeError("agentKeysWriter.replaceAssignments is required");
  }
  if (
    deviceServiceProvider !== undefined &&
    typeof deviceServiceProvider !== "function"
  ) {
    throw new TypeError("deviceServiceProvider must be a function");
  }
  const settingsIdempotencyCache = new Map();
  const agentKeysIdempotencyCache = new Map();

  const snapshotSettings = async () => {
    const result = await request("settings-read", {});
    if (!isRecord(result) || typeof result.filePath !== "string") {
      throw new CodexBridgeError(-32008, "Codex returned an invalid settings snapshot");
    }
    for (const name of ["settings", "effectiveSettings", "definitions"]) {
      if (!isRecord(result[name])) {
        throw new CodexBridgeError(-32008, `Codex settings ${name} was invalid`);
      }
    }
    const source = await readSettingsSource(result.filePath);
    if (!Buffer.isBuffer(source) && !(source instanceof Uint8Array)) {
      throw new CodexBridgeError(-32008, "Codex settings source was unavailable");
    }
    return {
      schemaVersion: CODEX_SETTINGS_SNAPSHOT_SCHEMA_VERSION,
      kind: CODEX_SETTINGS_SNAPSHOT_KIND,
      filePath: result.filePath,
      sourceSha256: sha256(source),
      settings: structuredClone(result.settings),
      effectiveSettings: structuredClone(result.effectiveSettings),
      definitions: structuredClone(result.definitions),
      settingsRevision: settingsRevision(result.settings),
    };
  };

  const snapshotAgentKeys = async () => {
    const response = await request("get-global-state", {
      key: CODEX_AGENT_KEYS_STATE_KEY,
    });
    const assignments = normalizeAssignments(response?.value);
    return {
      schemaVersion: 1,
      kind: CODEX_AGENT_KEYS_SNAPSHOT_KIND,
      globalStateKey: CODEX_AGENT_KEYS_STATE_KEY,
      slots: [...CODEX_AGENT_KEY_SLOTS],
      assignments,
      globalStateRevision: agentKeysRevision(assignments),
    };
  };

  const runMutation = async ({
    operation,
    expectedSourceSha256,
    expectedSettingsRevision,
    targetSettingsRevision,
    idempotencyKey,
    settings,
    effectiveSettings,
  }) => {
    validateSha(expectedSourceSha256, "expectedSourceSha256");
    validateSha(expectedSettingsRevision, "expectedSettingsRevision");
    validateSha(targetSettingsRevision, "targetSettingsRevision");
    if (settingsRevision(settings) !== targetSettingsRevision) {
      throw new CodexBridgeError(-32602, "targetSettingsRevision did not match settings");
    }
    const requestDigest = sha256(Buffer.from(canonicalJson({
      operation,
      expectedSourceSha256,
      expectedSettingsRevision,
      targetSettingsRevision,
      settings,
      effectiveSettings,
    })));
    const cached = settingsIdempotencyCache.get(idempotencyKey);
    if (cached) {
      if (cached.requestDigest !== requestDigest) {
        throw new CodexBridgeError(-32602, "idempotency key was reused with a different mutation");
      }
      return { ...cached.result, idempotentReplay: true };
    }

    const before = await snapshotSettings();
    if (
      before.sourceSha256 !== expectedSourceSha256 ||
      before.settingsRevision !== expectedSettingsRevision
    ) {
      throw new CodexBridgeError(-32005, "Codex settings revision conflict", {
        expectedSourceSha256,
        liveSourceSha256: before.sourceSha256,
        expectedSettingsRevision,
        liveSettingsRevision: before.settingsRevision,
      });
    }
    if (before.settingsRevision === targetSettingsRevision) {
      if (canonicalJson(before.effectiveSettings) !== canonicalJson(effectiveSettings)) {
        throw new CodexBridgeError(-32602, "target effectiveSettings differed from live settings");
      }
      const result = mutationResult({ operation, idempotencyKey, before, after: before, changed: false });
      cacheMutation(settingsIdempotencyCache, idempotencyKey, requestDigest, result);
      return result;
    }

    try {
      await settingsReplacer.replaceSettings({
        settings: structuredClone(settings),
        operation,
        targetSettingsRevision,
      });
      const after = await snapshotSettings();
      assertReadback(after, { settings, effectiveSettings, targetSettingsRevision });
      const result = mutationResult({ operation, idempotencyKey, before, after, changed: true });
      cacheMutation(settingsIdempotencyCache, idempotencyKey, requestDigest, result);
      return result;
    } catch (mutationError) {
      let restored;
      try {
        await settingsReplacer.replaceSettings({
          settings: structuredClone(before.settings),
          operation: "automatic-rollback",
          targetSettingsRevision: before.settingsRevision,
        });
        restored = await snapshotSettings();
        assertReadback(restored, {
          settings: before.settings,
          effectiveSettings: before.effectiveSettings,
          targetSettingsRevision: before.settingsRevision,
        });
      } catch (rollbackError) {
        throw new CodexBridgeError(-32008, "settings mutation and rollback failed", {
          mutationError: errorMessage(mutationError),
          rollbackError: errorMessage(rollbackError),
        });
      }
      throw new CodexBridgeError(-32008, "settings mutation failed and was rolled back", {
        operation,
        beforeSettingsRevision: before.settingsRevision,
        targetSettingsRevision,
        rollbackSettingsRevision: restored.settingsRevision,
        rollbackPerformed: true,
        mutationError: errorMessage(mutationError),
      });
    }
  };

  const runAgentKeysMutation = async ({
    operation,
    expectedGlobalStateRevision,
    targetGlobalStateRevision,
    idempotencyKey,
    assignments,
  }) => {
    validateSha(expectedGlobalStateRevision, "expectedGlobalStateRevision");
    validateSha(targetGlobalStateRevision, "targetGlobalStateRevision");
    const targetAssignments = normalizeAssignments(assignments);
    if (agentKeysRevision(targetAssignments) !== targetGlobalStateRevision) {
      throw new CodexBridgeError(-32602, "targetGlobalStateRevision did not match assignments");
    }
    const requestDigest = sha256(Buffer.from(canonicalJson({
      operation,
      expectedGlobalStateRevision,
      targetGlobalStateRevision,
      assignments: targetAssignments,
    })));
    const cached = agentKeysIdempotencyCache.get(idempotencyKey);
    if (cached) {
      if (cached.requestDigest !== requestDigest) {
        throw new CodexBridgeError(-32602, "idempotency key was reused with a different mutation");
      }
      return { ...cached.result, idempotentReplay: true };
    }

    const before = await snapshotAgentKeys();
    if (before.globalStateRevision !== expectedGlobalStateRevision) {
      throw new CodexBridgeError(-32005, "Codex Agent Key revision conflict", {
        expectedGlobalStateRevision,
        liveGlobalStateRevision: before.globalStateRevision,
      });
    }
    if (before.globalStateRevision === targetGlobalStateRevision) {
      const result = agentKeysMutationResult({
        operation,
        idempotencyKey,
        before,
        after: before,
        changed: false,
      });
      cacheMutation(agentKeysIdempotencyCache, idempotencyKey, requestDigest, result);
      return result;
    }

    try {
      await agentKeysWriter.replaceAssignments({
        key: CODEX_AGENT_KEYS_STATE_KEY,
        assignments: structuredClone(targetAssignments),
        operation,
        targetGlobalStateRevision,
      });
      const after = await snapshotAgentKeys();
      assertAgentKeysReadback(after, targetAssignments, targetGlobalStateRevision);
      const result = agentKeysMutationResult({
        operation,
        idempotencyKey,
        before,
        after,
        changed: true,
      });
      cacheMutation(agentKeysIdempotencyCache, idempotencyKey, requestDigest, result);
      return result;
    } catch (mutationError) {
      let restored;
      try {
        await agentKeysWriter.replaceAssignments({
          key: CODEX_AGENT_KEYS_STATE_KEY,
          assignments: structuredClone(before.assignments),
          operation: "automatic-rollback",
          targetGlobalStateRevision: before.globalStateRevision,
        });
        restored = await snapshotAgentKeys();
        assertAgentKeysReadback(restored, before.assignments, before.globalStateRevision);
      } catch (rollbackError) {
        throw new CodexBridgeError(-32008, "Agent Key mutation and rollback failed", {
          mutationError: errorMessage(mutationError),
          rollbackError: errorMessage(rollbackError),
        });
      }
      throw new CodexBridgeError(-32008, "Agent Key mutation failed and was rolled back", {
        operation,
        beforeGlobalStateRevision: before.globalStateRevision,
        targetGlobalStateRevision,
        rollbackGlobalStateRevision: restored.globalStateRevision,
        rollbackPerformed: true,
        mutationError: errorMessage(mutationError),
      });
    }
  };

  const focusDevice = async ({ app, expectLayer = null }) => {
    validateFocusApp(app);
    if (expectLayer !== null && !Number.isInteger(expectLayer)) {
      throw new CodexBridgeError(-32602, "expectLayer must be an integer or null");
    }
    const services = deviceServiceProvider();
    if (!Array.isArray(services)) {
      throw new CodexBridgeError(-32008, "Codex device services were unavailable");
    }
    const service = services.find(
      (value) =>
        value?.api?.api &&
        value?.comm &&
        value.getState?.().status === "connected",
    );
    if (!service) {
      throw new CodexBridgeError(-32008, "connected Codex device service was unavailable");
    }
    const serviceApi = service.api;
    const comm = service.comm;
    const connectionAttemptId = service.connectionAttemptId;
    const beforeStatus = await serviceApi.api.getDeviceStatus();
    await serviceApi.api.sendFocusApp(structuredClone(app));
    let afterStatus = await serviceApi.api.getDeviceStatus();
    const deadline = Date.now() + 2_000;
    while (
      expectLayer !== null &&
      afterStatus.selectedLayerIndex !== expectLayer &&
      Date.now() < deadline
    ) {
      await new Promise((resolve) => setTimeout(resolve, 50));
      afterStatus = await serviceApi.api.getDeviceStatus();
    }
    if (expectLayer !== null && afterStatus.selectedLayerIndex !== expectLayer) {
      throw new CodexBridgeError(
        -32008,
        `expected layer ${expectLayer}, observed ${afterStatus.selectedLayerIndex}`,
      );
    }
    return {
      operation: "focus",
      app: structuredClone(app),
      beforeStatus,
      afterStatus,
      continuity: {
        sameServiceApi: service.api === serviceApi,
        sameComm: service.comm === comm,
        sameConnectionAttempt: service.connectionAttemptId === connectionAttemptId,
        lifecycleState: service.lifecycleState,
        deviceState: service.getState(),
        hasHidSubscription: service.unsubscribeHid != null,
        hasJoystickSubscription: service.unsubscribeJoystick != null,
      },
    };
  };

  const adapter = { snapshotSettings, snapshotAgentKeys };
  if (settingsReplacer) {
    adapter.applySettings = (params) => runMutation({ ...params, operation: "apply" });
    adapter.restoreSettings = (params) => runMutation({ ...params, operation: "restore" });
  }
  if (agentKeysWriter) {
    adapter.applyAgentKeys = (params) => runAgentKeysMutation({ ...params, operation: "apply" });
    adapter.restoreAgentKeys = (params) => runAgentKeysMutation({ ...params, operation: "restore" });
  }
  if (deviceServiceProvider) adapter.focusDevice = focusDevice;
  return adapter;
}

function validateFocusApp(app) {
  if (!app || typeof app !== "object" || Array.isArray(app)) {
    throw new CodexBridgeError(-32602, "focus app must be an object");
  }
  for (const key of ["appName", "process", "path"]) {
    if (typeof app[key] !== "string" || app[key].length === 0 || app[key].length > 4096) {
      throw new CodexBridgeError(-32602, `focus app ${key} was invalid`);
    }
  }
}

export function settingsRevision(settings) {
  if (!isRecord(settings)) throw new CodexBridgeError(-32602, "settings must be an object");
  return sha256(Buffer.from(SETTINGS_PREFIX + canonicalJson(settings)));
}

export function agentKeysRevision(assignments) {
  return sha256(Buffer.from(AGENT_KEYS_PREFIX + canonicalJson(normalizeAssignments(assignments))));
}

export function canonicalJson(value) {
  return JSON.stringify(sortRecursively(value));
}

function normalizeAssignments(value) {
  const source = value === undefined || value === null ? {} : value;
  if (!isRecord(source)) throw new CodexBridgeError(-32008, "Agent Key assignments were invalid");
  for (const key of Object.keys(source)) {
    if (!CODEX_AGENT_KEY_SLOTS.includes(key)) {
      throw new CodexBridgeError(-32008, "Agent Key assignments contained an unknown slot", { slot: key });
    }
  }
  return Object.fromEntries(CODEX_AGENT_KEY_SLOTS.map((slot) => [slot, normalizeAssignment(source[slot], slot)]));
}

function normalizeAssignment(value, slot) {
  if (value === undefined || value === null) return null;
  if (!isRecord(value)) throw invalidAssignment(slot);
  if (value.type === "command" && nonEmpty(value.commandId)) {
    return structuredClone(value);
  }
  if (value.type === "skill" && nonEmpty(value.skillName) && nonEmpty(value.skillPath)) {
    return structuredClone(value);
  }
  if (nonEmpty(value.hostId) && nonEmpty(value.threadKey) && nonEmpty(value.title)) {
    return structuredClone(value);
  }
  if (nonEmpty(value.keycapId)) return structuredClone(value);
  throw invalidAssignment(slot);
}

function invalidAssignment(slot) {
  return new CodexBridgeError(-32008, "Agent Key assignment was invalid", { slot });
}

function assertReadback(after, target) {
  if (
    after.settingsRevision !== target.targetSettingsRevision ||
    canonicalJson(after.settings) !== canonicalJson(target.settings) ||
    canonicalJson(after.effectiveSettings) !== canonicalJson(target.effectiveSettings)
  ) {
    throw new Error("Codex exact settings readback did not match the target");
  }
}

function mutationResult({ operation, idempotencyKey, before, after, changed }) {
  return {
    schemaVersion: 1,
    kind: "worklouderctl-codex-settings-mutation",
    operation,
    idempotencyKey,
    idempotentReplay: false,
    changed,
    rollbackPerformed: false,
    beforeSourceSha256: before.sourceSha256,
    afterSourceSha256: after.sourceSha256,
    beforeSettingsRevision: before.settingsRevision,
    afterSettingsRevision: after.settingsRevision,
    targetSettingsRevision: after.settingsRevision,
  };
}

function assertAgentKeysReadback(after, assignments, targetGlobalStateRevision) {
  if (
    after.globalStateRevision !== targetGlobalStateRevision ||
    canonicalJson(after.assignments) !== canonicalJson(assignments)
  ) {
    throw new Error("Codex exact Agent Key readback did not match the target");
  }
}

function agentKeysMutationResult({ operation, idempotencyKey, before, after, changed }) {
  return {
    schemaVersion: 1,
    kind: CODEX_AGENT_KEYS_MUTATION_KIND,
    operation,
    idempotencyKey,
    idempotentReplay: false,
    changed,
    rollbackPerformed: false,
    beforeGlobalStateRevision: before.globalStateRevision,
    afterGlobalStateRevision: after.globalStateRevision,
    targetGlobalStateRevision: after.globalStateRevision,
  };
}

function cacheMutation(cache, key, requestDigest, result) {
  cache.set(key, { requestDigest, result });
  while (cache.size > MAX_IDEMPOTENCY_ENTRIES) cache.delete(cache.keys().next().value);
}

function sortRecursively(value) {
  if (Array.isArray(value)) return value.map(sortRecursively);
  if (!isRecord(value)) return value;
  return Object.fromEntries(Object.keys(value).sort().map((key) => [key, sortRecursively(value[key])]));
}

function validateSha(value, name) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
    throw new CodexBridgeError(-32602, name + " is invalid");
  }
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function nonEmpty(value) {
  return typeof value === "string" && value.length > 0;
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

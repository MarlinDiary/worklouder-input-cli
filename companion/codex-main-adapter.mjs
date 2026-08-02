import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { CodexBridgeError } from "./codex-main-bridge.mjs";

export const CODEX_SETTINGS_SNAPSHOT_SCHEMA_VERSION = 1;
export const CODEX_SETTINGS_SNAPSHOT_KIND = "worklouderctl-codex-settings-snapshot";
export const CODEX_SETTINGS_REVISION_ALGORITHM = "codex-settings-revision-v1";
export const CODEX_AGENT_KEYS_SNAPSHOT_KIND = "worklouder-codex-agent-keys-snapshot";
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
  const idempotencyCache = new Map();

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
    const cached = idempotencyCache.get(idempotencyKey);
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
      cacheMutation(idempotencyCache, idempotencyKey, requestDigest, result);
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
      cacheMutation(idempotencyCache, idempotencyKey, requestDigest, result);
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

  const adapter = { snapshotSettings, snapshotAgentKeys };
  if (settingsReplacer) {
    adapter.applySettings = (params) => runMutation({ ...params, operation: "apply" });
    adapter.restoreSettings = (params) => runMutation({ ...params, operation: "restore" });
  }
  return adapter;
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
    return { type: "command", commandId: value.commandId };
  }
  if (value.type === "skill" && nonEmpty(value.skillName) && nonEmpty(value.skillPath)) {
    return { type: "skill", skillName: value.skillName, skillPath: value.skillPath };
  }
  if (nonEmpty(value.hostId) && nonEmpty(value.threadKey) && nonEmpty(value.title)) {
    return { hostId: value.hostId, threadKey: value.threadKey, title: value.title };
  }
  if (nonEmpty(value.keycapId)) return { keycapId: value.keycapId };
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

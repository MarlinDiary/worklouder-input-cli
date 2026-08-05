import { installCodexCompanionBridge } from "./codex-main-integration.mjs";

export const SUPPORTED_CODEX_VERSION = "26.730.61309";
export const CODEX_LIVE_OVERLAY_REVISION = 10;
export const CODEX_MICRO_SETTING_KEYS = Object.freeze([
  "codex-micro-agent-source",
  "codex-micro-single-tap-agent-keys",
  "codex-micro-layout",
  "codex-micro-lighting-brightness",
  "codex-micro-lighting-auto-off",
]);

/**
 * Install the reference bridge into an already-running, release-gated Codex
 * main process. BrowserWindow is injected by the process-local bootstrap so
 * this module never imports or replaces Codex's Electron runtime.
 */
export async function installCodexLiveOverlay({
  app,
  BrowserWindow,
  settingsDefinitions,
  deviceServiceProvider,
  bridgeVersion = "0.1.1-live-overlay",
  socketPath,
  tokenPath,
  requestTimeoutMs = 15_000,
}) {
  assertVersion(app?.getVersion?.(), SUPPORTED_CODEX_VERSION, "Codex");
  const nativeRequest = createCodexNativeRequest({ BrowserWindow, requestTimeoutMs });
  const definitions = normalizeCodexMicroDefinitions(settingsDefinitions);
  const request = createCodexMicroRequest({ nativeRequest, definitions });
  const bridge = await installCodexCompanionBridge({
    app,
    request,
    bridgeVersion,
    socketPath,
    tokenPath,
    settingsReplacer: {
      async replaceSettings({ settings }) {
        validateCompleteSettings(settings, definitions);
        for (const key of CODEX_MICRO_SETTING_KEYS) {
          await nativeRequest("set-setting", { key, value: settings[key] });
        }
        // set-setting uses Codex's released schema and side-effect path. A
        // no-op settings-write then provides the released store's flush gate.
        const flushSource = await nativeRequest("settings-read", {});
        await nativeRequest("settings-write", { settings: flushSource.settings });
      },
    },
    agentKeysWriter: {
      async replaceAssignments({ key, assignments }) {
        await request("set-global-state", { key, value: assignments });
      },
    },
    deviceServiceProvider,
  });
  return { ...bridge, overlayRevision: CODEX_LIVE_OVERLAY_REVISION };
}

export function createCodexMicroRequest({ nativeRequest, definitions }) {
  if (typeof nativeRequest !== "function") {
    throw new TypeError("nativeRequest is required");
  }
  validateDefinitionMap(definitions);
  return async (method, params = {}) => {
    if (method !== "settings-read") return nativeRequest(method, params);
    const [source, settings] = await Promise.all([
      nativeRequest("settings-read", {}),
      nativeRequest("get-settings", {}),
    ]);
    if (typeof source?.filePath !== "string") {
      throw new Error("Codex settings-read omitted filePath");
    }
    return {
      filePath: source.filePath,
      settings: selectCodexMicroSettings(settings?.configuredValues),
      effectiveSettings: selectCodexMicroSettings(settings?.values),
      definitions: structuredClone(definitions),
    };
  };
}

export function normalizeCodexMicroDefinitions(settingsDefinitions) {
  if (!Array.isArray(settingsDefinitions)) {
    throw new TypeError("Codex released settings definitions are required");
  }
  const selected = new Map(
    settingsDefinitions
      .filter((definition) => CODEX_MICRO_SETTING_KEYS.includes(definition?.key))
      .map((definition) => [definition.key, definition]),
  );
  const normalized = {};
  for (const key of CODEX_MICRO_SETTING_KEYS) {
    const definition = selected.get(key);
    if (!definition || definition.agentAccess !== "hidden") {
      throw new Error(`Codex released definition missing or changed: ${key}`);
    }
    const schema = definition.schema?.toJSONSchema?.();
    if (!schema || typeof schema !== "object" || typeof schema.type !== "string") {
      throw new Error(`Codex released schema was unavailable: ${key}`);
    }
    normalized[key] = {
      type: schema.type,
      enum: Array.isArray(schema.enum) ? [...schema.enum] : [],
      minimum: Number.isInteger(schema.minimum) ? schema.minimum : null,
      maximum: Number.isInteger(schema.maximum) ? schema.maximum : null,
      default: structuredClone(definition.default),
    };
  }
  validateDefinitionMap(normalized);
  return normalized;
}

function selectCodexMicroSettings(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Codex get-settings returned an invalid object");
  }
  return Object.fromEntries(
    CODEX_MICRO_SETTING_KEYS.flatMap((key) =>
      Object.hasOwn(value, key) ? [[key, structuredClone(value[key])]] : [],
    ),
  );
}

function validateCompleteSettings(settings, definitions) {
  if (!settings || typeof settings !== "object" || Array.isArray(settings)) {
    throw new TypeError("complete Codex Micro settings are required");
  }
  const keys = Object.keys(settings).sort();
  const expected = Object.keys(definitions).sort();
  if (JSON.stringify(keys) !== JSON.stringify(expected)) {
    throw new TypeError("complete Codex Micro settings must contain exactly five keys");
  }
}

function validateDefinitionMap(definitions) {
  if (!definitions || typeof definitions !== "object" || Array.isArray(definitions)) {
    throw new TypeError("Codex Micro definitions are required");
  }
  const keys = Object.keys(definitions).sort();
  const expected = [...CODEX_MICRO_SETTING_KEYS].sort();
  if (JSON.stringify(keys) !== JSON.stringify(expected)) {
    throw new TypeError("Codex Micro definitions must contain exactly five keys");
  }
}

export function createCodexNativeRequest({
  BrowserWindow,
  requestTimeoutMs = 15_000,
}) {
  if (!BrowserWindow || typeof BrowserWindow.getAllWindows !== "function") {
    throw new TypeError("BrowserWindow.getAllWindows is required");
  }
  if (!Number.isInteger(requestTimeoutMs) || requestTimeoutMs < 1_000) {
    throw new TypeError("requestTimeoutMs must be an integer of at least 1000");
  }
  return async (method, params = {}) => {
    if (typeof method !== "string" || !/^[a-z][a-z0-9-]*$/.test(method)) {
      throw new TypeError("Codex native request method is invalid");
    }
    if (!params || typeof params !== "object" || Array.isArray(params)) {
      throw new TypeError("Codex native request params must be an object");
    }
    const rendererExpression = codexRendererRequestExpression({
      method,
      params,
      requestTimeoutMs,
    });
    const attempts = [];
    const windows = BrowserWindow.getAllWindows()
      .filter((window) => !window.isDestroyed())
      .sort((left, right) => Number(right.isVisible()) - Number(left.isVisible()));
    for (const window of windows) {
      try {
        const response = await window.webContents.executeJavaScript(
          rendererExpression,
          true,
        );
        if (
          !response ||
          typeof response !== "object" ||
          !Number.isInteger(response.status) ||
          response.status < 200 ||
          response.status >= 300
        ) {
          throw new Error(
            `Codex native request returned HTTP ${String(response?.status)}`,
          );
        }
        return response.body;
      } catch (error) {
        attempts.push(errorMessage(error));
      }
    }
    throw new Error(
      windows.length === 0
        ? "Codex has no live BrowserWindow"
        : `Codex native request failed in every BrowserWindow: ${attempts.join("; ")}`,
    );
  };
}

export function codexRendererRequestExpression({
  method,
  params,
  requestTimeoutMs,
}) {
  const request = JSON.stringify({ method, params, requestTimeoutMs });
  return `(async()=>{const input=${request};const requestId=crypto.randomUUID();return await new Promise((resolve,reject)=>{const finish=(callback,value)=>{clearTimeout(timer);window.removeEventListener("message",onMessage);callback(value)};const timer=setTimeout(()=>finish(reject,new Error("Codex native request timed out")),input.requestTimeoutMs);const onMessage=(event)=>{const value=event.data;if(!value||value.type!=="fetch-response"||value.requestId!==requestId)return;if(value.responseType!=="success")return finish(reject,new Error(value.error||"Codex native request failed"));let body;try{body=JSON.parse(value.bodyJsonString)}catch(error){return finish(reject,error)}finish(resolve,{status:value.status,body})};window.addEventListener("message",onMessage);const bridge=globalThis.electronBridge;if(!bridge||typeof bridge.sendMessageFromView!=="function")return finish(reject,new Error("Codex renderer bridge is unavailable"));Promise.resolve(bridge.sendMessageFromView({type:"fetch",requestId,url:"vscode://codex/"+input.method,method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify(input.params)})).catch(error=>finish(reject,error))})})()`;
}

function assertVersion(actual, expected, provider) {
  if (actual !== expected) {
    throw new Error(
      `${provider} live overlay supports ${expected}; detected ${String(actual)}`,
    );
  }
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

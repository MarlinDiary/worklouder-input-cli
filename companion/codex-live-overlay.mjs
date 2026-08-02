import { installCodexCompanionBridge } from "./codex-main-integration.mjs";

export const SUPPORTED_CODEX_VERSION = "26.727.51351";

/**
 * Install the reference bridge into an already-running, release-gated Codex
 * main process. BrowserWindow is injected by the process-local bootstrap so
 * this module never imports or replaces Codex's Electron runtime.
 */
export async function installCodexLiveOverlay({
  app,
  BrowserWindow,
  bridgeVersion = "0.1.0-live-overlay",
  socketPath,
  tokenPath,
  requestTimeoutMs = 15_000,
}) {
  assertVersion(app?.getVersion?.(), SUPPORTED_CODEX_VERSION, "Codex");
  const request = createCodexNativeRequest({ BrowserWindow, requestTimeoutMs });
  return installCodexCompanionBridge({
    app,
    request,
    bridgeVersion,
    socketPath,
    tokenPath,
    settingsReplacer: {
      async replaceSettings({ settings }) {
        await request("settings-write", { settings });
      },
    },
    agentKeysWriter: {
      async replaceAssignments({ key, assignments }) {
        await request("set-global-state", { key, value: assignments });
      },
    },
  });
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

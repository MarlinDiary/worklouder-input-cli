import { readFile } from "node:fs/promises";
import { createConnection } from "node:net";

export const DEFAULT_RETRY_DELAYS_MS = Object.freeze([0, 250, 1_000, 2_500]);

export class CodexBridgeRpcError extends Error {
  constructor(message, { code = null, data = null, transport = false } = {}) {
    super(message);
    this.name = "CodexBridgeRpcError";
    this.code = code;
    this.data = data;
    this.transport = transport;
  }
}

export class CodexBridgeClient {
  constructor({ socketPath, tokenPath, timeoutMs = 12_000 }) {
    this.socketPath = socketPath;
    this.tokenPath = tokenPath;
    this.timeoutMs = timeoutMs;
    this.socket = null;
    this.buffer = "";
    this.pending = new Map();
    this.nextId = 1;
    this.connecting = null;
    this.capabilities = [];
  }

  async connect() {
    if (this.socket && !this.socket.destroyed) return;
    if (this.connecting) return this.connecting;
    this.connecting = this.#connect();
    try {
      await this.connecting;
    } finally {
      this.connecting = null;
    }
  }

  async #connect() {
    const token = (await readFile(this.tokenPath, "utf8")).trim();
    if (Buffer.byteLength(token) < 32 || Buffer.byteLength(token) > 4096) {
      throw new CodexBridgeRpcError("Codex bridge token was invalid", { transport: true });
    }
    const socket = createConnection(this.socketPath);
    socket.setEncoding("utf8");
    this.socket = socket;
    this.buffer = "";
    socket.on("data", (chunk) => this.#onData(chunk));
    socket.on("error", (error) => this.#failTransport(error));
    socket.on("close", () => this.#failTransport(new Error("Codex bridge socket closed")));
    await new Promise((resolve, reject) => {
      const onConnect = () => { cleanup(); resolve(); };
      const onError = (error) => { cleanup(); reject(error); };
      const cleanup = () => {
        socket.off("connect", onConnect);
        socket.off("error", onError);
      };
      socket.once("connect", onConnect);
      socket.once("error", onError);
    }).catch((error) => {
      this.#failTransport(error);
      throw new CodexBridgeRpcError(errorMessage(error), { transport: true });
    });
    const hello = await this.#request("bridge.hello", {
      protocolVersion: 1,
      token,
      client: { name: "worklouderctl-appsense-relay", version: "1" },
    });
    if (hello?.protocolVersion !== 1 || !Array.isArray(hello.capabilities)) {
      this.close();
      throw new CodexBridgeRpcError("Codex bridge handshake was invalid", { transport: true });
    }
    this.capabilities = [...hello.capabilities];
    if (!this.capabilities.includes("codex.device.focus.v1")) {
      this.close();
      throw new CodexBridgeRpcError("Codex bridge focus capability was unavailable", {
        code: -32003,
      });
    }
  }

  async call(method, params) {
    await this.connect();
    return this.#request(method, params);
  }

  #request(method, params) {
    const socket = this.socket;
    if (!socket || socket.destroyed) {
      throw new CodexBridgeRpcError("Codex bridge socket was unavailable", { transport: true });
    }
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new CodexBridgeRpcError(`Codex bridge request timed out: ${method}`, {
          transport: true,
        }));
      }, this.timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      socket.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`, (error) => {
        if (!error) return;
        const pending = this.pending.get(id);
        if (!pending) return;
        this.pending.delete(id);
        clearTimeout(pending.timer);
        pending.reject(new CodexBridgeRpcError(errorMessage(error), { transport: true }));
      });
    });
  }

  #onData(chunk) {
    this.buffer += chunk;
    if (Buffer.byteLength(this.buffer, "utf8") > 16 * 1024 * 1024) {
      this.#failTransport(new Error("Codex bridge response exceeded maximum size"));
      return;
    }
    while (this.buffer.includes("\n")) {
      const newline = this.buffer.indexOf("\n");
      const line = this.buffer.slice(0, newline);
      this.buffer = this.buffer.slice(newline + 1);
      if (!line) continue;
      let response;
      try {
        response = JSON.parse(line);
      } catch (error) {
        this.#failTransport(error);
        return;
      }
      const pending = this.pending.get(response?.id);
      if (!pending) continue;
      this.pending.delete(response.id);
      clearTimeout(pending.timer);
      if (response.error) {
        pending.reject(new CodexBridgeRpcError(
          response.error.message ?? "Codex bridge request failed",
          { code: response.error.code ?? null, data: response.error.data ?? null },
        ));
      } else {
        pending.resolve(response.result);
      }
    }
  }

  #failTransport(error) {
    const failure = error instanceof CodexBridgeRpcError
      ? error
      : new CodexBridgeRpcError(errorMessage(error), { transport: true });
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(failure);
    }
    this.pending.clear();
    if (this.socket && !this.socket.destroyed) this.socket.destroy();
    this.socket = null;
  }

  close() {
    this.#failTransport(new CodexBridgeRpcError("Codex bridge client closed", {
      transport: true,
    }));
  }
}

export function createFocusForwarder({
  socketPath,
  tokenPath,
  installBridge,
  clientFactory = (options) => new CodexBridgeClient(options),
  retryDelaysMs = DEFAULT_RETRY_DELAYS_MS,
  sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds)),
  withCallLock = async (operation) => operation(),
}) {
  let client = null;

  const resetClient = () => {
    client?.close?.();
    client = null;
  };

  return {
    async forward(app, expectLayer = null) {
      validateFocusApp(app);
      let lastError;
      let installAttempted = false;
      for (let index = 0; index < retryDelaysMs.length; index += 1) {
        const delay = retryDelaysMs[index];
        if (delay > 0) await sleep(delay);
        try {
          client ??= clientFactory({ socketPath, tokenPath });
          const result = await withCallLock(() =>
            client.call("codex.device.focus", { app, expectLayer }),
          );
          validateFocusResult(result);
          return { result, retryCount: index, bridgeReinstalled: installAttempted };
        } catch (error) {
          lastError = error;
          if (error?.transport === true) resetClient();
          if (
            !installAttempted &&
            typeof installBridge === "function" &&
            (error?.transport === true || error?.code === -32601 || error?.code === -32003)
          ) {
            installAttempted = true;
            await installBridge();
            resetClient();
          }
        }
      }
      throw lastError;
    },
    close: resetClient,
  };
}

export function relayHealth({ running, installed, lastEvent }) {
  if (!installed) return { status: "not-installed", healthy: false };
  if (!running) return { status: "stopped", healthy: false };
  if (!lastEvent) return { status: "starting", healthy: false };
  if (typeof lastEvent.error === "string") {
    return { status: "degraded", healthy: false, error: lastEvent.error };
  }
  return {
    status: "healthy",
    healthy: true,
    lastSuccessAt: lastEvent.at ?? null,
    retryCount: lastEvent.retryCount ?? 0,
  };
}

export function launchAgentProgramArguments({ nodeCommand, scriptPath }) {
  if (typeof nodeCommand !== "string" || nodeCommand.length === 0) {
    throw new TypeError("nodeCommand is required");
  }
  return nodeCommand.includes("/")
    ? [nodeCommand, scriptPath, "run"]
    : ["/usr/bin/env", nodeCommand, scriptPath, "run"];
}

export async function runOneShot(operation, close) {
  if (typeof operation !== "function" || typeof close !== "function") {
    throw new TypeError("one-shot operation and close callbacks are required");
  }
  try {
    return await operation();
  } finally {
    close();
  }
}

function validateFocusApp(app) {
  if (!app || typeof app !== "object" || Array.isArray(app)) {
    throw new TypeError("focus app must be an object");
  }
  for (const key of ["appName", "process", "path"]) {
    if (typeof app[key] !== "string" || app[key].length === 0) {
      throw new TypeError(`focus app ${key} was invalid`);
    }
  }
}

function validateFocusResult(result) {
  if (result?.operation !== "focus") {
    throw new CodexBridgeRpcError("Codex focus response was invalid", { transport: true });
  }
  const continuity = result.continuity;
  if (
    continuity?.sameServiceApi !== true ||
    continuity?.sameComm !== true ||
    continuity?.sameConnectionAttempt !== true ||
    continuity?.lifecycleState !== "started" ||
    continuity?.deviceState?.status !== "connected"
  ) {
    throw new CodexBridgeRpcError("Codex focus response lost device continuity");
  }
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

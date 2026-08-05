import { randomBytes, timingSafeEqual } from "node:crypto";
import {
  chmod,
  lstat,
  mkdir,
  readFile,
  unlink,
  writeFile,
} from "node:fs/promises";
import { createServer } from "node:net";
import { dirname } from "node:path";

export const CODEX_BRIDGE_PROTOCOL_VERSION = 1;
export const CODEX_BRIDGE_MAX_REQUEST_BYTES = 16 * 1024 * 1024;
export const CODEX_BRIDGE_MAX_RESPONSE_BYTES = 16 * 1024 * 1024;

const METHOD_DEFINITIONS = new Map([
  ["codex.settings.snapshot", ["codex.settings.snapshot.v1", "snapshotSettings"]],
  ["codex.settings.apply", ["codex.settings.apply.v1", "applySettings"]],
  ["codex.settings.restore", ["codex.settings.restore.v1", "restoreSettings"]],
  ["codex.agentKeys.snapshot", ["codex.agentKeys.snapshot.v1", "snapshotAgentKeys"]],
  ["codex.agentKeys.apply", ["codex.agentKeys.apply.v1", "applyAgentKeys"]],
  ["codex.agentKeys.restore", ["codex.agentKeys.restore.v1", "restoreAgentKeys"]],
  ["codex.device.focus", ["codex.device.focus.v1", "focusDevice"]],
  ["codex.runtime.status", ["codex.runtime.status.v1", "runtimeStatus"]],
  ["codex.runtime.recover", ["codex.runtime.recover.v1", "recoverRuntime"]],
]);

const MUTATION_METHODS = new Set([
  "codex.settings.apply",
  "codex.settings.restore",
  "codex.agentKeys.apply",
  "codex.agentKeys.restore",
  "codex.runtime.recover",
]);

export class CodexBridgeError extends Error {
  constructor(code, message, data) {
    super(message);
    this.name = "CodexBridgeError";
    this.code = code;
    this.data = data;
  }
}

export async function startCodexCompanionBridge({
  adapter,
  codexVersion,
  socketPath,
  tokenPath,
  bridgeVersion = "0.1.1",
  token,
  maxRequestBytes = CODEX_BRIDGE_MAX_REQUEST_BYTES,
  maxResponseBytes = CODEX_BRIDGE_MAX_RESPONSE_BYTES,
}) {
  validateOptions({ adapter, codexVersion, socketPath, tokenPath });
  await mkdir(dirname(socketPath), { recursive: true, mode: 0o700 });
  await mkdir(dirname(tokenPath), { recursive: true, mode: 0o700 });
  await removeStaleSocket(socketPath);

  const bridgeToken = await prepareToken(tokenPath, token);
  const capabilities = [
    "bridge.handshake.v1",
    "bridge.health.v1",
    ...[...METHOD_DEFINITIONS.values()]
      .filter(([, adapterMethod]) => typeof adapter[adapterMethod] === "function")
      .map(([capability]) => capability),
  ];
  const sessionId = randomBytes(16).toString("hex");
  const startedAt = Date.now();
  let requestTail = Promise.resolve();
  const clients = new Set();

  const enqueue = (operation) => {
    const result = requestTail.then(operation, operation);
    requestTail = result.catch(() => {});
    return result;
  };

  const server = createServer((socket) => {
    clients.add(socket);
    socket.setEncoding("utf8");
    let authenticated = false;
    let buffer = "";

    socket.on("close", () => clients.delete(socket));
    socket.on("error", () => clients.delete(socket));
    socket.on("data", (chunk) => {
      buffer += chunk;
      if (Buffer.byteLength(buffer, "utf8") > maxRequestBytes) {
        writeError(socket, null, -32600, "request exceeded maximum size");
        socket.end();
        return;
      }
      while (buffer.includes("\n")) {
        const newline = buffer.indexOf("\n");
        const line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        if (line.length !== 0) {
          void handleLine(line);
        }
      }
    });

    async function handleLine(line) {
      let request;
      try {
        request = JSON.parse(line);
      } catch {
        writeError(socket, null, -32700, "parse error");
        return;
      }
      const id = request?.id ?? null;
      if (
        request?.jsonrpc !== "2.0" ||
        typeof request?.method !== "string" ||
        request?.params === null ||
        typeof request?.params !== "object" ||
        Array.isArray(request?.params)
      ) {
        writeError(socket, id, -32600, "invalid request");
        return;
      }

      if (!authenticated) {
        if (request.method !== "bridge.hello") {
          writeError(socket, id, -32001, "authentication required");
          socket.end();
          return;
        }
        try {
          validateHello(request.params, bridgeToken);
          authenticated = true;
          writeResult(socket, id, {
            protocolVersion: CODEX_BRIDGE_PROTOCOL_VERSION,
            bridgeVersion,
            codexVersion,
            sessionId,
            capabilities,
          }, maxResponseBytes);
        } catch (error) {
          writeKnownError(socket, id, error);
          socket.end();
        }
        return;
      }

      if (request.method === "bridge.hello") {
        writeError(socket, id, -32600, "bridge.hello is only valid once");
        return;
      }
      if (request.method === "bridge.health") {
        writeResult(socket, id, {
          protocolVersion: CODEX_BRIDGE_PROTOCOL_VERSION,
          bridgeVersion,
          codexVersion,
          sessionId,
          uptimeMs: Date.now() - startedAt,
        }, maxResponseBytes);
        return;
      }

      const definition = METHOD_DEFINITIONS.get(request.method);
      if (!definition) {
        writeError(socket, id, -32601, "method not found");
        return;
      }
      const [capability, adapterMethod] = definition;
      if (!capabilities.includes(capability)) {
        writeError(socket, id, -32003, "capability unavailable", { capability });
        return;
      }
      try {
        if (MUTATION_METHODS.has(request.method)) {
          validateMutationParams(request.method, request.params);
        }
        const result = await enqueue(() => adapter[adapterMethod](request.params));
        writeResult(socket, id, result, maxResponseBytes);
      } catch (error) {
        writeKnownError(socket, id, error);
      }
    }
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(socketPath, () => {
      server.off("error", reject);
      resolve();
    });
  });
  await chmod(socketPath, 0o600);

  return {
    bridgeVersion,
    capabilities,
    codexVersion,
    protocolVersion: CODEX_BRIDGE_PROTOCOL_VERSION,
    sessionId,
    socketPath,
    tokenPath,
    async stop() {
      for (const client of clients) client.destroy();
      await new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()));
      });
      await unlink(socketPath).catch((error) => {
        if (error.code !== "ENOENT") throw error;
      });
    },
  };
}

function validateOptions({ adapter, codexVersion, socketPath, tokenPath }) {
  if (!adapter || typeof adapter !== "object") {
    throw new TypeError("adapter must be an object");
  }
  for (const [name, value] of [
    ["codexVersion", codexVersion],
    ["socketPath", socketPath],
    ["tokenPath", tokenPath],
  ]) {
    if (typeof value !== "string" || value.length === 0) {
      throw new TypeError(name + " must be a non-empty string");
    }
  }
}

async function removeStaleSocket(socketPath) {
  try {
    const metadata = await lstat(socketPath);
    if (!metadata.isSocket()) {
      throw new Error("bridge path exists and is not a socket: " + socketPath);
    }
    await unlink(socketPath);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
}

async function prepareToken(tokenPath, suppliedToken) {
  let token = suppliedToken;
  if (token === undefined) {
    try {
      token = (await readFile(tokenPath, "utf8")).trim();
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      token = randomBytes(32).toString("hex");
    }
  }
  const byteLength = typeof token === "string" ? Buffer.byteLength(token) : 0;
  if (byteLength < 32 || byteLength > 4096) {
    throw new Error("bridge token must contain between 32 and 4096 bytes");
  }
  await writeFile(tokenPath, token, { encoding: "utf8", mode: 0o600 });
  await chmod(tokenPath, 0o600);
  return token;
}

function validateHello(params, expectedToken) {
  if (params.protocolVersion !== CODEX_BRIDGE_PROTOCOL_VERSION) {
    throw new CodexBridgeError(-32002, "protocol version unsupported", {
      supported: [CODEX_BRIDGE_PROTOCOL_VERSION],
    });
  }
  if (
    !params.client ||
    typeof params.client.name !== "string" ||
    typeof params.client.version !== "string"
  ) {
    throw new CodexBridgeError(-32602, "client name and version are required");
  }
  const supplied = Buffer.from(String(params.token ?? ""));
  const expected = Buffer.from(expectedToken);
  if (supplied.length !== expected.length || !timingSafeEqual(supplied, expected)) {
    throw new CodexBridgeError(-32001, "authentication failed");
  }
}

function validateMutationParams(method, params) {
  if (method === "codex.runtime.recover") {
    if (
      !Number.isInteger(params.timeoutMs) ||
      params.timeoutMs < 1_000 ||
      params.timeoutMs > 25_000
    ) {
      throw new CodexBridgeError(
        -32602,
        "runtime timeoutMs must be an integer from 1000 through 25000",
      );
    }
    return;
  }
  if (method.startsWith("codex.agentKeys.")) {
    validateAgentKeysMutationParams(params);
    return;
  }
  for (const name of [
    "expectedSourceSha256",
    "expectedSettingsRevision",
    "targetSettingsRevision",
  ]) {
    if (typeof params[name] !== "string" || !/^[0-9a-f]{64}$/.test(params[name])) {
      throw new CodexBridgeError(-32602, name + " is invalid");
    }
  }
  if (
    typeof params.idempotencyKey !== "string" ||
    params.idempotencyKey.length === 0 ||
    Buffer.byteLength(params.idempotencyKey) > 256 ||
    params.idempotencyKey.includes("\0")
  ) {
    throw new CodexBridgeError(-32602, "idempotencyKey is invalid");
  }
  for (const name of ["settings", "effectiveSettings"]) {
    if (!isRecord(params[name])) {
      throw new CodexBridgeError(-32602, name + " is required");
    }
  }
}

function validateAgentKeysMutationParams(params) {
  for (const name of [
    "expectedGlobalStateRevision",
    "targetGlobalStateRevision",
  ]) {
    if (typeof params[name] !== "string" || !/^[0-9a-f]{64}$/.test(params[name])) {
      throw new CodexBridgeError(-32602, name + " is invalid");
    }
  }
  if (
    typeof params.idempotencyKey !== "string" ||
    params.idempotencyKey.length === 0 ||
    Buffer.byteLength(params.idempotencyKey) > 256 ||
    params.idempotencyKey.includes("\0")
  ) {
    throw new CodexBridgeError(-32602, "idempotencyKey is invalid");
  }
  if (!isRecord(params.assignments)) {
    throw new CodexBridgeError(-32602, "assignments is required");
  }
}

function writeKnownError(socket, id, error) {
  if (error instanceof CodexBridgeError) {
    writeError(socket, id, error.code, error.message, error.data);
  } else {
    writeError(socket, id, -32603, "internal error", {
      message: error instanceof Error ? error.message : String(error),
    });
  }
}

function writeResult(socket, id, result, maxBytes) {
  const response = JSON.stringify({ jsonrpc: "2.0", id, result }) + "\n";
  if (Buffer.byteLength(response) > maxBytes) {
    writeError(socket, id, -32603, "response exceeded maximum size");
    return;
  }
  socket.write(response);
}

function writeError(socket, id, code, message, data) {
  const error = { code, message };
  if (data !== undefined) error.data = data;
  socket.write(JSON.stringify({ jsonrpc: "2.0", id, error }) + "\n");
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

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

export const BRIDGE_PROTOCOL_VERSION = 1;
export const DEFAULT_MAX_REQUEST_BYTES = 64 * 1024 * 1024;
export const DEFAULT_MAX_RESPONSE_BYTES = 64 * 1024 * 1024;

const METHOD_DEFINITIONS = new Map([
  ["device.list", ["device.list.v1", "listDevices"]],
  ["device.status", ["device.status.v1", "getDeviceStatus"]],
  ["device.files.list", ["device.files.list.v1", "listFiles"]],
  ["device.files.read", ["device.files.read.v1", "readFile"]],
  ["device.config.snapshot", ["device.config.snapshot.v1", "snapshotConfig"]],
  ["device.config.validate", ["device.config.validate.v1", "validateConfig"]],
  ["device.config.apply", ["device.config.apply.v1", "applyConfig"]],
  ["device.config.restore", ["device.config.restore.v1", "restoreConfig"]],
  [
    "input.host-settings.snapshot",
    ["input.host-settings.snapshot.v1", "snapshotHostSettings"],
  ],
  [
    "input.host-settings.apply",
    ["input.host-settings.apply.v1", "applyHostSettings"],
  ],
  [
    "input.host-settings.restore",
    ["input.host-settings.restore.v1", "restoreHostSettings"],
  ],
  ["input.presets.snapshot", ["input.presets.snapshot.v1", "snapshotPresets"]],
  [
    "input.appsense.runtime",
    ["input.appsense.runtime.v1", "getAppSenseRuntime"],
  ],
  [
    "input.permissions.status",
    ["input.permissions.status.v1", "getPermissionsStatus"],
  ],
  [
    "input.firmware.status",
    ["input.firmware.status.v1", "getFirmwareStatus"],
  ],
  ["input.logs.snapshot", ["input.logs.snapshot.v1", "snapshotLogs"]],
]);

const MUTATION_METHODS = new Set([
  "device.config.apply",
  "device.config.restore",
  "input.host-settings.apply",
  "input.host-settings.restore",
]);

export class BridgeError extends Error {
  constructor(code, message, data) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
    this.data = data;
  }
}

export async function startInputCompanionBridge({
  adapter,
  inputVersion,
  socketPath,
  tokenPath,
  bridgeVersion = "0.1.0",
  token,
  maxRequestBytes = DEFAULT_MAX_REQUEST_BYTES,
  maxResponseBytes = DEFAULT_MAX_RESPONSE_BYTES,
}) {
  validateOptions({ adapter, inputVersion, socketPath, tokenPath });
  await mkdir(dirname(socketPath), { recursive: true, mode: 0o700 });
  await mkdir(dirname(tokenPath), { recursive: true, mode: 0o700 });
  await removeStaleSocket(socketPath);

  const bridgeToken = await prepareToken(tokenPath, token);
  const capabilities = [
    "bridge.handshake.v1",
    "bridge.health.v1",
    ...[...METHOD_DEFINITIONS.values()]
      .filter(
        ([, adapterMethod]) => typeof adapter[adapterMethod] === "function",
      )
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
        if (line.length === 0) {
          continue;
        }
        void handleLine(line);
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
          writeResult(
            socket,
            id,
            {
              protocolVersion: BRIDGE_PROTOCOL_VERSION,
              bridgeVersion,
              inputVersion,
              sessionId,
              capabilities,
            },
            maxResponseBytes,
          );
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
        writeResult(
          socket,
          id,
          {
            protocolVersion: BRIDGE_PROTOCOL_VERSION,
            bridgeVersion,
            inputVersion,
            sessionId,
            uptimeMs: Date.now() - startedAt,
          },
          maxResponseBytes,
        );
        return;
      }

      const definition = METHOD_DEFINITIONS.get(request.method);
      if (!definition) {
        writeError(socket, id, -32601, "method not found");
        return;
      }
      const [capability, adapterMethod] = definition;
      if (
        !capabilities.includes(capability) ||
        typeof adapter[adapterMethod] !== "function"
      ) {
        writeError(socket, id, -32003, "capability unavailable", {
          capability,
        });
        return;
      }

      try {
        if (MUTATION_METHODS.has(request.method)) {
          validateMutationParams(request.method, request.params);
        }
        const result = await enqueue(() =>
          adapter[adapterMethod](request.params),
        );
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
    inputVersion,
    protocolVersion: BRIDGE_PROTOCOL_VERSION,
    sessionId,
    socketPath,
    tokenPath,
    async stop() {
      for (const client of clients) {
        client.destroy();
      }
      await new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()));
      });
      await unlink(socketPath).catch((error) => {
        if (error.code !== "ENOENT") {
          throw error;
        }
      });
    },
  };
}

function validateOptions({ adapter, inputVersion, socketPath, tokenPath }) {
  if (!adapter || typeof adapter !== "object") {
    throw new TypeError("adapter must be an object");
  }
  for (const [name, value] of [
    ["inputVersion", inputVersion],
    ["socketPath", socketPath],
    ["tokenPath", tokenPath],
  ]) {
    if (typeof value !== "string" || value.length === 0) {
      throw new TypeError(name + " must be a non-empty string");
    }
  }
}

async function removeStaleSocket(socketPath) {
  let metadata;
  try {
    metadata = await lstat(socketPath);
  } catch (error) {
    if (error.code === "ENOENT") {
      return;
    }
    throw error;
  }
  if (!metadata.isSocket()) {
    throw new Error("bridge path exists and is not a socket: " + socketPath);
  }
  await unlink(socketPath);
}

async function prepareToken(tokenPath, suppliedToken) {
  let token = suppliedToken;
  if (token === undefined) {
    try {
      token = (await readFile(tokenPath, "utf8")).trim();
    } catch (error) {
      if (error.code !== "ENOENT") {
        throw error;
      }
      token = randomBytes(32).toString("hex");
    }
  }
  if (
    typeof token !== "string" ||
    Buffer.byteLength(token, "utf8") < 32 ||
    Buffer.byteLength(token, "utf8") > 4096
  ) {
    throw new Error("bridge token must contain between 32 and 4096 bytes");
  }
  await writeFile(tokenPath, token, { encoding: "utf8", mode: 0o600 });
  await chmod(tokenPath, 0o600);
  return token;
}

function validateHello(params, expectedToken) {
  if (params.protocolVersion !== BRIDGE_PROTOCOL_VERSION) {
    throw new BridgeError(-32002, "protocol version unsupported", {
      supported: [BRIDGE_PROTOCOL_VERSION],
    });
  }
  const client = params.client;
  if (
    !client ||
    typeof client.name !== "string" ||
    typeof client.version !== "string"
  ) {
    throw new BridgeError(-32602, "client name and version are required");
  }
  if (
    typeof params.token !== "string" ||
    !constantTimeEqual(params.token, expectedToken)
  ) {
    throw new BridgeError(-32001, "authentication failed");
  }
}

function constantTimeEqual(left, right) {
  const leftBytes = Buffer.from(left, "utf8");
  const rightBytes = Buffer.from(right, "utf8");
  return (
    leftBytes.length === rightBytes.length &&
    timingSafeEqual(leftBytes, rightBytes)
  );
}

function validateMutationParams(method, params) {
  if (method.startsWith("input.host-settings.")) {
    validateRevisionAndIdempotency(params);
    const payload =
      method === "input.host-settings.apply" ? "settings" : "snapshot";
    if (
      !params[payload] ||
      typeof params[payload] !== "object" ||
      Array.isArray(params[payload])
    ) {
      throw new BridgeError(-32602, payload + " is required");
    }
    return;
  }
  if (
    typeof params.deviceId !== "string" ||
    params.deviceId.length === 0 ||
    Buffer.byteLength(params.deviceId, "utf8") > 512 ||
    params.deviceId.includes("\0")
  ) {
    throw new BridgeError(-32602, "deviceId is invalid");
  }
  validateRevisionAndIdempotency(params);
  const payload = method === "device.config.apply" ? "config" : "snapshot";
  if (
    !params[payload] ||
    typeof params[payload] !== "object" ||
    Array.isArray(params[payload])
  ) {
    throw new BridgeError(-32602, payload + " is required");
  }
}

function validateRevisionAndIdempotency(params) {
  if (
    typeof params.expectedRevision !== "string" ||
    !/^[0-9a-f]{64}$/i.test(params.expectedRevision)
  ) {
    throw new BridgeError(-32602, "expectedRevision is invalid");
  }
  if (
    typeof params.idempotencyKey !== "string" ||
    params.idempotencyKey.length === 0 ||
    Buffer.byteLength(params.idempotencyKey, "utf8") > 256 ||
    params.idempotencyKey.includes("\0")
  ) {
    throw new BridgeError(-32602, "idempotencyKey is invalid");
  }
}

function writeKnownError(socket, id, error) {
  if (error instanceof BridgeError) {
    writeError(socket, id, error.code, error.message, error.data);
    return;
  }
  writeError(socket, id, -32008, "adapter failure", {
    message: error instanceof Error ? error.message : String(error),
  });
}

function writeResult(socket, id, result, maxResponseBytes) {
  const response = JSON.stringify({ jsonrpc: "2.0", id, result });
  if (Buffer.byteLength(response, "utf8") > maxResponseBytes) {
    writeError(socket, id, -32008, "response exceeded maximum size");
    return;
  }
  socket.write(response + "\n");
}

function writeError(socket, id, code, message, data) {
  const error = { code, message };
  if (data !== undefined) {
    error.data = data;
  }
  socket.write(JSON.stringify({ jsonrpc: "2.0", id, error }) + "\n");
}

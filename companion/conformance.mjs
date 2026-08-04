#!/usr/bin/env node

import { readFile, realpath, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { createConnection } from "node:net";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { BRIDGE_PROTOCOL_VERSION } from "./input-main-bridge.mjs";

const MAX_LINE_BYTES = 1024 * 1024;

export async function inspectInputCompanionBridge({
  socketPath = defaultSocketPath(),
  tokenPath = defaultTokenPath(),
  requiredCapabilities = [],
  clientVersion = "0.1.1",
}) {
  await validatePrivatePath(socketPath, "socket");
  await validatePrivatePath(tokenPath, "token");
  const token = (await readFile(tokenPath, "utf8")).trim();
  if (Buffer.byteLength(token, "utf8") < 32) {
    throw new Error("bridge token is shorter than 32 bytes");
  }

  const socket = createConnection(socketPath);
  socket.setEncoding("utf8");
  socket.setTimeout(5000, () => socket.destroy(new Error("bridge timed out")));
  await waitForConnect(socket);
  const responses = responseLines(socket);
  let nextId = 1;
  const call = async (method, params) => {
    const id = nextId++;
    socket.write(
      JSON.stringify({ jsonrpc: "2.0", id: String(id), method, params }) +
        "\n",
    );
    const response = await responses.next();
    if (response.done) {
      throw new Error("bridge closed before responding");
    }
    if (response.value.id !== String(id)) {
      throw new Error("bridge returned a mismatched response ID");
    }
    if (response.value.error) {
      throw new Error(
        `bridge method ${method} failed: ${response.value.error.code} ${response.value.error.message}`,
      );
    }
    return response.value.result;
  };

  try {
    const handshake = await call("bridge.hello", {
      protocolVersion: BRIDGE_PROTOCOL_VERSION,
      token,
      client: {
        name: "input-companion-conformance",
        version: clientVersion,
      },
    });
    if (handshake.protocolVersion !== BRIDGE_PROTOCOL_VERSION) {
      throw new Error("bridge negotiated an unsupported protocol version");
    }
    if (!Array.isArray(handshake.capabilities)) {
      throw new Error("bridge handshake omitted capabilities");
    }
    for (const capability of [
      "bridge.handshake.v1",
      "bridge.health.v1",
      ...requiredCapabilities,
    ]) {
      if (!handshake.capabilities.includes(capability)) {
        throw new Error(`bridge omitted required capability ${capability}`);
      }
    }
    const health = await call("bridge.health", {});
    if (
      health.protocolVersion !== handshake.protocolVersion ||
      health.sessionId !== handshake.sessionId ||
      health.bridgeVersion !== handshake.bridgeVersion ||
      health.inputVersion !== handshake.inputVersion
    ) {
      throw new Error("bridge health did not match the authenticated session");
    }
    return {
      conformant: true,
      protocolVersion: handshake.protocolVersion,
      bridgeVersion: handshake.bridgeVersion,
      inputVersion: handshake.inputVersion,
      sessionId: handshake.sessionId,
      capabilities: handshake.capabilities,
      uptimeMs: health.uptimeMs,
      socketPath,
      tokenPath,
    };
  } finally {
    socket.destroy();
  }
}

function waitForConnect(socket) {
  return new Promise((resolveConnection, rejectConnection) => {
    const connected = () => {
      socket.off("error", failed);
      resolveConnection();
    };
    const failed = (error) => {
      socket.off("connect", connected);
      rejectConnection(error);
    };
    socket.once("connect", connected);
    socket.once("error", failed);
  });
}

function responseLines(socket) {
  let buffer = "";
  const lines = [];
  const waiting = [];
  let ended = false;
  let failure;

  const settle = () => {
    while (waiting.length > 0 && (lines.length > 0 || ended || failure)) {
      const waiter = waiting.shift();
      if (failure) {
        waiter.reject(failure);
      } else if (lines.length > 0) {
        waiter.resolve({ done: false, value: lines.shift() });
      } else {
        waiter.resolve({ done: true });
      }
    }
  };
  socket.on("data", (chunk) => {
    buffer += chunk;
    if (Buffer.byteLength(buffer, "utf8") > MAX_LINE_BYTES) {
      failure = new Error("bridge response exceeded maximum line size");
      socket.destroy(failure);
      settle();
      return;
    }
    while (buffer.includes("\n")) {
      const offset = buffer.indexOf("\n");
      const line = buffer.slice(0, offset);
      buffer = buffer.slice(offset + 1);
      if (line.length === 0) {
        continue;
      }
      try {
        lines.push(JSON.parse(line));
      } catch (error) {
        failure = new Error("bridge returned invalid JSON", { cause: error });
        socket.destroy(failure);
        break;
      }
    }
    settle();
  });
  socket.on("end", () => {
    ended = true;
    settle();
  });
  socket.on("error", (error) => {
    failure = error;
    settle();
  });
  return {
    next() {
      if (failure) {
        return Promise.reject(failure);
      }
      if (lines.length > 0) {
        return Promise.resolve({ done: false, value: lines.shift() });
      }
      if (ended) {
        return Promise.resolve({ done: true });
      }
      return new Promise((resolve, reject) => waiting.push({ resolve, reject }));
    },
  };
}

async function validatePrivatePath(path, kind) {
  const metadata = await stat(path);
  if (kind === "socket" ? !metadata.isSocket() : !metadata.isFile()) {
    throw new Error(`bridge ${kind} path has the wrong type: ${path}`);
  }
  if ((metadata.mode & 0o777) !== 0o600) {
    throw new Error(`bridge ${kind} permissions must be 0600: ${path}`);
  }
  if (typeof process.getuid === "function" && metadata.uid !== process.getuid()) {
    throw new Error(`bridge ${kind} must be owned by the current user: ${path}`);
  }
}

function defaultRoot() {
  return join(homedir(), "Library", "Application Support", "input");
}

function defaultSocketPath() {
  return (
    process.env.WORKLOUDERCTL_BRIDGE_SOCKET ??
    join(defaultRoot(), "worklouderctl-bridge-v1.sock")
  );
}

function defaultTokenPath() {
  return (
    process.env.WORKLOUDERCTL_BRIDGE_TOKEN_FILE ??
    join(defaultRoot(), "worklouderctl-bridge-v1.token")
  );
}

function parseArguments(argv) {
  const options = { requiredCapabilities: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (argument === "--socket" && value) {
      options.socketPath = value;
    } else if (argument === "--token" && value) {
      options.tokenPath = value;
    } else if (argument === "--require" && value) {
      options.requiredCapabilities.push(value);
    } else if (argument === "--help") {
      return { help: true };
    } else {
      throw new Error(`unknown or incomplete argument: ${argument}`);
    }
    index += 1;
  }
  return options;
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.help) {
    await writeStream(
      process.stdout,
      "usage: input-companion-conformance [--socket PATH] [--token PATH] [--require CAPABILITY]...\n",
    );
    return;
  }
  const report = await inspectInputCompanionBridge(options);
  await writeStream(process.stdout, JSON.stringify(report) + "\n");
}

function writeStream(stream, value) {
  return new Promise((resolveWrite, rejectWrite) => {
    stream.write(value, (error) => {
      if (error) {
        rejectWrite(error);
      } else {
        resolveWrite();
      }
    });
  });
}

async function isMainModule() {
  if (!process.argv[1]) {
    return false;
  }
  try {
    return (
      (await realpath(resolve(process.argv[1]))) ===
      (await realpath(fileURLToPath(import.meta.url)))
    );
  } catch {
    return false;
  }
}

if (await isMainModule()) {
  try {
    await main();
  } catch (error) {
    await writeStream(process.stderr, error.message + "\n");
    process.exitCode = 1;
  }
}

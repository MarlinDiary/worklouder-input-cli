import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { execFile, spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

export class InspectorClient {
  static async connect(webSocketDebuggerUrl) {
    if (typeof WebSocket !== "function") {
      throw new Error("This installer requires Node.js with global WebSocket support");
    }
    const socket = new WebSocket(webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", reject, { once: true });
    });
    return new InspectorClient(socket);
  }

  constructor(socket) {
    this.socket = socket;
    this.nextId = 1;
    this.pending = new Map();
    this.events = [];
    this.waiters = [];
    socket.addEventListener("message", (event) => this.#onMessage(event));
    socket.addEventListener("close", () => {
      const error = new Error("Inspector connection closed");
      for (const pending of this.pending.values()) pending.reject(error);
      this.pending.clear();
    });
  }

  async command(method, params = {}) {
    const id = this.nextId++;
    const result = new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
    this.socket.send(JSON.stringify({ id, method, params }));
    return result;
  }

  async evaluate(expression, { timeout = 30_000 } = {}) {
    const response = await this.command("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      timeout,
    });
    return unwrapRemoteResult(response);
  }

  waitForEvent(method, predicate = () => true, timeoutMs = 30_000) {
    const queued = this.events.findIndex(
      (event) => event.method === method && predicate(event.params),
    );
    if (queued >= 0) {
      const [event] = this.events.splice(queued, 1);
      return Promise.resolve(event.params);
    }
    return new Promise((resolve, reject) => {
      const waiter = { method, predicate, resolve, reject };
      this.waiters.push(waiter);
      waiter.timer = setTimeout(() => {
        this.waiters = this.waiters.filter((candidate) => candidate !== waiter);
        reject(new Error(`Timed out waiting for inspector event ${method}`));
      }, timeoutMs);
    });
  }

  close() {
    this.socket.close();
  }

  #onMessage(event) {
    const message = JSON.parse(event.data);
    if (message.id !== undefined) {
      const pending = this.pending.get(message.id);
      if (!pending) return;
      this.pending.delete(message.id);
      if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
      else pending.resolve(message.result);
      return;
    }
    if (!message.method) return;
    const waiterIndex = this.waiters.findIndex(
      (waiter) =>
        waiter.method === message.method && waiter.predicate(message.params),
    );
    if (waiterIndex >= 0) {
      const [waiter] = this.waiters.splice(waiterIndex, 1);
      clearTimeout(waiter.timer);
      waiter.resolve(message.params);
      return;
    }
    this.events.push(message);
    if (this.events.length > 500) this.events.shift();
  }
}

export async function inspectorTarget(port, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`);
      if (!response.ok) throw new Error(`inspector returned ${response.status}`);
      const targets = await response.json();
      const target = targets.find((candidate) => candidate.webSocketDebuggerUrl);
      if (target) return target;
      lastError = new Error("inspector published no debuggable target");
    } catch (error) {
      lastError = error;
    }
    await sleep(100);
  }
  throw new Error(
    `Inspector 127.0.0.1:${port} did not become ready: ${errorMessage(lastError)}`,
  );
}

export async function exactProcessIds(executable) {
  const { stdout } = await execFilePromise("ps", ["-axo", "pid=,command="]);
  return stdout
    .split("\n")
    .map((line) => line.trim().match(/^(\d+)\s+(.+)$/))
    .filter((match) => match && match[2].split(" ")[0] === executable)
    .map((match) => Number(match[1]));
}

export async function terminateExactProcess(executable, timeoutMs = 15_000) {
  const pids = await exactProcessIds(executable);
  for (const pid of pids) process.kill(pid, "SIGTERM");
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await exactProcessIds(executable)).length === 0) return pids;
    await sleep(100);
  }
  for (const pid of await exactProcessIds(executable)) process.kill(pid, "SIGKILL");
  return pids;
}

export function spawnDetached(executable, args, env = process.env) {
  const child = spawn(executable, args, {
    detached: true,
    stdio: "ignore",
    env,
  });
  child.unref();
  return child.pid;
}

export async function sha256File(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

export async function plistValue(plist, key) {
  const { stdout } = await execFilePromise("plutil", [
    "-extract",
    key,
    "raw",
    "-o",
    "-",
    plist,
  ]);
  return stdout.trim();
}

export function sourceLocation(source, offset) {
  const prefix = source.slice(0, offset);
  const lines = prefix.split("\n");
  return { lineNumber: lines.length - 1, columnNumber: lines.at(-1).length };
}

export function unwrapRemoteResult(response) {
  if (response.exceptionDetails) {
    throw new Error(
      response.exceptionDetails.exception?.description ??
        response.exceptionDetails.text ??
        "Inspector evaluation failed",
    );
  }
  if (response.result?.subtype === "error") {
    throw new Error(response.result.description ?? "Inspector evaluation failed");
  }
  return response.result?.value;
}

export function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`${label} mismatch: expected ${expected}, detected ${actual}`);
  }
}

function execFilePromise(file, args) {
  return new Promise((resolve, reject) => {
    execFile(file, args, { encoding: "utf8" }, (error, stdout, stderr) => {
      if (error) {
        reject(new Error(`${file} failed: ${stderr.trim() || error.message}`));
      } else {
        resolve({ stdout, stderr });
      }
    });
  });
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

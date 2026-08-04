import { createHash } from "node:crypto";
import { chmod, lstat, mkdir, open, readFile, rename, rm } from "node:fs/promises";
import { dirname, join } from "node:path";

const KIND = "worklouderctl-codex-device-idempotency-registry";
const MAX_ENTRIES = 256;

export async function bindCodexDeviceIdempotency({
  key,
  operation,
  baselineRevision,
  targetRevision,
  root = defaultRoot(),
}) {
  validateKey(key);
  if (!['apply', 'restore'].includes(operation)) {
    throw new Error("Codex device idempotency operation was invalid");
  }
  for (const [name, value] of [
    ["baselineRevision", baselineRevision],
    ["targetRevision", targetRevision],
  ]) {
    if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
      throw new Error(`${name} was invalid`);
    }
  }
  await mkdir(root, { recursive: true, mode: 0o700 });
  await chmod(root, 0o700);
  const path = join(root, "codex-device-v1.json");
  const registry = await readRegistry(path);
  const requestDigest = digest({ operation, baselineRevision, targetRevision });
  const existing = registry.entries.find((entry) => entry.key === key);
  if (existing) {
    if (existing.requestDigest !== requestDigest) {
      throw new Error("idempotency key was reused with a different Codex device mutation");
    }
    return { path, requestDigest, replay: true, sequence: existing.sequence };
  }
  const entry = {
    key,
    requestDigest,
    operation,
    baselineRevision,
    targetRevision,
    sequence: registry.nextSequence,
  };
  registry.nextSequence += 1;
  registry.entries.push(entry);
  if (registry.entries.length > MAX_ENTRIES) {
    registry.entries.splice(0, registry.entries.length - MAX_ENTRIES);
  }
  await writeRegistry(path, registry);
  return { path, requestDigest, replay: false, sequence: entry.sequence };
}

function defaultRoot() {
  const home = process.env.HOME;
  if (!home) throw new Error("HOME is required");
  return process.env.WORKLOUDERCTL_IDEMPOTENCY_ROOT ??
    join(home, "Library/Application Support/worklouderctl/idempotency");
}

async function readRegistry(path) {
  try {
    const metadata = await lstat(path);
    if (!metadata.isFile() || metadata.isSymbolicLink()) {
      throw new Error("Codex device idempotency registry was not a regular file");
    }
    const registry = JSON.parse(await readFile(path, "utf8"));
    validateRegistry(registry);
    return registry;
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
    return { schemaVersion: 1, kind: KIND, nextSequence: 1, entries: [] };
  }
}

async function writeRegistry(path, registry) {
  validateRegistry(registry);
  const staging = join(
    dirname(path),
    `.codex-device-v1.${process.pid}.${Date.now()}.tmp`,
  );
  const handle = await open(staging, "wx", 0o600);
  try {
    await handle.writeFile(`${JSON.stringify(registry)}\n`);
    await handle.sync();
  } finally {
    await handle.close();
  }
  try {
    await rename(staging, path);
    await chmod(path, 0o600);
    const reopened = JSON.parse(await readFile(path, "utf8"));
    validateRegistry(reopened);
    if (JSON.stringify(reopened) !== JSON.stringify(registry)) {
      throw new Error("Codex device idempotency registry readback differed");
    }
  } finally {
    await rm(staging, { force: true });
  }
}

function validateRegistry(registry) {
  if (
    registry?.schemaVersion !== 1 ||
    registry.kind !== KIND ||
    !Number.isSafeInteger(registry.nextSequence) ||
    registry.nextSequence < 1 ||
    !Array.isArray(registry.entries) ||
    registry.entries.length > MAX_ENTRIES
  ) {
    throw new Error("Codex device idempotency registry was invalid");
  }
  const keys = new Set();
  for (const entry of registry.entries) {
    validateKey(entry?.key);
    if (
      keys.has(entry.key) ||
      !/^[0-9a-f]{64}$/.test(entry.requestDigest ?? "") ||
      !['apply', 'restore'].includes(entry.operation) ||
      !/^[0-9a-f]{64}$/.test(entry.baselineRevision ?? "") ||
      !/^[0-9a-f]{64}$/.test(entry.targetRevision ?? "") ||
      !Number.isSafeInteger(entry.sequence) ||
      entry.sequence < 1
    ) {
      throw new Error("Codex device idempotency registry entry was invalid");
    }
    keys.add(entry.key);
  }
}

function validateKey(key) {
  if (
    typeof key !== "string" ||
    key.length < 1 ||
    key.length > 256 ||
    key.includes("\0")
  ) {
    throw new Error("idempotency key was invalid");
  }
}

function digest(value) {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex");
}

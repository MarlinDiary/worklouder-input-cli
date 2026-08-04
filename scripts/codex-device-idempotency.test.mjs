import assert from "node:assert/strict";
import { mkdtemp, readFile, stat } from "node:fs/promises";
import test from "node:test";
import { bindCodexDeviceIdempotency } from "./codex-device-idempotency.mjs";

const A = "a".repeat(64);
const B = "b".repeat(64);
const C = "c".repeat(64);

test("persistent Codex device idempotency binds a key to one mutation", async () => {
  const root = await mkdtemp("/tmp/worklouderctl-idempotency-");
  const first = await bindCodexDeviceIdempotency({
    key: "fixture-key",
    operation: "apply",
    baselineRevision: A,
    targetRevision: B,
    root,
  });
  const replay = await bindCodexDeviceIdempotency({
    key: "fixture-key",
    operation: "apply",
    baselineRevision: A,
    targetRevision: B,
    root,
  });
  assert.equal(first.replay, false);
  assert.equal(replay.replay, true);
  assert.equal(replay.requestDigest, first.requestDigest);
  await assert.rejects(
    bindCodexDeviceIdempotency({
      key: "fixture-key",
      operation: "apply",
      baselineRevision: A,
      targetRevision: C,
      root,
    }),
    /reused with a different/,
  );
  const registryPath = `${root}/codex-device-v1.json`;
  assert.equal((await stat(root)).mode & 0o777, 0o700);
  assert.equal((await stat(registryPath)).mode & 0o777, 0o600);
  assert.equal(JSON.parse(await readFile(registryPath, "utf8")).entries.length, 1);
});

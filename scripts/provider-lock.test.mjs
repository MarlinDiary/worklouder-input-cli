import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { acquireProviderLock } from "./provider-lock.mjs";

test("serializes provider operations and permits reacquisition", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "worklouderctl-provider-lock-"));
  const lockPath = join(root, "provider.lock");
  t.after(() => rm(root, { recursive: true, force: true }));

  const first = await acquireProviderLock({ lockPath, mode: "codex" });
  await assert.rejects(
    acquireProviderLock({
      lockPath,
      mode: "input",
      timeoutMs: 80,
      pollMs: 10,
    }),
    /Timed out waiting for provider handoff lock/,
  );
  await first.release();

  const second = await acquireProviderLock({ lockPath, mode: "input" });
  assert.equal(second.owner.mode, "input");
  await second.release();
});

test("reclaims a lock whose owner process is gone", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "worklouderctl-provider-stale-"));
  const lockPath = join(root, "provider.lock");
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(lockPath);
  await writeFile(
    join(lockPath, "owner.json"),
    JSON.stringify({ pid: 2_147_483_647, mode: "stale", nonce: "old" }),
  );

  const lock = await acquireProviderLock({ lockPath, mode: "status" });
  assert.equal(lock.owner.mode, "status");
  assert.notEqual(lock.owner.nonce, "old");
  await lock.release();
});

import { randomUUID } from "node:crypto";
import { mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

export async function acquireProviderLock({
  lockPath,
  mode,
  timeoutMs = 30_000,
  staleAfterMs = 120_000,
  pollMs = 100,
}) {
  const ownerPath = join(lockPath, "owner.json");
  const nonce = randomUUID();
  const deadline = Date.now() + timeoutMs;
  await mkdir(dirname(lockPath), { recursive: true, mode: 0o700 });

  while (true) {
    try {
      await mkdir(lockPath, { mode: 0o700 });
      const owner = {
        pid: process.pid,
        mode,
        nonce,
        startedAt: new Date().toISOString(),
      };
      await writeFile(ownerPath, `${JSON.stringify(owner)}\n`, {
        encoding: "utf8",
        mode: 0o600,
      });
      return {
        owner,
        async release() {
          const current = await readOwner(ownerPath);
          if (current?.nonce === nonce) {
            await rm(lockPath, { recursive: true, force: true });
          }
        },
      };
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
    }

    if (await isStale(lockPath, ownerPath, staleAfterMs)) {
      await rm(lockPath, { recursive: true, force: true });
      continue;
    }
    if (Date.now() >= deadline) {
      const owner = await readOwner(ownerPath);
      throw new Error(
        `Timed out waiting for provider handoff lock: ${JSON.stringify(owner)}`,
      );
    }
    await sleep(pollMs);
  }
}

async function isStale(lockPath, ownerPath, staleAfterMs) {
  const owner = await readOwner(ownerPath);
  if (owner?.pid && !isProcessAlive(owner.pid)) return true;
  try {
    const metadata = await stat(lockPath);
    return Date.now() - metadata.mtimeMs > staleAfterMs;
  } catch (error) {
    return error?.code === "ENOENT";
  }
}

async function readOwner(ownerPath) {
  try {
    return JSON.parse(await readFile(ownerPath, "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT" || error instanceof SyntaxError) return null;
    throw error;
  }
}

function isProcessAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error?.code === "EPERM";
  }
}

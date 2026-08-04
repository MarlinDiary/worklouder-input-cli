#!/usr/bin/env node

import { createHash } from "node:crypto";
import { access, readFile, writeFile } from "node:fs/promises";
import {
  InspectorClient,
  assertEqual,
  exactProcessIds,
  inspectorTargetForProcess,
  plistValue,
  sha256File,
  unwrapRemoteResult,
  waitForInspectorPortRelease,
} from "./live-bridge-cdp.mjs";
import { acquireProviderLock } from "./provider-lock.mjs";
import { bindCodexDeviceIdempotency } from "./codex-device-idempotency.mjs";

const CODEX_EXECUTABLE = "/Applications/ChatGPT.app/Contents/MacOS/ChatGPT";
const CODEX_PLIST = "/Applications/ChatGPT.app/Contents/Info.plist";
const CODEX_ASAR = "/Applications/ChatGPT.app/Contents/Resources/app.asar";
const CODEX_MAIN =
  "/Applications/ChatGPT.app/Contents/Resources/app.asar/.vite/build/src-CLstCQVF.js";
const EXPECTED_CODEX_VERSION = "26.727.51351";
const EXPECTED_ASAR_SHA256 =
  "a529edd72e10b08931c0d695b5e3e6a0be7f51874610dafc04f578436ab7d74d";
const SERVICE_MODULE = "./service-4uQDVZZZ.js";
const PORT = 9229;
const DEVICE_KIT_VERSION = "0.1.28";
const CONFIG_REVISION_ALGORITHM =
  "sha256:path-u32be-path-bytes-size-u64be-content-v1";
const CONFIG_FILES = new Set(["keymap.json", "smart_actions.json"]);
const MAX_CONFIG_FILE_BYTES = 16 * 1024 * 1024;
const MAX_CONFIG_TOTAL_BYTES = 32 * 1024 * 1024;
const PROVIDER_LOCK = `${process.env.HOME}/Library/Application Support/worklouderctl/provider-handoff.lock`;

const [command = "status", ...argv] = process.argv.slice(2);
const options = parseOptions(argv);

if (options.help || !["status", "snapshot", "apply", "focus"].includes(command)) {
  console.log(
    "usage: codex-device-rpc.mjs <status|snapshot|apply|focus> [options]\n" +
      "  snapshot --output PATH\n" +
      "  apply --baseline PATH --input PATH [--output PATH]\n" +
      "  focus --name NAME --process BUNDLE_ID --path APP_PATH [--expect-layer N] [--output PATH]",
  );
  process.exit(options.help ? 0 : 2);
}

await access(CODEX_EXECUTABLE);
assertEqual(
  await plistValue(CODEX_PLIST, "CFBundleShortVersionString"),
  EXPECTED_CODEX_VERSION,
  "Codex version",
);
assertEqual(await sha256File(CODEX_ASAR), EXPECTED_ASAR_SHA256, "Codex app.asar SHA-256");

const providerLock = await acquireProviderLock({
  lockPath: PROVIDER_LOCK,
  mode: `codex-device-${command}`,
});
try {
const result = await withConnectedCodexService(async (client, instances) => {
  if (command === "status" || command === "snapshot") {
    return callInstances(client, instances, async function (payload) {
      const service = connectedService(this);
      const api = service.api.api;
      const files = [];
      if (payload.includeFiles) {
        for (const entry of await api.getFileList({ recursive: true })) {
          const relativePath = entry?.path ?? entry?.name;
          if (relativePath == null) continue;
          const data = await api.readFileChunked(relativePath);
          if (data == null) throw new Error(`failed to read ${relativePath}`);
          files.push({
            relativePath,
            size: data.length,
            dataBase64: Buffer.from(data).toString("base64"),
          });
        }
      }
      return {
        service: serviceState(service),
        deviceStatus: await api.getDeviceStatus(),
        files,
      };

      function connectedService(instances) {
        const service = instances.find(
          (value) =>
            value?.api?.api &&
            value?.comm &&
            value.getState?.().status === "connected",
        );
        if (service == null) throw new Error("connected Codex service missing");
        return service;
      }
      function serviceState(service) {
        return {
          lifecycleState: service.lifecycleState,
          deviceState: service.getState(),
          connectionAttemptId: service.connectionAttemptId,
          hasComm: service.comm != null,
          hasApi: service.api != null,
          hasHidSubscription: service.unsubscribeHid != null,
          hasJoystickSubscription: service.unsubscribeJoystick != null,
        };
      }
    }, { includeFiles: command === "snapshot" });
  }

  if (command === "apply") {
    const baseline = await loadSnapshot(required(options, "baseline"));
    const candidate = await loadSnapshot(required(options, "input"));
    const expected = snapshotFiles(baseline);
    const desired = snapshotFiles(candidate);
    assertSamePaths(expected, desired);
    const idempotency = await bindCodexDeviceIdempotency({
      key: options["idempotency-key"] ?? `codex-device-${candidate.revision}`,
      operation: options.operation ?? "apply",
      baselineRevision: baseline.revision,
      targetRevision: candidate.revision,
    });
    const mutation = await callInstances(
      client,
      instances,
      async function (payload) {
        const service = connectedService(this);
        const serviceApi = service.api;
        const comm = service.comm;
        const connectionAttemptId = service.connectionAttemptId;
        const api = serviceApi.api;
        const beforeStatus = await api.getDeviceStatus();
        const before = await readFiles(api, payload.expected);
        if (hashesMatch(before, payload.desired)) {
          return {
            operation: "apply",
            idempotentReplay: true,
            changedPaths: [],
            beforeStatus,
            afterStatus: beforeStatus,
            before,
            after: before,
            continuity: continuity(service, serviceApi, comm, connectionAttemptId),
            rollback: null,
          };
        }
        assertHashes("live baseline", before, payload.expected);
        const changed = payload.desired.filter(
          (file) =>
            payload.expected.find(
              (entry) => entry.relativePath === file.relativePath,
            ).sha256 !== file.sha256,
        );
        let rollback = null;
        try {
          await writeFiles(api, changed);
          const after = await readFiles(api, payload.desired);
          assertHashes("modified readback", after, payload.desired);
          const afterStatus = await api.getDeviceStatus();
          return {
            operation: "apply",
            idempotentReplay: false,
            changedPaths: changed.map((file) => file.relativePath),
            beforeStatus,
            afterStatus,
            before,
            after,
            continuity: continuity(service, serviceApi, comm, connectionAttemptId),
            rollback,
          };
        } catch (error) {
          try {
            const restore = payload.expected.filter((file) =>
              changed.some((entry) => entry.relativePath === file.relativePath),
            );
            const failedState = await readFiles(api, restore);
            if (hashesMatch(failedState, restore)) {
              rollback = {
                restored: true,
                notRequired: true,
                files: failedState,
              };
            } else {
              await writeFiles(api, restore);
              const restored = await readFiles(api, payload.expected);
              assertHashes("automatic rollback", restored, payload.expected);
              rollback = { restored: true, notRequired: false, files: restored };
            }
          } catch (rollbackError) {
            rollback = { restored: false, error: errorMessage(rollbackError) };
          }
          throw new Error(
            `${errorMessage(error)}; rollback=${JSON.stringify(rollback)}`,
          );
        }

        function connectedService(instances) {
          const service = instances.find(
            (value) =>
              value?.api?.api &&
              value?.comm &&
              value.getState?.().status === "connected",
          );
          if (service == null) throw new Error("connected Codex service missing");
          return service;
        }
        async function readFiles(api, expectedFiles) {
          const files = [];
          for (const expected of expectedFiles) {
            const data = await api.readFileChunked(expected.relativePath);
            if (data == null) throw new Error(`failed to read ${expected.relativePath}`);
            const buffer = Buffer.from(data);
            files.push({
              relativePath: expected.relativePath,
              size: buffer.length,
              sha256: process
                .getBuiltinModule("crypto")
                .createHash("sha256")
                .update(buffer)
                .digest("hex"),
            });
          }
          return files;
        }
        function assertHashes(label, actual, expected) {
          for (const file of expected) {
            const observed = actual.find(
              (entry) => entry.relativePath === file.relativePath,
            );
            if (
              observed == null ||
              observed.size !== file.size ||
              observed.sha256 !== file.sha256
            ) {
              throw new Error(
                `${label} mismatch for ${file.relativePath}: expected ` +
                  `${file.size}/${file.sha256}, observed ` +
                  `${observed?.size ?? "missing"}/${observed?.sha256 ?? "missing"}`,
              );
            }
          }
        }
        function hashesMatch(actual, expected) {
          return expected.every((file) => {
            const observed = actual.find(
              (entry) => entry.relativePath === file.relativePath,
            );
            return observed?.size === file.size && observed?.sha256 === file.sha256;
          });
        }
        async function writeFiles(api, files) {
          if (files.length === 0) return;
          if (files.length === 1) {
            const [file] = files;
            const ok = await api.writeFileChunked(
              file.relativePath,
              Buffer.from(file.dataBase64, "base64"),
            );
            if (ok !== true) throw new Error(`failed to write ${file.relativePath}`);
            return;
          }
          const transactionId = await api.beginMultifileWrite();
          if (transactionId == null) {
            throw new Error(
              "device firmware does not provide atomic multi-file writes",
            );
          }
          for (const file of files) {
            const ok = await api.writeFileChunked(
              file.relativePath,
              Buffer.from(file.dataBase64, "base64"),
              transactionId,
            );
            if (ok !== true) throw new Error(`failed to write ${file.relativePath}`);
          }
          if ((await api.commitMultifileWrite(transactionId)) !== true) {
            throw new Error("failed to commit filesystem transaction");
          }
        }
        function continuity(service, api, comm, attempt) {
          return {
            sameServiceApi: service.api === api,
            sameComm: service.comm === comm,
            sameConnectionAttempt: service.connectionAttemptId === attempt,
            lifecycleState: service.lifecycleState,
            deviceState: service.getState(),
            hasHidSubscription: service.unsubscribeHid != null,
            hasJoystickSubscription: service.unsubscribeJoystick != null,
          };
        }
        function errorMessage(error) {
          return error instanceof Error ? error.message : String(error);
        }
      },
      { expected, desired },
    );
    return { ...mutation, idempotency };
  }

  const app = {
    appName: required(options, "name"),
    process: required(options, "process"),
    path: required(options, "path"),
  };
  const expectLayer = options["expect-layer"] == null
    ? null
    : Number(options["expect-layer"]);
  if (expectLayer != null && !Number.isInteger(expectLayer)) {
    throw new Error("--expect-layer must be an integer");
  }
  return callInstances(
    client,
    instances,
    async function (payload) {
      const service = this.find(
        (value) =>
          value?.api?.api &&
          value?.comm &&
          value.getState?.().status === "connected",
      );
      if (service == null) throw new Error("connected Codex service missing");
      const serviceApi = service.api;
      const comm = service.comm;
      const connectionAttemptId = service.connectionAttemptId;
      const beforeStatus = await serviceApi.api.getDeviceStatus();
      await serviceApi.api.sendFocusApp(payload.app);
      let afterStatus = await serviceApi.api.getDeviceStatus();
      const deadline = Date.now() + 2_000;
      while (
        payload.expectLayer != null &&
        afterStatus.selectedLayerIndex !== payload.expectLayer &&
        Date.now() < deadline
      ) {
        await new Promise((resolve) => setTimeout(resolve, 50));
        afterStatus = await serviceApi.api.getDeviceStatus();
      }
      if (
        payload.expectLayer != null &&
        afterStatus.selectedLayerIndex !== payload.expectLayer
      ) {
        throw new Error(
          `expected layer ${payload.expectLayer}, observed ${afterStatus.selectedLayerIndex}`,
        );
      }
      return {
        operation: "focus",
        app: payload.app,
        beforeStatus,
        afterStatus,
        continuity: {
          sameServiceApi: service.api === serviceApi,
          sameComm: service.comm === comm,
          sameConnectionAttempt: service.connectionAttemptId === connectionAttemptId,
          lifecycleState: service.lifecycleState,
          deviceState: service.getState(),
          hasHidSubscription: service.unsubscribeHid != null,
          hasJoystickSubscription: service.unsubscribeJoystick != null,
        },
      };
    },
    { app, expectLayer },
  );
});

const normalized = command === "snapshot" ? makeSnapshot(result) : result;
if (options.output != null) {
  await writeFile(options.output, `${JSON.stringify(normalized, null, 2)}\n`,
    command === "snapshot" ? { flag: "wx", mode: 0o600 } : undefined);
}
console.log(JSON.stringify(normalized, null, 2));
} finally {
  await providerLock.release();
}

async function withConnectedCodexService(callback) {
  const pids = await exactProcessIds(CODEX_EXECUTABLE);
  if (pids.length !== 1) {
    throw new Error(`expected one running Codex process, detected ${pids.length}`);
  }
  const { target } = await inspectorTargetForProcess({
    port: PORT,
    pid: pids[0],
    executable: CODEX_EXECUTABLE,
  });
  const client = await InspectorClient.connect(target.webSocketDebuggerUrl);
  const objectGroup = "worklouderctl-codex-device-rpc";
  try {
    const evaluated = await client.command("Runtime.evaluate", {
      expression:
        `(()=>{const require=process.getBuiltinModule('module').createRequire(${JSON.stringify(CODEX_MAIN)});` +
        `return require(${JSON.stringify(SERVICE_MODULE)}).CodexMicroService.prototype})()`,
      objectGroup,
    });
    const prototype = evaluated.result.objectId;
    if (prototype == null) throw new Error("Codex service prototype missing");
    const queried = await client.command("Runtime.queryObjects", {
      prototypeObjectId: prototype,
      objectGroup,
    });
    const instances = queried.objects.objectId;
    if (instances == null) throw new Error("Codex service instances missing");
    return await callback(client, instances);
  } finally {
    await client
      .command("Runtime.releaseObjectGroup", { objectGroup })
      .catch(() => null);
    const closeScheduled = await client
      .evaluate(
        `(()=>{const inspector=process.getBuiltinModule('inspector');` +
          `process.once('SIGUSR1',()=>inspector.open(${PORT},'127.0.0.1',false));` +
          `setTimeout(()=>inspector.close(),250);return true})()`,
      )
      .catch(() => false);
    client.close();
    if (closeScheduled) await waitForInspectorPortRelease(PORT);
  }
}

async function callInstances(client, objectId, implementation, argument) {
  const called = await client.command("Runtime.callFunctionOn", {
    objectId,
    functionDeclaration: implementation.toString(),
    arguments: argument == null ? [] : [{ value: argument }],
    returnByValue: true,
    awaitPromise: true,
    objectGroup: "worklouderctl-codex-device-rpc",
  });
  return unwrapRemoteResult(called);
}

function makeSnapshot(remote) {
  const files = remote.files
    .map((file) => {
      const data = Buffer.from(file.dataBase64, "base64");
      return {
        relativePath: file.relativePath,
        size: data.length,
        deviceChecksumSha1: createHash("sha1").update(data).digest("hex"),
        sha256: sha256(data),
        dataBase64: file.dataBase64,
      };
    })
    .sort((left, right) => left.relativePath.localeCompare(right.relativePath));
  return {
    schemaVersion: 1,
    kind: "worklouder-input-config-snapshot",
    revisionAlgorithm: CONFIG_REVISION_ALGORITHM,
    revision: revision(files),
    deviceId: "codex-owner",
    deviceKitVersion: DEVICE_KIT_VERSION,
    device: {
      devicePid: "33632",
      deviceType: "codex_micro",
      layoutType: "universal",
      connectionType: "hid",
      isUsbConnection: remote.service.deviceState.transport === "usb",
    },
    status: remote.deviceStatus,
    warnings: [],
    files,
  };
}

async function loadSnapshot(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

function snapshotFiles(snapshot) {
  if (
    snapshot?.schemaVersion !== 1 ||
    snapshot.kind !== "worklouder-input-config-snapshot" ||
    snapshot.revisionAlgorithm !== CONFIG_REVISION_ALGORITHM
  ) {
    throw new Error("snapshot header was invalid");
  }
  if (!Array.isArray(snapshot.files)) throw new Error("snapshot files missing");
  const seen = new Set();
  let totalBytes = 0;
  const files = snapshot.files
    .map((file) => {
      if (!CONFIG_FILES.has(file.relativePath) || seen.has(file.relativePath)) {
        throw new Error(`invalid snapshot path: ${file.relativePath}`);
      }
      seen.add(file.relativePath);
      if (typeof file.dataBase64 !== "string") {
        throw new Error(`snapshot payload missing for ${file.relativePath}`);
      }
      const data = Buffer.from(file.dataBase64, "base64");
      if (data.toString("base64") !== file.dataBase64) {
        throw new Error(`noncanonical snapshot payload for ${file.relativePath}`);
      }
      totalBytes += data.length;
      if (
        data.length > MAX_CONFIG_FILE_BYTES ||
        totalBytes > MAX_CONFIG_TOTAL_BYTES
      ) {
        throw new Error("snapshot payload exceeded configuration limits");
      }
      const actual = sha256(data);
      if (actual !== file.sha256 || data.length !== file.size) {
        throw new Error(`invalid snapshot payload for ${file.relativePath}`);
      }
      return {
        relativePath: file.relativePath,
        size: file.size,
        sha256: file.sha256,
        dataBase64: file.dataBase64,
      };
    })
    .sort((left, right) => left.relativePath.localeCompare(right.relativePath));
  if (files.length !== CONFIG_FILES.size) {
    throw new Error("snapshot did not contain the complete configuration file set");
  }
  if (snapshot.revision !== revision(files)) {
    throw new Error("snapshot revision did not match content");
  }
  return files;
}

function assertSamePaths(left, right) {
  const leftPaths = left.map((file) => file.relativePath);
  const rightPaths = right.map((file) => file.relativePath);
  if (JSON.stringify(leftPaths) !== JSON.stringify(rightPaths)) {
    throw new Error(
      `snapshot paths differ: baseline=${JSON.stringify(leftPaths)} candidate=${JSON.stringify(rightPaths)}`,
    );
  }
}

function revision(files) {
  const hash = createHash("sha256");
  hash.update("worklouder-input-config-revision-v1\0", "utf8");
  for (const file of files) {
    const path = Buffer.from(file.relativePath, "utf8");
    const pathLength = Buffer.alloc(4);
    pathLength.writeUInt32BE(path.length);
    const content = Buffer.from(file.dataBase64, "base64");
    const contentLength = Buffer.alloc(8);
    contentLength.writeBigUInt64BE(BigInt(content.length));
    hash.update(pathLength);
    hash.update(path);
    hash.update(contentLength);
    hash.update(content);
  }
  return hash.digest("hex");
}

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

function required(values, name) {
  if (values[name] == null) throw new Error(`--${name} is required`);
  return values[name];
}

function parseOptions(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const token = args[index];
    if (token === "--help" || token === "-h") {
      parsed.help = true;
      continue;
    }
    if (!token.startsWith("--")) throw new Error(`unexpected argument: ${token}`);
    const name = token.slice(2);
    const value = args[index + 1];
    if (value == null || value.startsWith("--")) {
      throw new Error(`missing value for ${token}`);
    }
    parsed[name] = value;
    index += 1;
  }
  return parsed;
}

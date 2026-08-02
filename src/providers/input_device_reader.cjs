"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const ADAPTER = "input-bundled-device-kit-read-v1";

function emit(value) {
  process.stdout.write(JSON.stringify(value) + "\n");
}

function safeRelativePath(value) {
  if (typeof value !== "string" || value.length === 0 || value.includes("\0")) {
    throw new Error("device returned an empty or invalid file path");
  }
  const portable = value.replaceAll("\\", "/");
  if (path.posix.isAbsolute(portable)) {
    throw new Error(`device returned an absolute file path: ${value}`);
  }
  const parts = portable.split("/");
  if (parts.some((part) => part === "" || part === "." || part === "..")) {
    throw new Error(`device returned an unsafe file path: ${value}`);
  }
  return parts.join("/");
}

function publicDevice(device) {
  const connectionType =
    device.connectionType === 0
      ? "serial"
      : device.connectionType === 1
        ? "hid"
        : String(device.connectionType);
  return {
    devicePid: String(device.devicePid),
    deviceType: String(device.deviceType),
    layoutType: String(device.layoutType),
    connectionType,
    isUsbConnection: Boolean(device.isUsbConnection),
  };
}

async function connect(appPath) {
  const kitPath = path.join(
    appPath,
    "Contents/Resources/app.asar/node_modules/@worklouder/wl-device-kit",
  );
  const kit = require(kitPath);
  const packageJson = require(path.join(kitPath, "package.json"));
  const discovery = new kit.WLDeviceDiscovery();
  const devices = discovery
    .findWLDevices()
    .filter((candidate) => String(candidate.deviceType) === "codex_micro");
  if (devices.length !== 1) {
    throw new Error(`expected exactly one Codex Micro, found ${devices.length}`);
  }

  const comm = new kit.WLDeviceCommImpl();
  if (!(await comm.connect(devices[0]))) {
    throw new Error("the bundled Input provider did not connect to Codex Micro");
  }
  return {
    api: new kit.WLRPCApi(comm),
    comm,
    device: devices[0],
    deviceKitVersion: String(packageJson.version),
  };
}

async function commonSnapshot(connected) {
  const firmwareVersion = await connected.api.getFirmwareVersion();
  const status = await connected.api.getDeviceStatus();
  if (!status.firmwareVersion) {
    status.firmwareVersion = firmwareVersion;
  }
  return {
    adapter: ADAPTER,
    deviceKitVersion: connected.deviceKitVersion,
    device: publicDevice(connected.device),
    status,
  };
}

function normalizedFiles(files) {
  if (!Array.isArray(files)) {
    throw new Error("Input device provider returned a non-array file list");
  }
  return files.map((file) => ({
    name: safeRelativePath(file.name),
    size: Number(file.size),
    checksum: typeof file.checksum === "string" ? file.checksum.toLowerCase() : null,
  }));
}

async function main() {
  const [action, appPath, ...args] = process.argv.slice(2);
  if (!action || !appPath) {
    throw new Error("usage: provider ACTION INPUT_APP [ARGS]");
  }

  let connected;
  try {
    connected = await connect(appPath);
    const snapshot = await commonSnapshot(connected);

    if (action === "status") {
      emit({ ok: true, action, ...snapshot });
      return;
    }

    if (action === "files") {
      const requestedPath = args[0] === "-" ? undefined : args[0];
      const recursive = args[1] === "true";
      const files = normalizedFiles(
        await connected.api.getFileList({ path: requestedPath, recursive }),
      );
      emit({ ok: true, action, ...snapshot, files });
      return;
    }

    if (action === "snapshot") {
      const output = args[0];
      if (!output || !fs.statSync(output).isDirectory()) {
        throw new Error("snapshot output must be a pre-created directory");
      }
      const files = normalizedFiles(
        await connected.api.getFileList({ recursive: true }),
      );
      if (!files.some((file) => file.name === "keymap.json")) {
        throw new Error("live file list did not contain required keymap.json");
      }

      const captured = [];
      for (const file of files) {
        if (!Number.isSafeInteger(file.size) || file.size < 0) {
          throw new Error(`invalid size for ${file.name}`);
        }
        if (!file.checksum || !/^[0-9a-f]{40}$/.test(file.checksum)) {
          throw new Error(`invalid device SHA-1 for ${file.name}`);
        }
        const bytes = await connected.api.readFileChunked(file.name);
        if (!Buffer.isBuffer(bytes)) {
          throw new Error(`device returned no bytes for ${file.name}`);
        }
        if (bytes.length !== file.size) {
          throw new Error(`size mismatch for ${file.name}`);
        }
        const sha1 = crypto.createHash("sha1").update(bytes).digest("hex");
        if (sha1 !== file.checksum) {
          throw new Error(`device SHA-1 mismatch for ${file.name}`);
        }
        const sha256 = crypto.createHash("sha256").update(bytes).digest("hex");
        const destination = path.join(output, ...file.name.split("/"));
        fs.mkdirSync(path.dirname(destination), { recursive: true });
        const descriptor = fs.openSync(destination, "wx", 0o600);
        try {
          fs.writeFileSync(descriptor, bytes);
          fs.fsyncSync(descriptor);
        } finally {
          fs.closeSync(descriptor);
        }
        const reopened = fs.readFileSync(destination);
        if (
          reopened.length !== bytes.length ||
          crypto.createHash("sha256").update(reopened).digest("hex") !== sha256
        ) {
          throw new Error(`host readback mismatch for ${file.name}`);
        }
        captured.push({
          relativePath: file.name,
          size: file.size,
          deviceChecksumSha1: file.checksum,
          sha256,
        });
      }
      emit({ ok: true, action, ...snapshot, files: captured });
      return;
    }

    throw new Error(`unknown provider action: ${action}`);
  } finally {
    if (connected) {
      await connected.comm.disconnect();
    }
  }
}

main().catch((error) => {
  emit({ ok: false, error: error instanceof Error ? error.message : String(error) });
  process.exitCode = 1;
});

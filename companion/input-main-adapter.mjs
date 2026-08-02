import { createHash } from "node:crypto";
import { BridgeError } from "./input-main-bridge.mjs";

export const CONFIG_SNAPSHOT_SCHEMA_VERSION = 1;
export const CONFIG_SNAPSHOT_KIND = "worklouder-input-config-snapshot";
export const CONFIG_REVISION_ALGORITHM =
  "sha256:path-u32be-path-bytes-size-u64be-content-v1";

const MAX_CONFIG_FILES = 4096;
const MAX_CONFIG_FILE_BYTES = 16 * 1024 * 1024;
const MAX_CONFIG_TOTAL_BYTES = 32 * 1024 * 1024;

export function createInputMainAdapter({
  devicesCommManager,
  deviceKitVersion,
}) {
  if (
    !devicesCommManager ||
    typeof devicesCommManager.getDevices !== "function"
  ) {
    throw new TypeError("devicesCommManager.getDevices is required");
  }
  if (typeof deviceKitVersion !== "string" || deviceKitVersion.length === 0) {
    throw new TypeError("deviceKitVersion is required");
  }

  const selectDevice = (deviceId) => {
    const devices = devicesCommManager
      .getDevices()
      .filter((device) => device.isConnected());
    if (deviceId !== null && deviceId !== undefined) {
      const selected = devices.find(
        (device) => String(device.id) === String(deviceId),
      );
      if (!selected) {
        throw new BridgeError(-32004, "device not found", { deviceId });
      }
      return selected;
    }
    const codexMicro = devices.filter(
      (device) => String(device.info.deviceType) === "codex_micro",
    );
    if (codexMicro.length !== 1) {
      throw new BridgeError(
        -32004,
        "expected exactly one connected Codex Micro, found " +
          codexMicro.length,
      );
    }
    return codexMicro[0];
  };

  const common = async (device) => {
    const [firmwareVersion, status] = await Promise.all([
      device.rpcService.getFirmwareVersion(),
      device.rpcService.getDeviceStatus(),
    ]);
    if (
      status.firmwareVersion &&
      status.firmwareVersion !== firmwareVersion
    ) {
      throw new BridgeError(
        -32008,
        "sys.version and device.status firmware versions differed",
      );
    }
    return {
      deviceKitVersion,
      device: publicDevice(device.info),
      status: {
        firmwareVersion: status.firmwareVersion ?? firmwareVersion,
        selectedProfileIndex: optionalNumber(
          status.selectedProfileIndex ?? status.selected_profile_index,
        ),
        selectedLayerIndex: optionalNumber(
          status.selectedLayerIndex ?? status.selected_layer_index,
        ),
        batteryPercentage: optionalNumber(
          status.batteryPercentage ?? status.battery_percentage,
        ),
        isCharging:
          status.isCharging === undefined
            ? (status.is_charging ?? null)
            : status.isCharging,
      },
      warnings: [],
    };
  };

  const captureConfigSnapshot = async (device) => {
    const [status, firstListing] = await Promise.all([
      common(device),
      device.rpcService.getFileList({ recursive: true }),
    ]);
    const listed = normalizeFileList(firstListing);
    if (listed.length === 0) {
      throw new BridgeError(-32008, "device configuration contained no files");
    }
    const files = [];
    let totalBytes = 0;
    for (const file of listed) {
      const bytes = await device.rpcService.readFileChunked(file.relativePath);
      if (!Buffer.isBuffer(bytes)) {
        throw new BridgeError(-32008, "device returned no file bytes");
      }
      if (bytes.length !== file.size) {
        throw new BridgeError(
          -32006,
          "device configuration changed during snapshot",
        );
      }
      const sha1 = createHash("sha1").update(bytes).digest("hex");
      if (sha1 !== file.deviceChecksumSha1) {
        throw new BridgeError(
          -32006,
          "device configuration changed during snapshot",
        );
      }
      totalBytes += bytes.length;
      if (
        bytes.length > MAX_CONFIG_FILE_BYTES ||
        totalBytes > MAX_CONFIG_TOTAL_BYTES
      ) {
        throw new BridgeError(
          -32008,
          "device configuration exceeded snapshot limits",
        );
      }
      files.push({
        ...file,
        sha256: createHash("sha256").update(bytes).digest("hex"),
        dataBase64: bytes.toString("base64"),
      });
    }
    const secondListing = normalizeFileList(
      await device.rpcService.getFileList({ recursive: true }),
    );
    if (listingIdentity(listed) !== listingIdentity(secondListing)) {
      throw new BridgeError(-32006, "device configuration changed during snapshot");
    }
    return {
      schemaVersion: CONFIG_SNAPSHOT_SCHEMA_VERSION,
      kind: CONFIG_SNAPSHOT_KIND,
      revisionAlgorithm: CONFIG_REVISION_ALGORITHM,
      revision: configRevision(files),
      deviceId: String(device.id),
      ...status,
      files,
    };
  };

  return {
    async listDevices() {
      return {
        deviceKitVersion,
        devices: devicesCommManager.getDevices().map((device) => ({
          id: String(device.id),
          connected: Boolean(device.isConnected()),
          device: publicDevice(device.info),
        })),
      };
    },

    async getDeviceStatus({ deviceId = null }) {
      const device = selectDevice(deviceId);
      return common(device);
    },

    async listFiles({ deviceId = null, path, recursive = false }) {
      const device = selectDevice(deviceId);
      const [snapshot, files] = await Promise.all([
        common(device),
        device.rpcService.getFileList({
          path: path ?? undefined,
          recursive: Boolean(recursive),
        }),
      ]);
      if (!Array.isArray(files)) {
        throw new BridgeError(-32008, "device returned a non-array file list");
      }
      return {
        ...snapshot,
        files: files.map((file) => ({
          relativePath: safeRelativePath(file.name),
          size: safeSize(file.size),
          deviceChecksumSha1: safeSha1(file.checksum),
        })),
      };
    },

    async readFile({ deviceId = null, path }) {
      const device = selectDevice(deviceId);
      const relativePath = safeRelativePath(path);
      const bytes = await device.rpcService.readFileChunked(relativePath);
      if (!Buffer.isBuffer(bytes)) {
        throw new BridgeError(-32008, "device returned no file bytes");
      }
      return {
        relativePath,
        size: bytes.length,
        deviceChecksumSha1: createHash("sha1").update(bytes).digest("hex"),
        dataBase64: bytes.toString("base64"),
      };
    },

    async snapshotConfig({ deviceId = null }) {
      return captureConfigSnapshot(selectDevice(deviceId));
    },

    async validateConfig({
      deviceId = null,
      snapshot,
      expectedRevision = null,
    }) {
      const validation = validateConfigSnapshot(snapshot);
      if (
        deviceId !== null &&
        deviceId !== undefined &&
        String(deviceId) !== validation.deviceId
      ) {
        throw new BridgeError(-32602, "snapshot deviceId did not match request");
      }
      let liveRevision = null;
      if (expectedRevision !== null && expectedRevision !== undefined) {
        const expected = safeSha256(expectedRevision, "expected revision");
        const live = await captureConfigSnapshot(selectDevice(deviceId));
        liveRevision = live.revision;
        if (live.deviceId !== validation.deviceId || liveRevision !== expected) {
          throw new BridgeError(-32005, "device revision conflict", {
            expectedRevision: expected,
            liveRevision,
            snapshotDeviceId: validation.deviceId,
            liveDeviceId: live.deviceId,
          });
        }
      }
      return {
        schemaVersion: CONFIG_SNAPSHOT_SCHEMA_VERSION,
        kind: "worklouder-input-config-validation",
        valid: true,
        revision: validation.revision,
        liveRevision,
        fileCount: validation.fileCount,
        totalBytes: validation.totalBytes,
      };
    },
  };
}

function normalizeFileList(files) {
  if (!Array.isArray(files)) {
    throw new BridgeError(-32008, "device returned a non-array file list");
  }
  if (files.length > MAX_CONFIG_FILES) {
    throw new BridgeError(-32008, "device configuration had too many files");
  }
  const normalized = files.map((file) => ({
    relativePath: safeRelativePath(file.name),
    size: safeSize(file.size),
    deviceChecksumSha1: safeSha1(file.checksum),
  }));
  normalized.sort((left, right) =>
    comparePaths(left.relativePath, right.relativePath),
  );
  for (let index = 1; index < normalized.length; index += 1) {
    if (normalized[index - 1].relativePath === normalized[index].relativePath) {
      throw new BridgeError(-32008, "device returned duplicate file paths");
    }
  }
  return normalized;
}

function listingIdentity(files) {
  return JSON.stringify(
    files.map((file) => [
      file.relativePath,
      file.size,
      file.deviceChecksumSha1,
    ]),
  );
}

function validateConfigSnapshot(snapshot) {
  if (!snapshot || typeof snapshot !== "object" || Array.isArray(snapshot)) {
    throw new BridgeError(-32602, "snapshot must be an object");
  }
  if (
    snapshot.schemaVersion !== CONFIG_SNAPSHOT_SCHEMA_VERSION ||
    snapshot.kind !== CONFIG_SNAPSHOT_KIND ||
    snapshot.revisionAlgorithm !== CONFIG_REVISION_ALGORITHM ||
    typeof snapshot.deviceId !== "string" ||
    snapshot.deviceId.length === 0 ||
    !Array.isArray(snapshot.files) ||
    snapshot.files.length === 0 ||
    snapshot.files.length > MAX_CONFIG_FILES
  ) {
    throw new BridgeError(-32602, "snapshot header or file list was invalid");
  }
  const seen = new Set();
  let totalBytes = 0;
  const files = snapshot.files.map((file) => {
    if (!file || typeof file !== "object" || Array.isArray(file)) {
      throw new BridgeError(-32602, "snapshot file entry was invalid");
    }
    const relativePath = safeRelativePath(file.relativePath, -32602);
    if (seen.has(relativePath)) {
      throw new BridgeError(-32602, "snapshot contained duplicate file paths");
    }
    seen.add(relativePath);
    const bytes = decodeCanonicalBase64(file.dataBase64);
    const size = safeSize(file.size, -32602);
    totalBytes += bytes.length;
    if (
      size !== bytes.length ||
      bytes.length > MAX_CONFIG_FILE_BYTES ||
      totalBytes > MAX_CONFIG_TOTAL_BYTES
    ) {
      throw new BridgeError(-32602, "snapshot file size was invalid");
    }
    const deviceChecksumSha1 = safeSha1(file.deviceChecksumSha1, -32602);
    if (createHash("sha1").update(bytes).digest("hex") !== deviceChecksumSha1) {
      throw new BridgeError(-32602, "snapshot file SHA-1 did not match content");
    }
    const sha256 = safeSha256(file.sha256, "snapshot file SHA-256");
    if (createHash("sha256").update(bytes).digest("hex") !== sha256) {
      throw new BridgeError(-32602, "snapshot file SHA-256 did not match content");
    }
    return { relativePath, bytes };
  });
  const revision = configRevision(files);
  if (safeSha256(snapshot.revision, "snapshot revision") !== revision) {
    throw new BridgeError(-32602, "snapshot revision did not match content");
  }
  return {
    deviceId: snapshot.deviceId,
    revision,
    fileCount: files.length,
    totalBytes,
  };
}

function configRevision(files) {
  const hash = createHash("sha256");
  hash.update("worklouder-input-config-revision-v1\0", "utf8");
  const sorted = [...files].sort((left, right) =>
    comparePaths(left.relativePath, right.relativePath),
  );
  for (const file of sorted) {
    const path = Buffer.from(file.relativePath, "utf8");
    const pathLength = Buffer.alloc(4);
    pathLength.writeUInt32BE(path.length);
    const content = file.bytes ?? decodeCanonicalBase64(file.dataBase64);
    const contentLength = Buffer.alloc(8);
    contentLength.writeBigUInt64BE(BigInt(content.length));
    hash.update(pathLength);
    hash.update(path);
    hash.update(contentLength);
    hash.update(content);
  }
  return hash.digest("hex");
}

function decodeCanonicalBase64(value) {
  if (
    typeof value !== "string" ||
    value.length % 4 !== 0 ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(
      value,
    )
  ) {
    throw new BridgeError(
      -32602,
      "snapshot file content was not canonical base64",
    );
  }
  const bytes = Buffer.from(value, "base64");
  if (bytes.toString("base64") !== value) {
    throw new BridgeError(
      -32602,
      "snapshot file content was not canonical base64",
    );
  }
  return bytes;
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

function optionalNumber(value) {
  if (value === null || value === undefined) {
    return null;
  }
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < 0) {
    throw new BridgeError(-32008, "device returned an invalid numeric status");
  }
  return number;
}

function safeSize(value, code = -32008) {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < 0) {
    throw new BridgeError(code, "device returned an invalid file size");
  }
  return number;
}

function safeSha1(value, code = -32008) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/i.test(value)) {
    throw new BridgeError(code, "device returned an invalid SHA-1");
  }
  return value.toLowerCase();
}

function safeSha256(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/i.test(value)) {
    throw new BridgeError(-32602, label + " was invalid");
  }
  return value.toLowerCase();
}

function safeRelativePath(value, code = -32008) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    Buffer.byteLength(value, "utf8") > 4096 ||
    value.includes("\0") ||
    value.startsWith("/") ||
    value.startsWith("\\")
  ) {
    throw new BridgeError(code, "device returned an invalid file path");
  }
  const portable = value.replaceAll("\\", "/");
  const parts = portable.split("/");
  if (parts.some((part) => part === "" || part === "." || part === "..")) {
    throw new BridgeError(code, "device returned an unsafe file path");
  }
  return parts.join("/");
}

function comparePaths(left, right) {
  return Buffer.compare(Buffer.from(left, "utf8"), Buffer.from(right, "utf8"));
}

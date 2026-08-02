import { createHash } from "node:crypto";
import { BridgeError } from "./input-main-bridge.mjs";

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
  };
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

function safeSize(value) {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < 0) {
    throw new BridgeError(-32008, "device returned an invalid file size");
  }
  return number;
}

function safeSha1(value) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/i.test(value)) {
    throw new BridgeError(-32008, "device returned an invalid SHA-1");
  }
  return value.toLowerCase();
}

function safeRelativePath(value) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.includes("\0") ||
    value.startsWith("/") ||
    value.startsWith("\\")
  ) {
    throw new BridgeError(-32008, "device returned an invalid file path");
  }
  const portable = value.replaceAll("\\", "/");
  const parts = portable.split("/");
  if (parts.some((part) => part === "" || part === "." || part === "..")) {
    throw new BridgeError(-32008, "device returned an unsafe file path");
  }
  return parts.join("/");
}

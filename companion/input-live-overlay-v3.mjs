import { createHash } from "node:crypto";
import { installInputCompanionBridge } from "./input-main-integration-v3.mjs";

export const SUPPORTED_INPUT_VERSION = "0.18.0";
export const SUPPORTED_DEVICE_KIT_VERSION = "0.1.28";
export const INPUT_LIVE_OVERLAY_REVISION = 3;

/**
 * Install the reference bridge into Input's existing main-process service
 * container. The overlay delegates transport, cache refresh, host settings,
 * AppSense, permission, firmware discovery, and logs to the released app.
 */
export async function installInputLiveOverlay({
  app,
  services,
  deviceKitVersion = SUPPORTED_DEVICE_KIT_VERSION,
  bridgeVersion = "0.1.0-live-overlay",
  socketPath,
  tokenPath,
}) {
  assertVersion(app?.getVersion?.(), SUPPORTED_INPUT_VERSION, "Input");
  if (!services || typeof services !== "object") {
    throw new TypeError("Input services are required");
  }
  // Input's service container publishes its authorities through prototype
  // getters backed by private underscore fields. Preserve that prototype
  // chain instead of flattening only enumerable storage fields.
  const integratedServices = Object.create(services);
  integratedServices.configurationWriter =
    services.configurationWriter ?? createInputConfigurationWriter(services);
  integratedServices.firmwareAuthority =
    services.firmwareAuthority ?? createInputFirmwareAuthority(services);
  const bridge = await installInputCompanionBridge({
    app,
    services: integratedServices,
    deviceKitVersion,
    bridgeVersion,
    socketPath,
    tokenPath,
  });
  return { ...bridge, overlayRevision: INPUT_LIVE_OVERLAY_REVISION };
}

export function createInputFirmwareAuthority(services) {
  const deviceFlashService = services?.deviceFlashService;
  const applicationService = services?.applicationService;
  if (
    typeof deviceFlashService?.checkForFwUpdates !== "function" ||
    typeof deviceFlashService?.getLatestFwRelease !== "function"
  ) {
    return undefined;
  }
  return {
    async readStatus({ device }) {
      const currentVersion = await device.rpcService.getFirmwareVersion();
      const available = await deviceFlashService.checkForFwUpdates(
        currentVersion,
        device.info.deviceType,
      );
      let release =
        available && typeof available === "object" && !Array.isArray(available)
          ? available
          : null;
      if (available === true) {
        const appVersion =
          typeof applicationService?.appVersion === "function"
            ? await applicationService.appVersion()
            : "";
        release =
          (await deviceFlashService.getLatestFwRelease(
            device.info.deviceType,
            String(appVersion).includes("rc"),
          )) ?? null;
      }
      return {
        updateAvailable:
          available === undefined || available === null
            ? null
            : available === false
              ? false
              : true,
        release,
      };
    },
  };
}

export function createInputConfigurationWriter(services) {
  if (
    !services?.deviceFileService ||
    typeof services.deviceFileService.fetchDeviceFiles !== "function"
  ) {
    throw new TypeError("deviceFileService.fetchDeviceFiles is required");
  }
  return {
    async replaceConfiguration({ device, files }) {
      if (!device?.rpcService) {
        throw new TypeError("connected Input device rpcService is required");
      }
      const rpc = device.rpcService;
      for (const method of ["getFileList", "deleteFile", "writeFileChunked"]) {
        if (typeof rpc[method] !== "function") {
          throw new TypeError(`device.rpcService.${method} is required`);
        }
      }
      const targets = normalizeTargetFiles(files);
      const existing = normalizeDeviceListing(
        await rpc.getFileList({ recursive: true }),
      );
      const targetPaths = new Set(targets.map((file) => file.relativePath));

      for (const file of existing
        .filter((file) => !targetPaths.has(file.relativePath))
        .sort((left, right) => left.relativePath.localeCompare(right.relativePath))) {
        const deleted = await rpc.deleteFile(file.relativePath);
        if (deleted === false) {
          throw new Error(`Input failed to delete ${file.relativePath}`);
        }
      }

      const existingByPath = new Map(
        existing.map((file) => [file.relativePath, file.deviceChecksumSha1]),
      );
      for (const file of targets.sort(compareWriteOrder)) {
        const targetSha1 = createHash("sha1").update(file.bytes).digest("hex");
        if (existingByPath.get(file.relativePath) === targetSha1) {
          continue;
        }
        const written = await rpc.writeFileChunked(file.relativePath, file.bytes);
        if (written === false) {
          throw new Error(`Input failed to write ${file.relativePath}`);
        }
      }

      await services.deviceFileService.fetchDeviceFiles(device);
    },
  };
}

function normalizeTargetFiles(files) {
  if (!Array.isArray(files) || files.length === 0) {
    throw new TypeError("complete Input configuration files are required");
  }
  const seen = new Set();
  return files.map((file) => {
    const relativePath = safeRelativePath(file?.relativePath);
    if (seen.has(relativePath)) {
      throw new TypeError(`duplicate Input configuration path: ${relativePath}`);
    }
    seen.add(relativePath);
    const bytes = Buffer.isBuffer(file?.bytes)
      ? Buffer.from(file.bytes)
      : file?.bytes instanceof Uint8Array
        ? Buffer.from(file.bytes)
        : null;
    if (!bytes) {
      throw new TypeError(`Input configuration bytes are required: ${relativePath}`);
    }
    return { relativePath, bytes };
  });
}

function normalizeDeviceListing(value) {
  if (!Array.isArray(value)) {
    throw new Error("Input device file listing was invalid");
  }
  return value.map((file) => ({
    relativePath: safeRelativePath(file?.relativePath ?? file?.name),
    deviceChecksumSha1:
      typeof file?.deviceChecksumSha1 === "string"
        ? file.deviceChecksumSha1.toLowerCase()
        : typeof file?.checksum === "string"
          ? file.checksum.toLowerCase()
          : null,
  }));
}

function safeRelativePath(value) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 4096 ||
    value.startsWith("/") ||
    value.includes("\\") ||
    value.includes("\0") ||
    value.split("/").some((part) => part === "" || part === "." || part === "..")
  ) {
    throw new TypeError("Input configuration path is invalid");
  }
  return value;
}

function compareWriteOrder(left, right) {
  return writePriority(left.relativePath) - writePriority(right.relativePath) ||
    left.relativePath.localeCompare(right.relativePath);
}

function writePriority(path) {
  if (path === "smart_actions.json") return 0;
  if (path === "keymap.json") return 2;
  return 1;
}

function assertVersion(actual, expected, provider) {
  if (actual !== expected) {
    throw new Error(
      `${provider} live overlay supports ${expected}; detected ${String(actual)}`,
    );
  }
}

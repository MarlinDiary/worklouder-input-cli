import { join } from "node:path";
import { platform } from "node:os";
import { createInputMainAdapter } from "./input-main-adapter.mjs";
import { startInputCompanionBridge } from "./input-main-bridge.mjs";

export async function installInputCompanionBridge({
  app,
  services,
  deviceKitVersion,
  bridgeVersion = "0.1.0",
  socketPath,
  tokenPath,
}) {
  validateApp(app);
  if (!services || typeof services !== "object") {
    throw new TypeError("services must be an object");
  }
  const userData = app.getPath("userData");
  const resolvedSocket =
    socketPath ?? join(userData, "worklouderctl-bridge-v1.sock");
  const resolvedToken =
    tokenPath ?? join(userData, "worklouderctl-bridge-v1.token");
  const hostSettingsAuthority =
    services.hostSettingsAuthority ??
    (services.applicationService
      ? applicationServiceHostSettingsAuthority(
          services.applicationService,
          services.analyticsService,
        )
      : undefined);
  const appsenseRuntimeAuthority =
    services.appsenseRuntimeAuthority ??
    (services.nativeService && services.focusAppService
      ? inputFocusServiceRuntimeAuthority(
          services.nativeService,
          services.focusAppService,
        )
      : undefined);
  const permissionsAuthority =
    services.permissionsAuthority ??
    (services.applicationService
      ? inputPermissionAuthority(services.applicationService)
      : undefined);
  const firmwareAuthority =
    services.firmwareAuthority ??
    (services.deviceFlashService && services.applicationService
      ? inputFirmwareAuthority(
          services.deviceFlashService,
          services.applicationService,
        )
      : undefined);
  const logsAuthority =
    services.logsAuthority ??
    (services.windowService ? inputWindowLogsAuthority(services.windowService) : undefined);
  const adapter = createInputMainAdapter({
    devicesCommManager: services.devicesCommManager,
    deviceKitVersion,
    configurationWriter: services.configurationWriter,
    hostSettingsAuthority,
    presetCatalogAuthority: services.presetCatalogAuthority,
    appsenseRuntimeAuthority,
    permissionsAuthority,
    firmwareAuthority,
    firmwareOperationsAuthority: services.firmwareOperationsAuthority,
    logsAuthority,
  });
  const bridge = await startInputCompanionBridge({
    adapter,
    inputVersion: app.getVersion(),
    bridgeVersion,
    socketPath: resolvedSocket,
    tokenPath: resolvedToken,
  });

  let stopped = false;
  const stop = async () => {
    if (stopped) {
      return;
    }
    stopped = true;
    app.removeListener("before-quit", onBeforeQuit);
    await bridge.stop();
  };
  const onBeforeQuit = () => {
    void stop();
  };
  app.once("before-quit", onBeforeQuit);

  return {
    ...bridge,
    stop,
  };
}

function inputPermissionAuthority(applicationService) {
  if (typeof applicationService.checkAppPermissions !== "function") {
    return undefined;
  }
  return {
    async readStatus({ device }) {
      const currentPlatform = platform();
      const devicePaths = device.info.portPath
        ? [String(device.info.portPath)]
        : [];
      return {
        platform: currentPlatform,
        requiredPermission:
          currentPlatform === "darwin"
            ? "input-monitoring"
            : currentPlatform === "linux"
              ? "hid-read-write"
              : "none",
        granted: Boolean(
          await applicationService.checkAppPermissions(devicePaths),
        ),
        checkedDevicePaths: currentPlatform === "linux" ? devicePaths : [],
      };
    },
  };
}

function inputFirmwareAuthority(deviceFlashService, applicationService) {
  if (
    typeof deviceFlashService.checkForFwUpdates !== "function" ||
    typeof deviceFlashService.getLatestFwRelease !== "function"
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
      let release = null;
      if (available === true) {
        const appVersion =
          typeof applicationService.appVersion === "function"
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
            : Boolean(available),
        release,
      };
    },
  };
}

function inputWindowLogsAuthority(windowService) {
  if (typeof windowService.getWindowsLogs !== "function") {
    return undefined;
  }
  return {
    async readLogs() {
      return windowService.getWindowsLogs();
    },
  };
}

function inputFocusServiceRuntimeAuthority(nativeService, focusAppService) {
  if (typeof nativeService.getWindowInFocus !== "function") {
    throw new TypeError("nativeService.getWindowInFocus must be a function");
  }
  return {
    async readState() {
      const focusedApp = await nativeService.getWindowInFocus();
      const devices = Array.isArray(focusAppService.focusAppDevices)
        ? focusAppService.focusAppDevices
        : [];
      return {
        collecting: Boolean(focusAppService.getFocusApp),
        deviceIds: devices.map((device) => String(device.id)),
        focusedApp: focusedApp ?? null,
        lastForwardedApp: focusAppService.lastAppInFocus ?? null,
      };
    },
  };
}

function applicationServiceHostSettingsAuthority(
  applicationService,
  analyticsService,
) {
  if (
    typeof applicationService.getAppSettings !== "function" ||
    typeof applicationService.saveAppSettings !== "function"
  ) {
    throw new TypeError(
      "applicationService must provide getAppSettings and saveAppSettings",
    );
  }
  const readModel = () => {
    const model = applicationService.getAppSettings();
    if (!model || typeof model !== "object") {
      throw new Error("Input application settings were unavailable");
    }
    return model;
  };
  const toSettings = (model) => {
    const dto = typeof model.toDTO === "function" ? model.toDTO() : model;
    return {
      showedAnalyticsPopUp: dto.showedAnalyticsPopUp,
      analyticsConsented: dto.analyticsConsented,
      smartActionCmdEnabled: dto.smartActionCmdEnabled,
    };
  };
  return {
    async readSettings() {
      return toSettings(readModel());
    },
    async replaceSettings(settings) {
      const model = readModel();
      const previous = toSettings(model);
      Object.assign(model, settings);
      try {
        await applicationService.saveAppSettings(model);
        if (typeof analyticsService?.checkUserConsented === "function") {
          await analyticsService.checkUserConsented();
        }
      } catch (error) {
        Object.assign(model, previous);
        throw error;
      }
    },
  };
}

function validateApp(app) {
  if (
    !app ||
    typeof app.getPath !== "function" ||
    typeof app.getVersion !== "function" ||
    typeof app.once !== "function" ||
    typeof app.removeListener !== "function"
  ) {
    throw new TypeError(
      "app must provide getPath, getVersion, once, and removeListener",
    );
  }
}

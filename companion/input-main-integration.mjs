import { join } from "node:path";
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
  const adapter = createInputMainAdapter({
    devicesCommManager: services.devicesCommManager,
    deviceKitVersion,
    configurationWriter: services.configurationWriter,
    hostSettingsAuthority,
    presetCatalogAuthority: services.presetCatalogAuthority,
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

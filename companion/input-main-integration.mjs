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
  const adapter = createInputMainAdapter({
    devicesCommManager: services.devicesCommManager,
    deviceKitVersion,
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

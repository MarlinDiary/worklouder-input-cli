import { join } from "node:path";
import { createCodexMainAdapter } from "./codex-main-adapter.mjs";
import { startCodexCompanionBridge } from "./codex-main-bridge.mjs";

export async function installCodexCompanionBridge({
  app,
  request,
  settingsReplacer,
  bridgeVersion,
  socketPath,
  tokenPath,
}) {
  if (!app || typeof app.getPath !== "function" || typeof app.getVersion !== "function") {
    throw new TypeError("Electron app.getPath/getVersion are required");
  }
  if (typeof app.once !== "function" || typeof app.removeListener !== "function") {
    throw new TypeError("Electron app lifecycle methods are required");
  }
  const userData = app.getPath("userData");
  const adapter = createCodexMainAdapter({ request, settingsReplacer });
  const bridge = await startCodexCompanionBridge({
    adapter,
    codexVersion: app.getVersion(),
    bridgeVersion,
    socketPath: socketPath ?? join(userData, "worklouderctl-codex-bridge-v1.sock"),
    tokenPath: tokenPath ?? join(userData, "worklouderctl-codex-bridge-v1.token"),
  });
  const stop = () => void bridge.stop();
  app.once("before-quit", stop);
  return {
    ...bridge,
    async stop() {
      app.removeListener("before-quit", stop);
      await bridge.stop();
    },
  };
}

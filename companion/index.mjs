export {
  BRIDGE_PROTOCOL_VERSION,
  BridgeError,
  startInputCompanionBridge,
} from "./input-main-bridge.mjs";
export {
  CONFIG_REVISION_ALGORITHM,
  CONFIG_SNAPSHOT_KIND,
  CONFIG_SNAPSHOT_SCHEMA_VERSION,
  createInputMainAdapter,
} from "./input-main-adapter.mjs";
export { installInputCompanionBridge } from "./input-main-integration.mjs";
export { inspectInputCompanionBridge } from "./conformance.mjs";

import assert from "node:assert/strict";
import test from "node:test";
import {
  codexOwnsDevice,
  currentOwnerResult,
  inputOwnsDevice,
} from "./provider-state.mjs";

function fixture() {
  return {
    input: {
      action: "status",
      available: false,
      state: {
        processRunning: false,
        discoveryStarted: false,
        polling: false,
        startSuppressed: false,
        connectedCount: 0,
      },
    },
    codex: {
      action: "status",
      state: {
        lifecycleState: "started",
        deviceState: { status: "connected" },
        startSuppressed: false,
        hasComm: true,
        hasApi: true,
        hasHidSubscription: true,
        hasJoystickSubscription: true,
      },
    },
  };
}

test("detects an exclusive Codex owner", () => {
  const state = fixture();
  assert.equal(codexOwnsDevice(state), true);
  assert.equal(inputOwnsDevice(state), false);
});

test("rejects partial Codex acquisition", () => {
  for (const field of [
    "hasComm",
    "hasApi",
    "hasHidSubscription",
    "hasJoystickSubscription",
  ]) {
    const state = fixture();
    state.codex.state[field] = false;
    assert.equal(codexOwnsDevice(state), false, field);
  }
});

test("detects an exclusive Input owner", () => {
  const state = fixture();
  Object.assign(state.input, { available: true });
  Object.assign(state.input.state, {
    processRunning: true,
    discoveryStarted: true,
    polling: true,
    startSuppressed: false,
    connectedCount: 1,
  });
  Object.assign(state.codex.state, {
    lifecycleState: "stopped",
    deviceState: { status: "disconnected" },
    startSuppressed: true,
    hasComm: false,
    hasApi: false,
    hasHidSubscription: false,
    hasJoystickSubscription: false,
  });
  assert.equal(inputOwnsDevice(state), true);
  assert.equal(codexOwnsDevice(state), false);
});

test("does not call a contested or ownerless state exclusive", () => {
  const contested = fixture();
  Object.assign(contested.input.state, {
    processRunning: true,
    discoveryStarted: true,
    connectedCount: 1,
  });
  assert.equal(inputOwnsDevice(contested), false);
  assert.equal(codexOwnsDevice(contested), false);

  const ownerless = fixture();
  Object.assign(ownerless.codex.state, {
    lifecycleState: "stopped",
    deviceState: { status: "disconnected" },
    startSuppressed: true,
    hasComm: false,
    hasApi: false,
    hasHidSubscription: false,
    hasJoystickSubscription: false,
  });
  assert.equal(inputOwnsDevice(ownerless), false);
  assert.equal(codexOwnsDevice(ownerless), false);
});

test("builds an idempotent no-op receipt", () => {
  const before = fixture();
  const result = currentOwnerResult("codex", before);
  assert.equal(result.provider, "codex");
  assert.equal(result.idempotent, true);
  assert.equal(result.released, null);
  assert.equal(result.acquired.action, "acquire");
  assert.equal(result.acquired.idempotent, true);
  assert.deepEqual(result.before, before);
});

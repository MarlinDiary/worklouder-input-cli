export function inputOwnsDevice(state) {
  const input = state.input.state;
  const codex = state.codex.state;
  return (
    input.discoveryStarted === true &&
    input.startSuppressed === false &&
    input.connectedCount > 0 &&
    codex.lifecycleState === "stopped" &&
    codex.startSuppressed === true &&
    codex.hasComm === false &&
    codex.hasApi === false
  );
}

export function codexOwnsDevice(state) {
  const input = state.input.state;
  const codex = state.codex.state;
  return (
    input.discoveryStarted === false &&
    input.connectedCount === 0 &&
    codex.lifecycleState === "started" &&
    codex.startSuppressed === false &&
    codex.deviceState.status === "connected" &&
    codex.hasComm === true &&
    codex.hasApi === true &&
    codex.hasHidSubscription === true &&
    codex.hasJoystickSubscription === true
  );
}

export function currentOwnerResult(provider, before) {
  const source = provider === "input" ? before.input : before.codex;
  return {
    action: "handoff",
    provider,
    idempotent: true,
    before,
    released: null,
    acquired: { ...source, action: "acquire", idempotent: true },
  };
}

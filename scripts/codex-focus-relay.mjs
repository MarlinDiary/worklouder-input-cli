#!/usr/bin/env node

import { execFile, spawn } from "node:child_process";
import { access, chmod, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFilePromise = promisify(execFile);
const LABEL = "dev.worklouderctl.appsense-relay";
const DOMAIN = `gui/${process.getuid()}`;
const HOME = process.env.HOME;
if (HOME == null) throw new Error("HOME is required");
const SUPPORT = join(HOME, "Library/Application Support/worklouderctl");
const PLIST = join(HOME, "Library/LaunchAgents", `${LABEL}.plist`);
const LOG = join(SUPPORT, "appsense-relay.log");
const ERROR_LOG = join(SUPPORT, "appsense-relay.error.log");
const STATE = join(SUPPORT, "appsense-relay-state.json");
const RPC = fileURLToPath(new URL("./codex-device-rpc.mjs", import.meta.url));
const [mode = "status"] = process.argv.slice(2);

if (process.argv.includes("--help")) {
  console.log("usage: codex-focus-relay.mjs <status|install|remove|once|run>");
  process.exit(0);
}
if (!["status", "install", "remove", "once", "run"].includes(mode)) {
  throw new Error("relay mode must be status, install, remove, once, or run");
}

if (mode === "run") {
  await runRelay();
} else if (mode === "once") {
  console.log(JSON.stringify({ action: "once", relay: await forwardFrontmost() }));
} else if (mode === "install") {
  console.log(JSON.stringify(await installRelay()));
} else if (mode === "remove") {
  console.log(JSON.stringify(await removeRelay()));
} else {
  console.log(JSON.stringify(await relayStatus()));
}

async function runRelay() {
  let currentIdentity = null;
  let pending = Promise.resolve();
  let timer = null;
  const scheduleForward = () => {
    if (timer != null) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      pending = pending
        .then(async () => {
          const app = await frontmostApp();
          const identity = JSON.stringify(app);
          if (identity === currentIdentity) return;
          const result = await forwardApp(app);
          currentIdentity = identity;
          await writeRelayState({ at: new Date().toISOString(), app, result });
        })
        .catch(async (error) => {
          await writeRelayState({
            at: new Date().toISOString(),
            error: errorMessage(error),
          }).catch(() => null);
        });
    }, 75);
  };

  scheduleForward();
  const listener = spawn("/usr/bin/lsappinfo", [
    "listen",
    "+becameFrontmost",
    "forever",
  ], { stdio: ["ignore", "pipe", "pipe"] });
  listener.stdout.on("data", scheduleForward);
  listener.stderr.on("data", (data) => process.stderr.write(data));
  listener.on("error", (error) => {
    process.stderr.write(`${JSON.stringify({ error: errorMessage(error) })}\n`);
    process.exitCode = 1;
  });
  const stop = () => listener.kill("SIGTERM");
  process.once("SIGTERM", stop);
  process.once("SIGINT", stop);
  const code = await new Promise((resolve) => listener.once("exit", resolve));
  if (timer != null) clearTimeout(timer);
  await pending;
  if (code !== 0 && process.exitCode == null) process.exitCode = code ?? 1;
}

async function installRelay() {
  await mkdir(dirname(PLIST), { recursive: true, mode: 0o700 });
  await mkdir(SUPPORT, { recursive: true, mode: 0o700 });
  const plist = launchAgentPlist();
  await writeFile(PLIST, plist, { mode: 0o600 });
  await chmod(PLIST, 0o600);
  const readback = await readFile(PLIST, "utf8");
  if (readback !== plist) throw new Error("relay LaunchAgent readback differed");
  await execFilePromise("/bin/launchctl", ["bootout", DOMAIN, PLIST]).catch(
    () => null,
  );
  await execFilePromise("/bin/launchctl", ["bootstrap", DOMAIN, PLIST]);
  await execFilePromise("/bin/launchctl", ["kickstart", "-k", `${DOMAIN}/${LABEL}`]);
  const status = await waitForRunning(true);
  return { action: "install", relay: status };
}

async function removeRelay() {
  const before = await relayStatus();
  await execFilePromise("/bin/launchctl", ["bootout", DOMAIN, PLIST]).catch(
    () => null,
  );
  await rm(PLIST, { force: true });
  const after = await waitForRunning(false);
  return { action: "remove", before: before.relay, relay: after };
}

async function relayStatus() {
  let installed = true;
  try {
    await access(PLIST);
  } catch {
    installed = false;
  }
  let running = false;
  let detail = null;
  let lastEvent = null;
  try {
    lastEvent = JSON.parse(await readFile(STATE, "utf8"));
  } catch {
    lastEvent = null;
  }
  try {
    const { stdout } = await execFilePromise("/bin/launchctl", [
      "print",
      `${DOMAIN}/${LABEL}`,
    ]);
    running = /\bstate = running\b/.test(stdout);
    const pid = stdout.match(/\bpid = (\d+)\b/)?.[1];
    detail = { pid: pid == null ? null : Number(pid) };
  } catch {
    running = false;
  }
  return {
    action: "status",
    relay: {
      label: LABEL,
      installed,
      running,
      plist: PLIST,
      log: LOG,
      errorLog: ERROR_LOG,
      state: STATE,
      detail,
      lastEvent,
    },
  };
}

async function waitForRunning(expected) {
  const deadline = Date.now() + 5_000;
  let last;
  do {
    last = (await relayStatus()).relay;
    if (last.running === expected && (!expected || last.installed)) return last;
    await new Promise((resolve) => setTimeout(resolve, 100));
  } while (Date.now() < deadline);
  throw new Error(`relay state did not become running=${expected}: ${JSON.stringify(last)}`);
}

async function forwardFrontmost() {
  const app = await frontmostApp();
  return { app, result: await forwardApp(app) };
}

async function frontmostApp() {
  const { stdout: front } = await execFilePromise("/usr/bin/lsappinfo", ["front"]);
  const asn = front.trim();
  if (!/^ASN:/.test(asn)) throw new Error(`invalid frontmost ASN: ${asn}`);
  const { stdout: info } = await execFilePromise("/usr/bin/lsappinfo", [
    "info",
    "-only",
    "bundleID,name,bundlepath",
    asn,
  ]);
  const appName = info.match(/^\s*"([^"]+)"/)?.[1];
  const processId = info.match(/\bbundleID="([^"]+)"/)?.[1];
  const path = info.match(/\bbundle path="([^"]+)"/)?.[1];
  if (appName == null || processId == null || path == null) {
    throw new Error(`frontmost application identity was incomplete: ${info.trim()}`);
  }
  return { appName, process: processId, path };
}

async function forwardApp(app) {
  const { stdout } = await execFilePromise(process.execPath, [
    RPC,
    "focus",
    "--name",
    app.appName,
    "--process",
    app.process,
    "--path",
    app.path,
  ], { maxBuffer: 2 * 1024 * 1024 });
  return JSON.parse(stdout);
}

async function writeRelayState(value) {
  await writeFile(STATE, `${JSON.stringify(value)}\n`, { mode: 0o600 });
  await chmod(STATE, 0o600);
}

function launchAgentPlist() {
  const argumentsXml = [process.execPath, fileURLToPath(import.meta.url), "run"]
    .map((value) => `      <string>${xmlEscape(value)}</string>`)
    .join("\n");
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key><string>${LABEL}</string>
    <key>ProgramArguments</key>
    <array>
${argumentsXml}
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>ProcessType</key><string>Interactive</string>
    <key>LimitLoadToSessionType</key><string>Aqua</string>
    <key>StandardOutPath</key><string>${xmlEscape(LOG)}</string>
    <key>StandardErrorPath</key><string>${xmlEscape(ERROR_LOG)}</string>
  </dict>
</plist>
`;
}

function xmlEscape(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;");
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

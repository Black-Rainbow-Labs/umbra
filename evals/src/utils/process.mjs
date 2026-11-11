// Starting agent processes, enforcing the wall time limit, and killing a process tree by PID.

import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";

/**
 * Kills a process and all of its children by PID. On Windows this is `taskkill /T /F /PID`.
 * @param pid - the process id of the agent the runner started
 */
export function killProcessTree(pid) {
  if (!pid) return;
  if (process.platform === "win32") {
    spawnSync("taskkill", ["/T", "/F", "/PID", String(pid)], { stdio: "ignore", windowsHide: true });
    return;
  }
  try { process.kill(-pid, "SIGKILL"); } catch { try { process.kill(pid, "SIGKILL"); } catch { /* already gone */ } }
}

/**
 * Runs a command with stdin closed, stdout and stderr written to files, and a kill switch.
 * Resolves when the process exits. `controller.kill(reason)` kills the tree from outside.
 * @param spec - { command, args, cwd, env, stdoutPath, stderrPath, timeoutSeconds, killAfterSeconds }
 */
export function runProcess(spec) {
  const out = fs.openSync(spec.stdoutPath, "w");
  const err = fs.openSync(spec.stderrPath, "w");
  const started = Date.now();
  const child = spawn(spec.command, spec.args, { cwd: spec.cwd, env: spec.env, stdio: ["ignore", out, err], windowsHide: true, detached: process.platform !== "win32" });
  const status = { killReason: "" };
  const kill = (reason) => { if (status.killReason) return; status.killReason = reason; killProcessTree(child.pid); };
  const timers = [setTimeout(() => kill("timeout"), spec.timeoutSeconds * 1000)];
  if (spec.killAfterSeconds) timers.push(setTimeout(() => kill("planned-interrupt"), spec.killAfterSeconds * 1000));
  const done = new Promise((resolve) => {
    const finish = (code, error) => {
      if (status.finished) return;
      status.finished = true;
      timers.forEach(clearTimeout);
      fs.closeSync(out);
      fs.closeSync(err);
      resolve({ exitCode: code, killReason: status.killReason, killed: Boolean(status.killReason), timedOut: status.killReason === "timeout", wallMs: Date.now() - started, spawnError: error ? error.message : "" });
    };
    child.on("error", (error) => finish(null, error));
    child.on("exit", (code) => finish(status.killReason ? null : code));
  });
  return { pid: child.pid, kill, done };
}

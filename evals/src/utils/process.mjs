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
 * Watches the bytes a process has written (Windows `WriteTransferCount`, which counts every
 * write the process makes: files, pipes and devices), once a second while it runs. Returns a
 * function that stops the watcher and gives the last value read, or null when none was read.
 * @param pid - the process id
 */
function watchWrittenBytes(pid) {
  if (process.platform !== "win32") return () => null;
  const script = `while ($true) { $p = Get-CimInstance Win32_Process -Filter "ProcessId=${pid}"; if (-not $p) { break }; [Console]::Out.WriteLine($p.WriteTransferCount); [Console]::Out.Flush(); Start-Sleep -Milliseconds 500 }`;
  const watcher = spawn("powershell", ["-NoProfile", "-NonInteractive", "-Command", script], { stdio: ["ignore", "pipe", "ignore"], windowsHide: true });
  let last = null;
  watcher.stdout.setEncoding("utf8");
  watcher.stdout.on("data", (text) => { for (const line of text.split(/\r?\n/)) if (/^\d+$/.test(line.trim())) last = Number(line.trim()); });
  return () => { try { watcher.kill(); } catch { /* already gone */ } return last; };
}

/**
 * Runs a command with stdin closed, stdout and stderr written to files, and a kill switch.
 * Resolves when the process exits. `controller.kill(reason)` kills the tree from outside. With
 * `measureWrites`, the result has `bytesWritten`, the bytes the process wrote, read about every
 * half second while it ran.
 * @param spec - { command, args, cwd, env, stdoutPath, stderrPath, timeoutSeconds, killAfterSeconds, measureWrites }
 */
export function runProcess(spec) {
  const out = fs.openSync(spec.stdoutPath, "w");
  const err = fs.openSync(spec.stderrPath, "w");
  const started = Date.now();
  const child = spawn(spec.command, spec.args, { cwd: spec.cwd, env: spec.env, stdio: ["ignore", out, err], windowsHide: true, detached: process.platform !== "win32" });
  const stopWatching = spec.measureWrites ? watchWrittenBytes(child.pid) : () => null;
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
      const bytesWritten = stopWatching();
      resolve({ exitCode: code, killReason: status.killReason, killed: Boolean(status.killReason), timedOut: status.killReason === "timeout", wallMs: Date.now() - started, startedAtMs: started, spawnError: error ? error.message : "", ...(spec.measureWrites ? { bytesWritten } : {}) });
    };
    child.on("error", (error) => finish(null, error));
    child.on("exit", (code) => finish(status.killReason ? null : code));
  });
  return { pid: child.pid, kill, done };
}

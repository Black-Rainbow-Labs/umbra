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
 * write the process makes: files, pipes and devices). The watcher opens a handle to the process
 * as soon as it starts and keeps it open, so after the process exits it can still read the final
 * count, including what the process wrote in its last moments (pi prints its whole message list
 * as one event just before it exits, which a poll of a running process missed). Returns a
 * function that resolves to the final value, or the last value read when no final one came, or
 * null when none was read.
 * @param pid - the process id
 */
function watchWrittenBytes(pid) {
  if (process.platform !== "win32") return async () => null;
  const script = [
    "Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public struct IoCounters { public ulong R; public ulong W; public ulong O; public ulong RB; public ulong WB; public ulong OB; } public static class Io { [DllImport(\"kernel32.dll\")] public static extern bool GetProcessIoCounters(IntPtr h, out IoCounters c); }'",
    `$p = [System.Diagnostics.Process]::GetProcessById(${pid}); $h = $p.Handle`,
    "function Written { $c = New-Object IoCounters; [void][Io]::GetProcessIoCounters($h, [ref]$c); $c.WB }",
    "while (-not $p.HasExited) { [Console]::Out.WriteLine((Written)); [Console]::Out.Flush(); Start-Sleep -Milliseconds 500 }",
    "[Console]::Out.WriteLine('final ' + (Written)); [Console]::Out.Flush()",
  ].join("; ");
  const watcher = spawn("powershell", ["-NoProfile", "-NonInteractive", "-Command", script], { stdio: ["ignore", "pipe", "ignore"], windowsHide: true });
  let last = null;
  let final = null;
  const closed = new Promise((resolve) => watcher.on("close", resolve));
  watcher.stdout.setEncoding("utf8");
  watcher.stdout.on("data", (text) => {
    for (const line of text.split(/\r?\n/)) {
      const match = /^(final )?(\d+)$/.exec(line.trim());
      if (match && match[1]) final = Number(match[2]);
      else if (match) last = Number(match[2]);
    }
  });
  return async () => {
    await Promise.race([closed, new Promise((resolve) => setTimeout(resolve, 10000))]);
    try { watcher.kill(); } catch { /* already gone */ }
    return final ?? last;
  };
}

/**
 * Runs a command with stdin closed, stdout and stderr written to files, and a kill switch.
 * Resolves when the process exits. `controller.kill(reason)` kills the tree from outside. With
 * `measureWrites`, the result has `bytesWritten`, the bytes the process wrote in total, read from
 * the process handle after it exited. `killWhen`, a function polled every 250 ms, kills the process as a
 * planned interrupt the first time it returns true.
 * @param spec - { command, args, cwd, env, stdoutPath, stderrPath, timeoutSeconds, killAfterSeconds, killWhen, measureWrites }
 */
export function runProcess(spec) {
  const out = fs.openSync(spec.stdoutPath, "w");
  const err = fs.openSync(spec.stderrPath, "w");
  const started = Date.now();
  const child = spawn(spec.command, spec.args, { cwd: spec.cwd, env: spec.env, stdio: ["ignore", out, err], windowsHide: true, detached: process.platform !== "win32" });
  const stopWatching = spec.measureWrites ? watchWrittenBytes(child.pid) : async () => null;
  const status = { killReason: "" };
  const kill = (reason) => { if (status.killReason) return; status.killReason = reason; killProcessTree(child.pid); };
  const timers = [setTimeout(() => kill("timeout"), spec.timeoutSeconds * 1000)];
  if (spec.killAfterSeconds) timers.push(setTimeout(() => kill("planned-interrupt"), spec.killAfterSeconds * 1000));
  const poll = spec.killWhen ? setInterval(() => { try { if (spec.killWhen()) kill("planned-interrupt"); } catch { /* keep polling */ } }, 250) : null;
  const done = new Promise((resolve) => {
    const finish = (code, error) => {
      if (status.finished) return;
      status.finished = true;
      timers.forEach(clearTimeout);
      if (poll) clearInterval(poll);
      fs.closeSync(out);
      fs.closeSync(err);
      stopWatching().then((bytesWritten) => resolve({ exitCode: code, killReason: status.killReason, killed: Boolean(status.killReason), timedOut: status.killReason === "timeout", wallMs: Date.now() - started, startedAtMs: started, spawnError: error ? error.message : "", ...(spec.measureWrites ? { bytesWritten } : {}) }));
    };
    child.on("error", (error) => finish(null, error));
    child.on("exit", (code) => finish(status.killReason ? null : code));
  });
  return { pid: child.pid, kill, done };
}

// A machine wide lock that lets only one eval run use the model at a time. The local model server
// has one slot, so two runners at once make each cell wait on the other and every timing and cache
// number wrong. The lock file lives in the home folder, not in the run folder, so runs with a
// different EVAL_RUNS_DIR still see each other.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export const LOCK_FILE = process.env.EVAL_LOCK_FILE || path.join(os.homedir(), ".umbra-evals-runner.lock");

/**
 * Returns true when a process with this id is still running.
 * @param pid - the process id read from the lock file
 */
function processAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try { process.kill(pid, 0); return true; } catch (error) { return error.code === "EPERM"; }
}

/**
 * Reads the lock file, or returns null when it is missing or unreadable.
 * @param file - the lock file path
 */
function readLock(file) {
  try { return JSON.parse(fs.readFileSync(file, "utf8")); } catch { return null; }
}

/**
 * Takes the runner lock or throws when another live eval run holds it. A lock left by a process
 * that has exited is replaced. The lock is removed when this process exits.
 * @param runId - the run id, written into the lock so the other runner can be named
 * @param file - the lock file path (defaults to LOCK_FILE)
 */
export function acquireRunLock(runId, file = LOCK_FILE) {
  const body = JSON.stringify({ pid: process.pid, runId, argv: process.argv.slice(2), startedAt: new Date().toISOString() });
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      fs.writeFileSync(file, body, { flag: "wx" });
      const release = () => { const held = readLock(file); if (held && held.pid === process.pid) fs.rmSync(file, { force: true }); };
      process.once("exit", release);
      for (const signal of ["SIGINT", "SIGTERM", "SIGBREAK", "SIGHUP"]) process.once(signal, () => { release(); process.exit(130); });
      return release;
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
      const held = readLock(file);
      if (held && processAlive(held.pid)) throw new Error(`another eval run holds the model: pid ${held.pid}, run ${held.runId}, started ${held.startedAt} (lock ${file})`);
      fs.rmSync(file, { force: true });
    }
  }
  throw new Error(`could not take the eval runner lock ${file}`);
}

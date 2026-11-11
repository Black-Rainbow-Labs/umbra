// Chooses the cells a scorecard compares: the run's own cells, plus pi's cells from the fixed
// reference run when the run measured Umbra only, and marks cells whose agent reached the
// task definitions in evals/tasks.

import fs from "node:fs";
import path from "node:path";
import { loadRunCells, runDir } from "../repositories/runRepository.mjs";

export const DEFAULT_PI_REFERENCE = "pi-1.3.0-ref";
const TASKS_PATH = /evals[\\/]+tasks[\\/]/i;
const TRANSCRIPTS = ["stdout.log", "trajectory.json"];

/**
 * Returns the transcript files of a cell: the agent's output and Umbra's trajectory, for every
 * phase of a phased task.
 * @param dir - the cell folder
 */
function transcriptFiles(dir) {
  const folders = [dir, ...fs.readdirSync(dir).filter((name) => /^phase-\d+$/.test(name)).map((name) => path.join(dir, name))];
  return folders.flatMap((folder) => TRANSCRIPTS.map((name) => path.join(folder, name))).filter((file) => fs.existsSync(file));
}

/**
 * Returns true when a cell's transcript names a path under evals/tasks, which means the agent
 * found the verifiers, hidden tests or solutions. Such a cell fails.
 * @param dir - the cell folder
 */
export function reachedTaskDefinitions(dir) {
  return transcriptFiles(dir).some((file) => TASKS_PATH.test(fs.readFileSync(file, "utf8")));
}

/**
 * Marks a cell failed when its agent reached evals/tasks, keeping the verifier's verdict.
 * @param cell - { dir, result, requests }
 */
function applyIsolationCheck(cell) {
  const breached = cell.result.isolationBreach ?? reachedTaskDefinitions(cell.dir);
  if (!breached) return { ...cell, result: { ...cell.result, isolationBreach: false } };
  return { ...cell, result: { ...cell.result, isolationBreach: true, pass: false, detail: `[read evals/tasks] ${cell.result.detail}` } };
}

/**
 * Loads the cells to score. When the run has Umbra cells and no pi cells, pi's cells come from
 * the reference run. Only tasks that both agents ran are kept, so totals compare like with like.
 * @param runId - the run id
 * @param referenceRunId - the pi reference run id, or null for the default
 */
export function scorecardCells(runId, referenceRunId) {
  const own = loadRunCells(runId);
  const hasPi = own.some((c) => c.result.agent === "pi");
  const referenceId = referenceRunId || DEFAULT_PI_REFERENCE;
  const usesReference = !hasPi && referenceId !== runId && fs.existsSync(runDir(referenceId));
  const reference = usesReference ? loadRunCells(referenceId).filter((c) => c.result.agent === "pi") : [];
  let cells = [...own, ...reference];
  const agents = new Set(cells.map((c) => c.result.agent));
  if (agents.has("umbra") && agents.has("pi")) {
    const tasksOf = (agent) => new Set(cells.filter((c) => c.result.agent === agent).map((c) => c.result.taskId));
    const umbraTasks = tasksOf("umbra");
    const piTasks = tasksOf("pi");
    cells = cells.filter((c) => umbraTasks.has(c.result.taskId) && piTasks.has(c.result.taskId));
  }
  return { cells: cells.map(applyIsolationCheck), referenceRunId: usesReference ? referenceId : null };
}

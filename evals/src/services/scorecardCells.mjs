// Chooses the cells a scorecard compares: the run's own cells, plus pi's cells from the fixed
// reference run when the run measured Umbra only, and marks cells whose agent reached the
// task definitions in evals/tasks or named a path outside its workspace.

import fs from "node:fs";
import path from "node:path";
import { loadRunCells, runDir } from "../repositories/runRepository.mjs";
import { outsidePathsForCell } from "./outsidePaths.mjs";

export const DEFAULT_PI_REFERENCE = "pi-1.3.0-ref";
const TASKS_PATH = /evals[\\/]+tasks[\\/]/i;
const TRANSCRIPTS = ["stdout.log", "trajectory.json"];
const BLOCKED_PREFIX = "Not run.";

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
 * Returns a trajectory's text without the commands Umbra's workspace guard refused and their
 * refusal messages (task-2712, E28). A refused command never ran, so the path it names was not
 * reached.
 * @param text - the trajectory JSON text
 */
function withoutBlockedCommands(text) {
  let trajectory;
  try { trajectory = JSON.parse(text); } catch { return text; }
  const isBlocked = (m) => m.role === "tool" && (m.extra?.blocked_path || String(m.extra?.exception_info || "").startsWith(BLOCKED_PREFIX));
  const blockedIds = new Set((trajectory.messages || []).filter(isBlocked).map((m) => m.tool_call_id));
  if (!blockedIds.size) return text;
  const messages = (trajectory.messages || []).filter((m) => !blockedIds.has(m.tool_call_id) || m.role !== "tool").map((m) => (m.tool_calls ? { ...m, tool_calls: m.tool_calls.filter((c) => !blockedIds.has(c.id)) } : m));
  return JSON.stringify({ ...trajectory, messages });
}

/**
 * Returns true when a cell's transcript names a path under evals/tasks, which means the agent
 * found the verifiers, hidden tests or solutions. Such a cell fails. Commands the workspace guard
 * refused are left out.
 * @param dir - the cell folder
 */
export function reachedTaskDefinitions(dir) {
  return transcriptFiles(dir).some((file) => {
    const text = fs.readFileSync(file, "utf8");
    return TASKS_PATH.test(file.endsWith("trajectory.json") ? withoutBlockedCommands(text) : text);
  });
}

/**
 * Marks a cell failed when its agent reached evals/tasks, keeping the verifier's verdict, and
 * records the outside paths its commands named and the commands the guard refused.
 * @param cell - { dir, runId, result, requests }
 */
function applyIsolationCheck(cell) {
  const outside = outsidePathsForCell(cell.dir, cell.runId, cell.result);
  const result = { ...cell.result, outsidePaths: outside.paths, blockedCommands: outside.blocked };
  const breached = reachedTaskDefinitions(cell.dir);
  // A cell the runner failed for a path that only a refused command named passes on its verdict.
  const pass = cell.result.isolationBreach ? cell.result.verifierPass && !cell.result.windowExceededCount : cell.result.pass;
  if (!breached) return { ...cell, result: { ...result, pass, isolationBreach: false } };
  return { ...cell, result: { ...result, isolationBreach: true, pass: false, detail: `[read evals/tasks] ${cell.result.detail}` } };
}

/**
 * Loads the cells of a run with the run id on each, for the outside path check.
 * @param runId - the run id
 */
function cellsOf(runId) {
  return loadRunCells(runId).map((cell) => ({ ...cell, runId }));
}

/**
 * Loads the Umbra cells of several runs as one run (`report --pool`): the reps of the second run
 * are numbered after those of the first, so two 3 rep runs are scored as 6 reps.
 * @param runIds - the run ids
 */
function pooledUmbraCells(runIds) {
  let offset = 0;
  const cells = [];
  for (const runId of runIds) {
    const own = cellsOf(runId).filter((c) => c.result.agent === "umbra");
    const reps = Math.max(0, ...own.map((c) => c.result.rep));
    cells.push(...own.map((c) => ({ ...c, result: { ...c.result, rep: c.result.rep + offset } })));
    offset += reps;
  }
  return cells;
}

/**
 * Loads the cells to score. When the run has Umbra cells and no pi cells, pi's cells come from
 * the reference run. Only tasks that both agents ran are kept, so totals compare like with like.
 * A list of run ids pools their Umbra cells.
 * @param runId - the run id, or a list of run ids to pool
 * @param referenceRunId - the pi reference run id, or null for the default
 */
export function scorecardCells(runId, referenceRunId) {
  const own = Array.isArray(runId) ? pooledUmbraCells(runId) : cellsOf(runId);
  const hasPi = own.some((c) => c.result.agent === "pi");
  const referenceId = referenceRunId || DEFAULT_PI_REFERENCE;
  const usesReference = !hasPi && referenceId !== runId && fs.existsSync(runDir(referenceId));
  const reference = usesReference ? cellsOf(referenceId).filter((c) => c.result.agent === "pi") : [];
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

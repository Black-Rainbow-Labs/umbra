// selftest: for every task, the verifier must fail on the untouched workspace and pass on the
// reference solution. A task that does not meet both does not ship.

import fs from "node:fs";
import path from "node:path";
import { prepareWorkspace, runVerifier } from "./cellService.mjs";
import { loadAllTasks, selectTasks, taskPhases, importTaskModule } from "../repositories/taskRepository.mjs";
import { removeCreatedTree, listFiles } from "../utils/files.mjs";

/**
 * Builds the runInfo a verifier would get from a clean run, for use in selftest.
 * @param task - the EvalTask
 */
function syntheticRunInfo(task) {
  const phases = taskPhases(task).map((phase, index) => ({ index, label: phase.label || `phase-${index}`, mode: phase.mode || (index ? "resume" : "new"), exitCode: 0, killReason: index === 0 && phase.killAfterSeconds ? "planned-interrupt" : "", sessionRef: index && (phase.mode === "fork" || phase.mode === "branch") ? `selftest-${phase.mode}` : "selftest", sessionValid: true, malformedToolCalls: 0 }));
  return { task, agent: "selftest", phases, requests: [], taskDir: task.dir, selftest: true };
}

/**
 * Applies a task's reference solution to a workspace: files under solution/ are copied over
 * the workspace, and solution/solve.mjs, when present, is run instead of copied.
 * @param task - the EvalTask
 * @param workspaceDir - the prepared workspace
 */
async function applySolution(task, workspaceDir) {
  const solutionDir = path.join(task.dir, "solution");
  if (!fs.existsSync(solutionDir)) throw new Error("task has no solution/ folder");
  for (const relative of listFiles(solutionDir)) {
    if (relative === "solve.mjs") continue;
    const target = path.join(workspaceDir, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(solutionDir, relative), target);
  }
  const solver = await importTaskModule(task, "solution/solve.mjs");
  if (solver) await solver.solve(workspaceDir, { seed: task.seed ?? 2614 });
}

/**
 * Runs the two selftest checks for one task and returns { ok, untouched, solved }.
 * @param task - the EvalTask
 */
async function selftestTask(task) {
  const untouchedWs = await prepareWorkspace(task, `selftest__${task.id}__untouched`);
  const untouched = await runVerifier(task, untouchedWs.workspaceDir, syntheticRunInfo(task));
  removeCreatedTree(untouchedWs.root);
  const solvedWs = await prepareWorkspace(task, `selftest__${task.id}__solved`);
  let solved;
  try {
    await applySolution(task, solvedWs.workspaceDir);
    solved = await runVerifier(task, solvedWs.workspaceDir, syntheticRunInfo(task));
  } catch (error) {
    solved = { pass: false, score: 0, detail: `solution error: ${error.message}` };
  }
  removeCreatedTree(solvedWs.root);
  const ok = !untouched.pass && solved.pass && solved.score === 1;
  return { ok, untouched, solved };
}

/**
 * Runs selftest over the selected tasks (including tasks left out of default runs) and prints
 * one line per task. Returns true when every task passed.
 * @param filters - { taskIds, category }
 */
export async function runSelftest(filters) {
  const all = loadAllTasks();
  const tasks = filters.taskIds || filters.category ? selectTasks(all, filters) : all;
  let failures = 0;
  for (const task of tasks) {
    const started = Date.now();
    const outcome = await selftestTask(task);
    if (!outcome.ok) failures += 1;
    const seconds = ((Date.now() - started) / 1000).toFixed(1);
    console.log(`${outcome.ok ? "ok  " : "FAIL"} ${task.category}/${task.id} (${seconds}s) untouched: ${outcome.untouched.pass ? "PASS" : "fail"} ${outcome.untouched.score.toFixed(2)} | solution: ${outcome.solved.pass ? "pass" : "FAIL"} ${outcome.solved.score.toFixed(2)}`);
    if (!outcome.ok) console.log(`     untouched detail: ${outcome.untouched.detail.slice(0, 300)}\n     solution detail: ${outcome.solved.detail.slice(0, 600)}`);
  }
  console.log(`selftest: ${tasks.length - failures} of ${tasks.length} tasks ok`);
  return failures === 0;
}

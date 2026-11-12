// Runs one cell: one task, one agent, one repetition. Prepares a private workspace, starts the
// proxy, runs every phase, verifies, and writes result.json.

import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { startProxy } from "./proxy.mjs";
import { runPhase } from "./phaseService.mjs";
import { taskPhases, importTaskModule } from "../repositories/taskRepository.mjs";
import { writeCellResult } from "../repositories/runRepository.mjs";
import { copyTree, removeCreatedTree, readJsonl } from "../utils/files.mjs";
import { UPSTREAM, WORK_ROOT } from "../utils/config.mjs";
import { reachedTaskDefinitions } from "./scorecardCells.mjs";

/**
 * Creates the agent's workspace outside the repository, so the agent cannot find the
 * verifier or the solution by searching upward, and fills it from the task. The folder name is
 * a hash, so the path names neither the task nor the agent. It is not under the system temp
 * folder, because Git Bash shows that folder as /tmp, a path that pi's file tools cannot open.
 * @param task - the EvalTask
 * @param scratchName - a unique name for this cell
 */
export async function prepareWorkspace(task, scratchName) {
  const root = path.join(WORK_ROOT, `w-${createHash("sha1").update(scratchName).digest("hex").slice(0, 12)}`);
  removeCreatedTree(root);
  const workspaceDir = path.join(root, "workspace");
  copyTree(path.join(task.dir, "workspace"), workspaceDir);
  const generator = await importTaskModule(task, "generate.mjs");
  if (generator) await generator.generate(workspaceDir, { seed: task.seed ?? 2614 });
  return { root, workspaceDir };
}

/**
 * Runs a task's verifier and turns any exception into a failed result.
 * @param task - the EvalTask
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - phase results and request records for the verifier
 */
export async function runVerifier(task, workspaceDir, runInfo) {
  try {
    const verifier = await importTaskModule(task, "verify.mjs");
    const outcome = await verifier.verify(workspaceDir, runInfo);
    const score = Math.max(0, Math.min(1, Number(outcome.score ?? (outcome.pass ? 1 : 0))));
    return { pass: Boolean(outcome.pass), score, detail: String(outcome.detail ?? "") };
  } catch (error) {
    return { pass: false, score: 0, detail: `verifier error: ${error.stack || error.message}` };
  }
}

/**
 * Runs every phase of a task for one agent against one proxy.
 * @param ctx - { task, adapter, cellDir, workspaceDir, sampling, model, proxyBox }
 */
async function runAllPhases(ctx) {
  const phases = taskPhases(ctx.task);
  const results = [];
  for (let index = 0; index < phases.length; index += 1) {
    const phase = phases[index];
    const phaseDir = phases.length === 1 ? ctx.cellDir : path.join(ctx.cellDir, `phase-${index}`);
    const result = await runPhase({ ...ctx, phase, index, phaseDir, earlier: results });
    results.push(result);
    console.log(`    phase ${index} ${phase.label || ""}: exit ${result.exitCode} ${result.killReason || ""} ${(result.wallMs / 1000).toFixed(1)}s session ${result.sessionRef || "-"}`);
  }
  return results;
}

/**
 * Runs one cell end to end and writes result.json.
 * @param ctx - { task, adapter, runId, rep, cellDir, sampling, model }
 */
export async function runCell(ctx) {
  const { task, adapter, cellDir } = ctx;
  removeCreatedTree(cellDir);
  fs.mkdirSync(cellDir, { recursive: true });
  const startedAt = new Date().toISOString();
  const scratch = await prepareWorkspace(task, `${ctx.runId}__${task.id}__${adapter.name}__${ctx.rep}`);
  const requestsPath = path.join(cellDir, "requests.jsonl");
  const proxyBox = { current: null, killer: null };
  const proxy = await startProxy({ upstreamHost: UPSTREAM.host, upstreamPort: UPSTREAM.port, upstreamKind: UPSTREAM.kind, upstreamTls: UPSTREAM.tls, sampling: ctx.sampling, requestsPath, onChatRequest: (count) => proxyBox.onChat?.(count) });
  proxyBox.proxy = proxy;
  let phases;
  try {
    phases = await runAllPhases({ ...ctx, workspaceDir: scratch.workspaceDir, proxyBox });
  } finally {
    await proxy.close();
  }
  const requests = readJsonl(requestsPath);
  const verdict = await runVerifier(task, scratch.workspaceDir, { task, agent: adapter.name, phases, requests, taskDir: task.dir });
  const result = buildCellResult({ task, adapter, rep: ctx.rep, phases, requests, verdict, startedAt, isolationBreach: reachedTaskDefinitions(cellDir) });
  saveWorkspace(task, scratch, cellDir);
  writeCellResult(cellDir, result);
  return result;
}

/**
 * Removes the generated large fixtures, copies the workspace into the cell folder, and removes
 * the scratch copy the runner created.
 * @param task - the EvalTask
 * @param scratch - { root, workspaceDir }
 * @param cellDir - the cell folder
 */
function saveWorkspace(task, scratch, cellDir) {
  for (const relative of task.largeFixtures || []) removeCreatedTree(path.join(scratch.workspaceDir, relative));
  try { copyTree(scratch.workspaceDir, path.join(cellDir, "workspace")); } catch (error) { console.log(`    could not copy workspace: ${error.message}`); }
  removeCreatedTree(scratch.root);
}

/**
 * Combines the verifier verdict, phase results and request records into result.json. A task
 * with a context window fails when any request went over that window.
 * @param input - { task, adapter, rep, phases, requests, verdict, startedAt }
 */
function buildCellResult(input) {
  const { task, adapter, phases, requests, verdict } = input;
  const windowExceededCount = requests.filter((r) => r.windowExceeded).length;
  const usesWindow = Boolean(task.contextWindow || taskPhases(task).some((p) => p.contextWindow));
  const pass = verdict.pass && !(usesWindow && windowExceededCount > 0) && !input.isolationBreach;
  return {
    taskId: task.id, category: task.category, scenario: task.scenario || null, agent: adapter.name, rep: input.rep,
    pass, score: verdict.score, verifierPass: verdict.pass, detail: verdict.detail,
    windowExceededCount, contextWindow: task.contextWindow || null, isolationBreach: Boolean(input.isolationBreach),
    wallMs: phases.reduce((sum, p) => sum + p.wallMs, 0),
    timedOut: phases.some((p) => p.killReason === "timeout"), stepLimitHit: phases.some((p) => p.killReason === "step-limit" || p.agentExitStatus === "LimitsExceeded"),
    exitCodes: phases.map((p) => p.exitCode), nonzeroExit: phases.some((p) => p.killReason !== "planned-interrupt" && p.exitCode !== 0),
    malformedToolCalls: phases.reduce((sum, p) => sum + (p.malformedToolCalls || 0), 0),
    agentCompactions: phases.reduce((sum, p) => sum + (p.compactions || 0), 0),
    agentVersion: phases.find((p) => p.agentVersion)?.agentVersion || null,
    phases, startedAt: input.startedAt, finishedAt: new Date().toISOString(),
  };
}

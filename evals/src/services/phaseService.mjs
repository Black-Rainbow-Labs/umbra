// Runs one phase of a cell: sets the proxy up for the phase, launches the agent through its
// adapter, enforces the wall time and step limits, and reads the agent's session artifacts.

import fs from "node:fs";
import path from "node:path";
import { runProcess } from "../utils/process.mjs";
import { listFiles } from "../utils/files.mjs";

/**
 * Works out which session a phase resumes or forks, from the phase's mode and `from` index.
 * @param phase - the TaskPhase
 * @param index - the phase index
 * @param earlier - results of the phases already run
 */
export function resumeTarget(phase, index, earlier) {
  const mode = phase.mode || (index === 0 ? "new" : "resume");
  if (mode === "new") return null;
  const from = earlier[phase.from ?? index - 1];
  if (!from || !from.sessionRef) return { mode, sessionRef: null, missing: true };
  return { mode, sessionRef: from.sessionRef };
}

/**
 * Waits until the proxy has no request in flight, up to a few seconds, so that the records of
 * a killed agent's last request are written before the next phase starts.
 * @param proxy - the running proxy
 */
async function waitForProxyIdle(proxy) {
  for (let i = 0; i < 50 && proxy.state.inFlight > 0; i += 1) await new Promise((resolve) => setTimeout(resolve, 100));
}

/**
 * Runs one phase and returns its result, including the adapter's reading of the session.
 * @param ctx - { task, adapter, phase, index, phaseDir, earlier, workspaceDir, cellDir, sampling, model, proxyBox }
 */
export async function runPhase(ctx) {
  const { task, adapter, phase, index, phaseDir, proxyBox } = ctx;
  fs.mkdirSync(phaseDir, { recursive: true });
  const proxy = proxyBox.proxy;
  const contextWindow = phase.contextWindow ?? task.contextWindow ?? null;
  Object.assign(proxy.state, { phase: index, contextWindow, phaseChatCount: 0 });
  const resume = resumeTarget(phase, index, ctx.earlier);
  if (resume?.missing) return skippedPhase(phase, index, resume, "the phase to resume has no session id");
  const input = { workspaceDir: ctx.workspaceDir, prompt: phase.prompt, proxyBaseUrl: proxy.baseUrl, model: ctx.model, contextWindow, timeoutSeconds: task.timeoutSeconds, stepLimit: task.stepLimit, homeDir: path.join(ctx.cellDir, "home"), phaseDir, resume };
  const launch = adapter.buildLaunch(input);
  const handle = runProcess({ command: launch.command, args: launch.args, cwd: launch.cwd, env: launch.env, stdoutPath: path.join(phaseDir, "stdout.log"), stderrPath: path.join(phaseDir, "stderr.log"), timeoutSeconds: task.timeoutSeconds, killAfterSeconds: phase.killAfterSeconds });
  proxyBox.onChat = (count) => { if (count > task.stepLimit) handle.kill("step-limit"); };
  const outcome = await handle.done;
  proxyBox.onChat = null;
  await waitForProxyIdle(proxy);
  const artifacts = safeReadArtifacts(adapter, launch);
  const workspaceFiles = Array.isArray(task.phases) ? listFiles(ctx.workspaceDir) : undefined;
  return { index, label: phase.label || `phase-${index}`, mode: resume?.mode || "new", resumedFrom: resume?.sessionRef || null, contextWindow, ...outcome, stdoutPath: path.join(phaseDir, "stdout.log"), ...artifacts, requests: proxy.state.phaseChatCount, workspaceFiles };
}

/**
 * Reads an adapter's session artifacts, turning a read failure into an invalid session.
 * @param adapter - the agent adapter
 * @param launch - what the adapter's buildLaunch returned
 */
function safeReadArtifacts(adapter, launch) {
  try { return adapter.readArtifacts(launch); } catch (error) {
    return { sessionRef: null, sessionValid: false, sessionDetail: `could not read artifacts: ${error.message}`, malformedToolCalls: 0, agentExitStatus: null, toolCalls: 0, compactions: 0 };
  }
}

/**
 * Builds the result of a phase that could not run because its session was missing.
 * @param phase - the TaskPhase
 * @param index - the phase index
 * @param resume - the resume target
 * @param reason - why it was skipped
 */
function skippedPhase(phase, index, resume, reason) {
  return { index, label: phase.label || `phase-${index}`, mode: resume.mode, resumedFrom: null, exitCode: null, killReason: "skipped", killed: false, timedOut: false, wallMs: 0, skipped: reason, sessionRef: null, sessionValid: false, malformedToolCalls: 0, compactions: 0, requests: 0 };
}

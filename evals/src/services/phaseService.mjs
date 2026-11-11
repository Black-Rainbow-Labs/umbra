// Runs one phase of a cell: sets the proxy up for the phase, launches the agent through its
// adapter, enforces the wall time and step limits, and reads the agent's session artifacts.

import fs from "node:fs";
import path from "node:path";
import { runProcess } from "../utils/process.mjs";
import { listFiles } from "../utils/files.mjs";
import { taskPhases } from "../repositories/taskRepository.mjs";

/**
 * Works out which session a phase resumes, forks or branches, from the phase's mode and `from`
 * index. A branch continues the session from just before the user message that phase `at` sent,
 * so the branch's history has none of that phase or anything after it.
 * @param phase - the TaskPhase
 * @param index - the phase index
 * @param earlier - results of the phases already run
 * @param phases - every phase of the task
 */
export function resumeTarget(phase, index, earlier, phases = []) {
  const mode = phase.mode || (index === 0 ? "new" : "resume");
  if (mode === "new") return null;
  const from = earlier[phase.from ?? index - 1];
  if (!from || !from.sessionRef) return { mode, sessionRef: null, missing: true };
  const target = { mode, sessionRef: from.sessionRef, fromResult: from };
  if (mode === "branch") target.beforePrompt = phases[phase.at]?.prompt || "";
  return target;
}

/**
 * Cuts the last bytes off the session file a phase is about to resume, the way a crash in the
 * middle of a write would leave it.
 * @param adapter - the agent adapter
 * @param input - the AdapterInput
 * @param bytes - how many bytes to remove from the end
 */
function cutSessionFile(adapter, input, bytes) {
  const file = adapter.sessionStoreFile?.(input.homeDir, input.resume.sessionRef, input.resume.fromResult);
  if (!file || !fs.existsSync(file)) return { cutFile: file || null, cutBytes: 0 };
  const size = fs.statSync(file).size;
  fs.truncateSync(file, Math.max(0, size - bytes));
  return { cutFile: file, cutBytes: Math.min(bytes, size) };
}

/**
 * Reads the measurements a phase asked for: the time from launch to its first model request,
 * and the bytes the agent wrote apart from its own stdout and stderr, which is what it wrote to
 * its session store.
 * @param phase - the TaskPhase
 * @param outcome - what runProcess returned
 * @param proxy - the running proxy
 * @param phaseDir - the phase folder holding stdout.log and stderr.log
 */
function phaseMeasurements(phase, outcome, proxy, phaseDir) {
  const out = {};
  if (phase.measureResumeStartup) {
    out.measureResumeStartup = true;
    out.firstRequestAfterMs = proxy.state.phaseFirstChatAt ? proxy.state.phaseFirstChatAt - outcome.startedAtMs : null;
  }
  if (phase.measureStoreWrites && Number.isFinite(outcome.bytesWritten)) {
    const logBytes = ["stdout.log", "stderr.log"].reduce((total, name) => total + (fs.existsSync(path.join(phaseDir, name)) ? fs.statSync(path.join(phaseDir, name)).size : 0), 0);
    out.storeBytesWritten = Math.max(0, outcome.bytesWritten - logBytes);
  }
  return out;
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
  Object.assign(proxy.state, { phase: index, contextWindow, phaseChatCount: 0, phaseFirstChatAt: null });
  const resume = resumeTarget(phase, index, ctx.earlier, taskPhases(task));
  if (resume?.missing) return skippedPhase(phase, index, resume, "the phase to resume has no session id");
  const input = { workspaceDir: ctx.workspaceDir, prompt: phase.prompt, proxyBaseUrl: proxy.baseUrl, model: ctx.model, contextWindow, timeoutSeconds: task.timeoutSeconds, stepLimit: phase.stepLimit ?? task.stepLimit, homeDir: path.join(ctx.cellDir, "home"), phaseDir, resume };
  const cut = phase.cutSessionBytes && resume ? cutSessionFile(adapter, input, phase.cutSessionBytes) : {};
  const launch = adapter.buildLaunch(input);
  const handle = runProcess({ command: launch.command, args: launch.args, cwd: launch.cwd, env: launch.env, stdoutPath: path.join(phaseDir, "stdout.log"), stderrPath: path.join(phaseDir, "stderr.log"), timeoutSeconds: task.timeoutSeconds, killAfterSeconds: phase.killAfterSeconds, measureWrites: Boolean(phase.measureStoreWrites) });
  proxyBox.onChat = (count) => { if (count > input.stepLimit) handle.kill("step-limit"); };
  const outcome = await handle.done;
  proxyBox.onChat = null;
  await waitForProxyIdle(proxy);
  const artifacts = safeReadArtifacts(adapter, launch);
  const workspaceFiles = Array.isArray(task.phases) ? listFiles(ctx.workspaceDir) : undefined;
  const measured = phaseMeasurements(phase, outcome, proxy, phaseDir);
  return { index, label: phase.label || `phase-${index}`, mode: resume?.mode || "new", resumedFrom: resume?.sessionRef || null, contextWindow, ...outcome, ...cut, ...measured, stdoutPath: path.join(phaseDir, "stdout.log"), ...artifacts, requests: proxy.state.phaseChatCount, workspaceFiles };
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

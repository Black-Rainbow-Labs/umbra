// Runs a whole eval: every selected task, every repetition, every agent, one cell at a time.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { runCell } from "./cellService.mjs";
import { waitForIdleServer } from "./serverCheck.mjs";
import { loadAllTasks, selectTasks } from "../repositories/taskRepository.mjs";
import { cellDir, cellFinished, writeRunMeta, runDir } from "../repositories/runRepository.mjs";
import { readJson } from "../utils/files.mjs";
import { DEFAULT_MODEL, DEFAULT_SAMPLING } from "../utils/config.mjs";
import * as umbraAdapter from "../../agents/umbra.mjs";
import * as piAdapter from "../../agents/pi.mjs";

const ADAPTERS = { umbra: umbraAdapter, pi: piAdapter };

/**
 * Returns the adapter modules for the agent names given on the command line.
 * @param names - agent names such as ["umbra", "pi"]
 */
export function adaptersFor(names) {
  return names.map((agentName) => {
    if (!ADAPTERS[agentName]) throw new Error(`unknown agent ${agentName}; known: ${Object.keys(ADAPTERS).join(", ")}`);
    return ADAPTERS[agentName];
  });
}

/**
 * Reads the installed versions of both agents for run.json.
 */
function agentVersions() {
  const read = (command, args) => (spawnSync(command, args, { encoding: "utf8", windowsHide: true }).stdout || "").trim();
  return { umbra: read(umbraAdapter.UMBRA_BIN, ["--version"]), pi: read(process.execPath, [process.env.PI_CLI_JS || "C:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js", "--version"]), node: process.version };
}

/**
 * Describes the Umbra build under test: the binary, the git commit of the checkout these evals
 * live in with a flag when its Rust sources have uncommitted changes, and the arm settings.
 */
function umbraBuild() {
  const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
  const git = (args) => (spawnSync("git", ["-C", repo, ...args], { encoding: "utf8", windowsHide: true }).stdout || "").trim();
  return { bin: umbraAdapter.UMBRA_BIN, commit: git(["rev-parse", "--short", "HEAD"]), dirty: git(["status", "--porcelain", "--", "src", "Cargo.toml", "Cargo.lock"]) !== "", settings: umbraAdapter.armSettings() };
}

/**
 * Refuses to resume a run when an agent's installed version differs from the version the run
 * started with, so one run never mixes two builds of an agent.
 * @param runId - the run id
 * @param versions - the versions installed now
 */
function checkVersionsUnchanged(runId, versions) {
  const previous = readJson(path.join(runDir(runId), "run.json"), {}).versions;
  if (!previous) return;
  const changed = ["umbra", "pi"].filter((agent) => previous[agent] && previous[agent] !== versions[agent]);
  if (changed.length) throw new Error(`run ${runId} started with ${changed.map((a) => `${a} "${previous[a]}"`).join(", ")} but now finds ${changed.map((a) => `"${versions[a]}"`).join(", ")}; use a new --run-id or set UMBRA_BIN / PI_CLI_JS to the original build`);
}

/**
 * Runs one cell, unless it already has a result, and reports it on the console. A runner
 * error is written to error.txt and the cell is left without result.json, so a resumed run
 * tries it again.
 * Before each cell it checks that the agent's installed version is the one the run started with.
 * @param ctx - { task, adapter, runId, rep, sampling, model, versions }
 */
async function runOrSkipCell(ctx) {
  const dir = cellDir(ctx.runId, ctx.task.id, ctx.adapter.name, ctx.rep);
  const label = `${ctx.task.id} / ${ctx.adapter.name} / rep ${ctx.rep}`;
  if (cellFinished(dir)) { console.log(`skip ${label} (finished)`); return null; }
  const now = agentVersions()[ctx.adapter.name];
  if (now !== ctx.versions[ctx.adapter.name]) throw new Error(`${ctx.adapter.name} changed from "${ctx.versions[ctx.adapter.name]}" to "${now}" during the run; stopping so the run does not mix builds`);
  await waitForIdleServer(ctx.model);
  console.log(`run  ${label} [${new Date().toISOString()}]`);
  try {
    const result = await runCell({ ...ctx, cellDir: dir });
    console.log(`  => ${result.pass ? "PASS" : "FAIL"} score ${result.score.toFixed(2)} ${(result.wallMs / 1000).toFixed(0)}s ${result.detail.slice(0, 160).replace(/\s+/g, " ")}`);
    return result;
  } catch (error) {
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, "error.txt"), String(error.stack || error));
    console.log(`  => RUNNER ERROR ${error.message}`);
    return null;
  }
}

/**
 * Runs the eval. Cells run strictly one after another, agents interleaved per task.
 * @param options - { agents: string[], reps: number, taskIds: string[]|null, category: string|null, runId: string }
 */
export async function runEval(options) {
  const adapters = adaptersFor(options.agents);
  const tasks = selectTasks(loadAllTasks(), { taskIds: options.taskIds, category: options.category });
  const model = DEFAULT_MODEL;
  const sampling = DEFAULT_SAMPLING;
  const versions = agentVersions();
  checkVersionsUnchanged(options.runId, versions);
  writeRunMeta(options.runId, { runId: options.runId, agents: options.agents, reps: options.reps, tasks: tasks.map((t) => t.id), model, sampling, versions, ...(options.agents.includes("umbra") ? { umbraBuild: umbraBuild() } : {}), lastStartedAt: new Date().toISOString() });
  console.log(`run ${options.runId}: ${tasks.length} tasks x ${adapters.length} agents x ${options.reps} reps`);
  const ping = await waitForIdleServer(model);
  console.log(`server idle (tiny request ${ping.ms} ms)`);
  for (const task of tasks) {
    for (let rep = 1; rep <= options.reps; rep += 1) {
      for (const adapter of adapters) await runOrSkipCell({ task, adapter, runId: options.runId, rep, sampling, model, versions });
    }
  }
  writeRunMeta(options.runId, { lastFinishedAt: new Date().toISOString() });
}

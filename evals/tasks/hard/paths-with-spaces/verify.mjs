// Verifier: builds the site from pristine inputs with the agent's build script and compares every output file.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { inputFiles, toolFiles, brokenBuildScript, OUTPUT_DIR_NAME } from "./fixture.mjs";

/**
 * Writes files (project relative path to content) under a folder.
 * @param root - the folder
 * @param files - map of relative path with forward slashes to content
 */
function writeFiles(root, files) {
  for (const [relative, content] of Object.entries(files)) {
    const target = path.join(root, ...relative.split("/"));
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
  }
}

/**
 * Lists every file under a folder as relative paths with forward slashes, sorted.
 * @param root - the folder
 * @param prefix - the relative prefix used in recursion
 */
function listTree(root, prefix = "") {
  if (!fs.existsSync(root)) return [];
  const found = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) found.push(...listTree(path.join(root, entry.name), relative));
    else found.push(relative);
  }
  return found.sort();
}

/**
 * Reads every file under a folder into a map of relative path to bytes.
 * @param root - the folder
 */
function readTree(root) {
  return new Map(listTree(root).map((relative) => [relative, fs.readFileSync(path.join(root, ...relative.split("/")))]));
}

/**
 * Copies the agent's own files (build script, tools, anything but the inputs and old output) over a fresh project.
 * @param workspaceDir - the agent's workspace
 * @param projectDir - the fresh project folder
 */
function overlayAgentFiles(workspaceDir, projectDir) {
  for (const relative of listTree(workspaceDir)) {
    if (relative.startsWith("my docs/") || relative.startsWith(`${OUTPUT_DIR_NAME}/`) || relative.startsWith("node_modules/") || relative.startsWith(".git/")) continue;
    const target = path.join(projectDir, ...relative.split("/"));
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(workspaceDir, ...relative.split("/")), target);
  }
}

/**
 * Runs node build.mjs in a project folder and returns { code, output }.
 * @param projectDir - the project folder
 */
function runBuild(projectDir) {
  const run = spawnSync(process.execPath, ["build.mjs"], { cwd: projectDir, encoding: "utf8", timeout: 60000 });
  const output = `${run.stderr || ""}${run.stdout || ""}${run.error ? run.error.message : ""}`.trim().split("\n").slice(-2).join(" | ");
  return { code: run.status, output: output.slice(0, 200) };
}

/**
 * Compares the output tree of a build with the expected one and returns a problem text per file path.
 * @param actual - map of relative path to bytes from the agent's build
 * @param expected - map of relative path to bytes from the reference build
 */
function compareTrees(actual, expected) {
  const problems = new Map();
  for (const [relative, bytes] of expected) {
    if (!actual.has(relative)) problems.set(relative, "missing");
    else if (!actual.get(relative).equals(bytes)) problems.set(relative, "content differs");
  }
  const extra = [...actual.keys()].filter((relative) => !expected.has(relative));
  return { problems, extra };
}

/**
 * Builds the checks list for the agent's workspace.
 * @param workspaceDir - the agent's workspace
 * @param taskDir - the task folder
 * @param scratch - a scratch folder
 */
function runChecks(workspaceDir, taskDir, scratch) {
  const checks = [];
  const add = (name, why) => checks.push({ name, ok: !why, why: why || "" });
  const pristine = { ...inputFiles(), ...toolFiles() };
  const expectedDir = path.join(scratch, "expected", "site");
  writeFiles(expectedDir, { ...pristine, "build.mjs": brokenBuildScript() });
  fs.copyFileSync(path.join(taskDir, "solution", "build.mjs"), path.join(expectedDir, "build.mjs"));
  const reference = runBuild(expectedDir);
  if (reference.code !== 0) throw new Error(`reference build failed: ${reference.output}`);
  const expected = readTree(path.join(expectedDir, OUTPUT_DIR_NAME));

  const agentDir = path.join(scratch, "agent", "site");
  writeFiles(agentDir, pristine);
  overlayAgentFiles(workspaceDir, agentDir);
  const first = runBuild(agentDir);
  add("node build.mjs exits 0", first.code === 0 ? "" : first.output);
  const actual = readTree(path.join(agentDir, OUTPUT_DIR_NAME));
  const compared = compareTrees(actual, expected);
  for (const relative of expected.keys()) add(`output ${relative}`, compared.problems.get(relative));
  add("no other files in build output", compared.extra.length ? `unexpected: ${compared.extra.slice(0, 3).join(", ")}` : "");

  fs.writeFileSync(path.join(agentDir, OUTPUT_DIR_NAME, "stale file.txt"), "old\n");
  const second = runBuild(agentDir);
  const again = compareTrees(readTree(path.join(agentDir, OUTPUT_DIR_NAME)), expected);
  add("second run works, removes stale files, same output", second.code !== 0 ? second.output : again.problems.size || again.extra.length ? `differs: ${[...again.problems.keys(), ...again.extra].slice(0, 2).join(", ")}` : "");

  const movedDir = path.join(scratch, "my project", "site copy");
  writeFiles(movedDir, pristine);
  overlayAgentFiles(workspaceDir, movedDir);
  const moved = runBuild(movedDir);
  const movedCompared = compareTrees(readTree(path.join(movedDir, OUTPUT_DIR_NAME)), expected);
  add("works from a folder with spaces in its path", moved.code !== 0 ? moved.output : movedCompared.problems.size || movedCompared.extra.length ? "output differs" : "");

  const inputs = readTree(path.join(workspaceDir, "my docs"));
  const wanted = Object.keys(pristine).filter((relative) => relative.startsWith("my docs/"));
  const inputProblem = wanted.find((relative) => !inputs.has(relative.slice("my docs/".length)) || !inputs.get(relative.slice("my docs/".length)).equals(Buffer.from(pristine[relative])));
  add("input files and template are unchanged", inputProblem ? `changed or missing: ${inputProblem}` : "");
  return checks;
}

/**
 * Verifies the build script against the reference output.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "eval-spaces-"));
  try {
    const checks = runChecks(workspaceDir, runInfo.taskDir, scratch);
    const passed = checks.filter((check) => check.ok).length;
    const failed = checks.filter((check) => !check.ok);
    const detail = failed.length ? `${passed}/${checks.length} checks; failed: ${failed.slice(0, 3).map((check) => `${check.name} (${check.why.slice(0, 100)})`).join("; ")}` : `all ${checks.length} checks passed`;
    return { pass: failed.length === 0, score: passed / checks.length, detail };
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

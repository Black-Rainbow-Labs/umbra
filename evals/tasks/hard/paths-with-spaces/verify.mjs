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
 * Runs node build.mjs --check in a project folder and returns { code, stdout, stderr }.
 * @param projectDir - the project folder
 */
function runCheckMode(projectDir) {
  const run = spawnSync(process.execPath, ["build.mjs", "--check"], { cwd: projectDir, encoding: "utf8", timeout: 60000 });
  return { code: run.status, stdout: run.stdout || "", stderr: run.stderr || "" };
}

/**
 * Lists the lines the --check mode must print: one per manifest entry, in manifest order, with a status per output file.
 * @param manifestText - the reference manifest.json text
 * @param statuses - map of output path to "changed" or "missing"; every other file is "ok"
 */
function expectedCheckLines(manifestText, statuses) {
  return JSON.parse(manifestText).map((entry) => `${statuses.get(entry.output) || "ok"} ${entry.output}`);
}

/**
 * Checks `node build.mjs --check` on a clean build and on a build where one file was changed, one deleted and one emptied.
 * @param add - records one check (name, problem text or empty)
 * @param agentDir - a project folder that was just built by the agent's script
 * @param expected - map of relative path to bytes from the reference build
 */
function addCheckModeChecks(add, agentDir, expected) {
  const manifestText = expected.get("manifest.json") ? expected.get("manifest.json").toString("utf8") : "[]";
  const clean = runCheckMode(agentDir);
  const cleanLines = clean.stdout.split(/\r?\n/).filter((line) => line !== "");
  add("--check on a clean build prints ok for every file and exits 0", clean.code === 0 && cleanLines.join("\n") === expectedCheckLines(manifestText, new Map()).join("\n") ? "" : `exit ${clean.code}, first line: ${cleanLines[0] || "(none)"}`);
  const out = (relative) => path.join(agentDir, OUTPUT_DIR_NAME, ...relative.split("/"));
  try {
    fs.appendFileSync(out("pages/dads-notes.html"), "<!-- edited -->\n");
    fs.rmSync(out("raw/notes (draft).txt"), { force: true });
    fs.writeFileSync(out("raw/empty file.bin"), "x");
    fs.writeFileSync(out("tables/tricky-multi-line.html"), "");
  } catch (error) {
    add("--check on a changed build", `could not change files: ${error.message}`);
    return;
  }
  const statuses = new Map([
    [`${OUTPUT_DIR_NAME}/pages/dads-notes.html`, "changed"],
    [`${OUTPUT_DIR_NAME}/raw/notes (draft).txt`, "missing"],
    [`${OUTPUT_DIR_NAME}/raw/empty file.bin`, "changed"],
    [`${OUTPUT_DIR_NAME}/tables/tricky-multi-line.html`, "changed"],
  ]);
  const before = readTree(path.join(agentDir, OUTPUT_DIR_NAME));
  const changed = runCheckMode(agentDir);
  const lines = changed.stdout.split(/\r?\n/).filter((line) => line !== "");
  const wanted = expectedCheckLines(manifestText, statuses);
  add("--check reports changed and missing files in manifest order", lines.join("\n") === wanted.join("\n") ? "" : `first difference: ${lines.find((line, i) => line !== wanted[i]) || "line count"}`);
  add("--check exits 1 when a file is changed or missing", changed.code === 1 ? "" : `exit ${changed.code}`);
  const after = compareTrees(readTree(path.join(agentDir, OUTPUT_DIR_NAME)), before);
  add("--check does not build, delete or change anything", after.problems.size || after.extra.length ? "build output was modified" : "");
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
  overlayAgentFiles(path.join(taskDir, "solution"), expectedDir);
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
  addCheckModeChecks(add, agentDir, expected);

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

  const bare = path.join(scratch, "bare", "site");
  writeFiles(bare, pristine);
  overlayAgentFiles(workspaceDir, bare);
  const noManifest = runCheckMode(bare);
  add("--check without a manifest exits 2 with no output on stdout", noManifest.code === 2 && noManifest.stdout.trim() === "" ? "" : `exit ${noManifest.code}, stdout: ${noManifest.stdout.slice(0, 60)}`);

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

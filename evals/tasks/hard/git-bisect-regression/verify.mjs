// Verifier for git-bisect-regression: eleven checks on FIRST_BAD.txt, the history, the new commit, the working
// tree, the visible tests and the hidden behavior tests.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

/**
 * Runs git in a folder and returns { ok, out }.
 * @param dir - the repository folder
 * @param args - git arguments
 */
function git(dir, args) {
  const run = spawnSync("git", args, { cwd: dir, encoding: "utf8", timeout: 60000 });
  return { ok: run.status === 0, out: (run.stdout || "").replace(/\r/g, "") };
}

/**
 * Runs node's test runner and reports whether every test passed.
 * @param cwd - the working folder
 * @param files - test files (none means the default discovery in cwd)
 * @param workspaceDir - passed as WORKSPACE_DIR
 */
function testsPass(cwd, files, workspaceDir) {
  const run = spawnSync("node", ["--test", "--test-reporter=tap", ...files], { cwd, env: { ...process.env, WORKSPACE_DIR: workspaceDir }, encoding: "utf8", timeout: 60000 });
  const output = `${run.stdout || ""}${run.stderr || ""}`;
  const pass = /^# pass (\d+)/m.exec(output);
  const fail = /^# fail (\d+)/m.exec(output);
  return Boolean(pass && fail && Number(fail[1]) === 0 && Number(pass[1]) > 0);
}

/**
 * Builds a fresh copy of the original repository and returns { dir, revs, badHash }.
 * @param taskDir - the task folder
 */
async function buildOriginal(taskDir) {
  const { buildRepo, BAD_SUBJECT } = await import(pathToFileURL(path.join(taskDir, "hidden", "repo.mjs")).href);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "bisect-orig-"));
  buildRepo(dir);
  const revs = git(dir, ["rev-list", "main"]).out.trim().split("\n");
  const rows = git(dir, ["log", "--format=%H%x09%s", "main"]).out.trim().split("\n").map((line) => line.split("\t"));
  return { dir, revs, badHash: rows.find(([, subject]) => subject === BAD_SUBJECT)[0], message: (await import(pathToFileURL(path.join(taskDir, "hidden", "repo.mjs")).href)).FIX_MESSAGE };
}

/**
 * Runs the checks and returns { pass, score, detail }.
 * @param workspaceDir - the repository after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  let original = null;
  try {
    if (!fs.existsSync(path.join(workspaceDir, ".git"))) return { pass: false, score: 0, detail: "the repository is missing" };
    original = await buildOriginal(runInfo.taskDir);
    const checks = runChecks(workspaceDir, original, runInfo.taskDir);
    const failed = checks.filter((c) => !c.ok).map((c) => c.label);
    return { pass: failed.length === 0, score: (checks.length - failed.length) / checks.length, detail: failed.length ? `failed checks: ${failed.join("; ")}` : `all ${checks.length} checks passed` };
  } catch (error) {
    return { pass: false, score: 0, detail: `verifier error: ${error.message}` };
  } finally {
    if (original) fs.rmSync(original.dir, { recursive: true, force: true });
  }
}

/**
 * Returns the list of { label, ok } checks.
 * @param ws - the workspace repository folder
 * @param original - { revs, badHash, message } of a fresh copy of the original repository
 * @param taskDir - the task folder
 */
function runChecks(ws, original, taskDir) {
  const file = path.join(ws, "FIRST_BAD.txt");
  const raw = fs.existsSync(file) ? fs.readFileSync(file, "utf8") : "";
  const revs = git(ws, ["rev-list", "HEAD"]).out.trim().split("\n");
  const oneNew = revs.length === original.revs.length + 1 && revs.slice(1).join(",") === original.revs.join(",");
  const changed = git(ws, ["diff", "--name-only", "HEAD~1", "HEAD"]).out.trim().split("\n");
  const branch = git(ws, ["symbolic-ref", "--short", "HEAD"]);
  const status = git(ws, ["status", "--porcelain"]);
  const bisectRefs = git(ws, ["for-each-ref", "refs/bisect"]).out.trim();
  const sameTests = changed.every((name) => !name.startsWith("test/"));
  return [
    { label: "FIRST_BAD.txt holds the hash of the first bad commit", ok: raw.trim() === original.badHash },
    { label: "FIRST_BAD.txt is only the hash and a newline", ok: /^[0-9a-f]{40}\n?$/.test(raw) },
    { label: "history is unchanged with exactly one new commit", ok: oneNew },
    { label: "commit message is exactly the stated one", ok: git(ws, ["log", "-1", "--format=%B"]).out.trim() === original.message },
    { label: "on branch main with a clean working tree", ok: branch.ok && branch.out.trim() === "main" && status.ok && status.out.trim() === "" },
    { label: "the bisect session is ended", ok: !fs.existsSync(path.join(ws, ".git", "BISECT_START")) && bisectRefs === "" },
    { label: "FIRST_BAD.txt is in the new commit", ok: oneNew && changed.includes("FIRST_BAD.txt") },
    { label: "no test file was changed", ok: oneNew && sameTests },
    { label: "the repository's own tests pass", ok: testsPass(ws, [], ws) },
    { label: "hidden tax behavior checks pass", ok: testsPass(ws, [path.join(taskDir, "hidden", "tax.test.mjs")], ws) },
    { label: "features added after the broken commit still work", ok: testsPass(ws, [path.join(taskDir, "hidden", "features.test.mjs")], ws) },
  ];
}

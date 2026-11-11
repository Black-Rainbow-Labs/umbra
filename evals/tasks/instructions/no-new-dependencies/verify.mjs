// Verifier for no-new-dependencies: package.json untouched, no installs, only relative or node: imports, parser works.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const IMPORT_PATTERNS = [/\bfrom\s*["']([^"']+)["']/g, /\bimport\s*["']([^"']+)["']/g, /\bimport\s*\(\s*["']([^"']+)["']\s*\)/g, /\brequire\s*\(\s*["']([^"']+)["']\s*\)/g];

/**
 * Lists source files (.mjs, .js, .cjs, .ts) under a folder, skipping node_modules.
 * @param folder - absolute folder to scan
 */
function listSourceFiles(folder) {
  const found = [];
  for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
    const full = path.join(folder, entry.name);
    if (entry.isDirectory() && entry.name !== "node_modules") found.push(...listSourceFiles(full));
    else if (entry.isFile() && /\.(mjs|js|cjs|ts)$/.test(entry.name)) found.push(full);
  }
  return found;
}

/**
 * Returns a description of the first import that is neither relative nor a node: builtin, or "".
 * @param workspaceDir - the workspace after the agent finished
 */
function findForbiddenImport(workspaceDir) {
  for (const file of listSourceFiles(workspaceDir)) {
    const text = fs.readFileSync(file, "utf8");
    for (const pattern of IMPORT_PATTERNS) {
      for (const match of text.matchAll(pattern)) {
        const specifier = match[1];
        if (!specifier.startsWith("./") && !specifier.startsWith("../") && !specifier.startsWith("node:")) return `${path.relative(workspaceDir, file)} imports ${specifier}`;
      }
    }
  }
  return "";
}

/**
 * Returns a description of the first constraint violation, or "" when none.
 * @param workspaceDir - the workspace after the agent finished
 * @param taskDir - the absolute task folder
 */
function findConstraintViolation(workspaceDir, taskDir) {
  const original = fs.readFileSync(path.join(taskDir, "workspace", "package.json"));
  const current = path.join(workspaceDir, "package.json");
  if (!fs.existsSync(current) || !fs.readFileSync(current).equals(original)) return "package.json was changed or removed";
  for (const name of ["node_modules", "package-lock.json", "yarn.lock", "pnpm-lock.yaml"]) {
    if (fs.existsSync(path.join(workspaceDir, name))) return `${name} exists`;
  }
  return findForbiddenImport(workspaceDir);
}

/**
 * Runs the hidden parser tests and returns { total, bad } or null when they did not finish.
 * @param workspaceDir - the workspace after the agent finished
 * @param taskDir - the absolute task folder
 */
function runHiddenTests(workspaceDir, taskDir) {
  const script = path.join(taskDir, "hidden", "test.mjs");
  const run = spawnSync(process.execPath, [script], { env: { ...process.env, WORKSPACE: workspaceDir }, encoding: "utf8", timeout: 60000 });
  try {
    return JSON.parse(String(run.stdout || "").trim().split("\n").pop());
  } catch {
    return null;
  }
}

/**
 * Passes when no constraint is violated and every hidden parser case passes.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, including the absolute task folder
 */
export async function verify(workspaceDir, runInfo) {
  const violation = findConstraintViolation(workspaceDir, runInfo.taskDir);
  const summary = runHiddenTests(workspaceDir, runInfo.taskDir);
  if (!summary) return { pass: false, score: 0, detail: violation || "hidden tests did not finish" };
  const score = summary.total ? (summary.total - summary.bad.length) / summary.total : 0;
  if (violation) return { pass: false, score: Math.min(score, 0.5), detail: `constraint broken: ${violation}` };
  const detail = summary.bad.length ? `${summary.bad.length} of ${summary.total} failed, failed: ${summary.bad.join(" | ")}` : `no new dependencies, all ${summary.total} hidden cases passed`;
  return { pass: summary.bad.length === 0, score, detail };
}

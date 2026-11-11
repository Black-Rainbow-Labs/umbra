// Verifier for the route handler task: calls every route and checks that only the invoice summary total changed.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { buildRepo } from "./fixture.mjs";
import { writeFiles, readFilesAsBuffers } from "./gen-utils.mjs";

const DUMP_SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), "hidden", "dump-routes.mjs");
const TARGET_ROUTE = "GET /api/v1/invoices/:id/summary";

/**
 * Runs the route dump against a folder and returns parsed JSON or an error text.
 * @param root - folder that contains src/app.mjs
 */
function dumpRoutes(root) {
  const run = spawnSync("node", [DUMP_SCRIPT, root], { encoding: "utf8", timeout: 60000, maxBuffer: 64 * 1024 * 1024 });
  if (run.status !== 0) return { error: (run.stderr || "node failed").trim().split("\n").pop() };
  return { value: JSON.parse(run.stdout) };
}

/**
 * Checks that only handler and route files of the invoices resource differ from the original.
 * @param original - the original build
 * @param actual - Map of path to Buffer from the workspace
 */
function checkFiles(original, actual) {
  if ([...original.files.keys()].sort().join("\n") !== [...actual.keys()].sort().join("\n")) return describeFileSetChange(original.files, actual);
  const allowed = (file) => file.startsWith("src/handlers/invoices/") && !file.includes("summary-csv") && !file.includes("summary-v2") || file === "src/routes/invoices.mjs";
  for (const [file, text] of original.files) {
    if (!actual.get(file).equals(Buffer.from(text)) && !allowed(file)) return `${file} was changed but is not part of the invoices summary endpoint`;
  }
  return "";
}

/**
 * Checks that every route except the target returns the same response as before.
 * @param before - dump of the original
 * @param after - dump of the workspace
 */
function checkOtherRoutes(before, after) {
  for (const key of Object.keys(before.routes)) {
    if (key === TARGET_ROUTE) continue;
    if (JSON.stringify(before.routes[key]) !== JSON.stringify(after.routes[key])) return `the response of ${key} changed`;
  }
  return "";
}

/**
 * Checks the invoices summary: total in dollars, other fields unchanged, unknown ids still 404, records unchanged.
 * @param before - dump of the original
 * @param after - dump of the workspace
 */
function checkInvoiceSummary(before, after) {
  for (const id of Object.keys(before.invoices)) {
    const record = before.invoices[id].record;
    const summary = after.invoices[id].summary;
    const oldSummary = before.invoices[id].summary;
    if (JSON.stringify(record) !== JSON.stringify(after.invoices[id].record)) return `GET /api/v1/invoices/${id} changed`;
    if (record.status === 404) { if (summary.status !== 404) return `summary for unknown invoice ${id} no longer returns 404`; continue; }
    if (summary.status !== 200) return `summary for invoice ${id} returned status ${summary.status}`;
    if (typeof summary.body.total !== "number" || Math.abs(summary.body.total - record.body.totalCents / 100) > 1e-9) return `summary total for invoice ${id} is ${summary.body.total}, expected ${record.body.totalCents / 100}`;
    for (const field of ["id", "currency", "itemCount"]) if (summary.body[field] !== oldSummary.body[field]) return `summary field ${field} changed for invoice ${id}`;
    if (Object.keys(summary.body).length !== Object.keys(oldSummary.body).length) return `summary for invoice ${id} has different fields`;
  }
  return "";
}

/**
 * Scores the task over three checks: files, other routes, and the invoices summary itself.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, used for the task seed
 */
export async function verify(workspaceDir, runInfo) {
  const seed = runInfo?.task?.seed ?? 2614;
  const original = buildRepo(seed);
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "route-original-"));
  const results = [];
  try {
    writeFiles(temp, original.files);
    const before = dumpRoutes(temp);
    const after = dumpRoutes(workspaceDir);
    const run = (fn) => { try { results.push(fn()); } catch (error) { results.push(`check crashed: ${error.message}`); } };
    run(() => checkFiles(original, readFilesAsBuffers(workspaceDir)));
    if (after.error) results.push(`the service does not run: ${after.error}`);
    else {
      run(() => checkOtherRoutes(before.value, after.value));
      run(() => checkInvoiceSummary(before.value, after.value));
    }
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
  const failures = results.filter(Boolean);
  const total = 3;
  return { pass: failures.length === 0 && results.length === total, score: Math.max(0, (total - failures.length) / total), detail: failures.length ? failures.join("; ") : "invoices summary returns dollars and nothing else changed" };
}

/**
 * Names the files that were added or removed compared with the original repository.
 * @param originalFiles - Map of the original paths
 * @param actual - Map of the paths in the workspace
 */
function describeFileSetChange(originalFiles, actual) {
  const added = [...actual.keys()].filter((p) => !originalFiles.has(p));
  const removed = [...originalFiles.keys()].filter((p) => !actual.has(p));
  return `the set of files changed: added ${added.slice(0, 5).join(", ") || "none"}; removed ${removed.slice(0, 5).join(", ") || "none"}`;
}

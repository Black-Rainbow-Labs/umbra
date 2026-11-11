// Verifier for inventory-64k-reconcile. Each week is scored on its own: its STOCK must equal the
// stock the agent wrote for the week before (the true stock when that week has none, the opening
// stock for week 1) plus received minus shipped, so one arithmetic slip costs one week. The
// reconcile.json values are compared with the true values. Score is the fraction of the 45 items right.

import fs from "node:fs";
import path from "node:path";
import { buildWeeks, SUPPLIER } from "./fixture.mjs";

/**
 * Reads the STOCK the agent appended to a week, or null with a reason.
 * @param workspaceDir - the workspace
 * @param week - the week from the fixture
 */
function readStock(workspaceDir, week) {
  const file = path.join(workspaceDir, week.path);
  if (!fs.existsSync(file)) return { value: null, problem: "missing" };
  const text = fs.readFileSync(file, "utf8");
  const original = week.text.replace(/\n$/, "");
  if (!text.startsWith(original)) return { value: null, problem: "original text changed" };
  const match = /^STOCK: (-?\d+)$/.exec(text.slice(original.length).trim());
  return match ? { value: Number(match[1]), problem: "" } : { value: null, problem: "no STOCK line" };
}

/**
 * Reads reconcile.json, returning an empty object when it is missing or does not parse.
 * @param workspaceDir - the workspace
 */
function readReport(workspaceDir) {
  try { return JSON.parse(fs.readFileSync(path.join(workspaceDir, "reconcile.json"), "utf8")) || {}; } catch { return {}; }
}

/**
 * Scores the weekly lines and the report.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const { opening, reorder, weeks, finalStock, weeksBelow } = buildWeeks(runInfo.task?.seed ?? 2614);
  let previous = opening;
  const problems = [];
  weeks.forEach((week, i) => {
    const { value, problem } = readStock(workspaceDir, week);
    const expected = previous + week.received - week.shipped;
    if (value === null || value !== expected) problems.push(`week ${i + 1}: ${problem || `got ${value}, expected ${expected}`}`);
    previous = value ?? week.closing;
  });
  const report = readReport(workspaceDir);
  const supplierOk = typeof report.supplier === "string" && report.supplier.trim().toLowerCase() === SUPPLIER.toLowerCase();
  const checks = { opening_stock: report.opening_stock === opening, reorder_point: report.reorder_point === reorder, supplier: supplierOk, final_stock: report.final_stock === finalStock, weeks_below_reorder: report.weeks_below_reorder === weeksBelow };
  const wrong = Object.keys(checks).filter((k) => !checks[k]);
  const correct = weeks.length - problems.length + (Object.keys(checks).length - wrong.length);
  const total = weeks.length + Object.keys(checks).length;
  const detail = `weeks ${weeks.length - problems.length}/${weeks.length}${problems.length ? ` (${problems.slice(0, 3).join("; ")})` : ""}; report wrong: ${wrong.map((k) => `${k}=${JSON.stringify(report[k])}`).join(", ") || "none"}`;
  return { pass: correct === total, score: correct / total, detail };
}

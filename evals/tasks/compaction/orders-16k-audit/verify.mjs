// Verifier for orders-16k-audit: each order must keep its text and end with its exact TOTAL
// line, and summary.json must hold the tax rate from order 1, the count, the grand total and the
// largest order. Score is the fraction of the 28 items right.

import fs from "node:fs";
import path from "node:path";
import { buildOrders } from "./fixture.mjs";

/**
 * Checks one order file and returns "" when right, or a short reason.
 * @param workspaceDir - the workspace
 * @param order - { path, text, total }
 */
function checkOrder(workspaceDir, order) {
  const file = path.join(workspaceDir, order.path);
  if (!fs.existsSync(file)) return "missing";
  const text = fs.readFileSync(file, "utf8");
  const original = order.text.replace(/\n$/, "");
  if (!text.startsWith(original)) return "original text changed";
  const added = text.slice(original.length).trim();
  if (added === `TOTAL: ${order.total}`) return "";
  return added ? `wrong line ${JSON.stringify(added.slice(0, 40))}` : "no TOTAL line";
}

/**
 * Reads summary.json, returning an empty object when it is missing or does not parse.
 * @param workspaceDir - the workspace
 */
function readSummary(workspaceDir) {
  try { return JSON.parse(fs.readFileSync(path.join(workspaceDir, "summary.json"), "utf8")) || {}; } catch { return {}; }
}

/**
 * Scores the order files and the summary.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const { rate, orders, grandTotal, largest } = buildOrders(runInfo.task?.seed ?? 2614);
  const problems = [];
  orders.forEach((order, i) => { const p = checkOrder(workspaceDir, order); if (p) problems.push(`order ${i + 1}: ${p}`); });
  const summary = readSummary(workspaceDir);
  const checks = { tax_rate: summary.tax_rate === rate, order_count: summary.order_count === orders.length, grand_total: summary.grand_total === grandTotal, largest_order: largest.includes(summary.largest_order) };
  const wrong = Object.keys(checks).filter((k) => !checks[k]);
  const correct = orders.length - problems.length + (Object.keys(checks).length - wrong.length);
  const total = orders.length + Object.keys(checks).length;
  const detail = `orders ${orders.length - problems.length}/${orders.length}${problems.length ? ` (${problems.slice(0, 3).join("; ")})` : ""}; summary wrong: ${wrong.map((k) => `${k}=${JSON.stringify(summary[k])}`).join(", ") || "none"}`;
  return { pass: correct === total, score: correct / total, detail };
}

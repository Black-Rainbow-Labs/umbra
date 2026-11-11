// Verifier for long-150-steps: each of the 70 items must keep its text and end with its exact
// CHECK line (which needs the code word from the first prompt and the limit from item 1), and
// summary.json must hold the code word, the limit, the count and the over count. Score is the
// fraction of the 74 items right, so a loss of early facts shows as wrong lines late in the run.

import fs from "node:fs";
import path from "node:path";
import { buildItems, expectedCheckLine, CODE_WORD } from "./fixture.mjs";

/**
 * Checks one item file and returns "" when right, or a short reason.
 * @param workspaceDir - the workspace
 * @param item - { path, text, weight }
 * @param limit - the batch limit
 */
function checkItem(workspaceDir, item, limit) {
  const file = path.join(workspaceDir, item.path);
  if (!fs.existsSync(file)) return "missing";
  const text = fs.readFileSync(file, "utf8");
  const original = item.text.replace(/\n$/, "");
  if (!text.startsWith(original)) return "original text changed";
  const added = text.slice(original.length).trim();
  if (added === expectedCheckLine(item.weight, limit)) return "";
  return added ? `wrong line ${JSON.stringify(added.slice(0, 40))}` : "no CHECK line";
}

/**
 * Reads summary.json, returning an empty object when it is missing or does not parse.
 * @param workspaceDir - the workspace
 */
function readSummary(workspaceDir) {
  try { return JSON.parse(fs.readFileSync(path.join(workspaceDir, "summary.json"), "utf8")) || {}; } catch { return {}; }
}

/**
 * Scores the item files and the summary.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const { limit, items, overCount } = buildItems(runInfo.task?.seed ?? 2614);
  const problems = [];
  items.forEach((item, i) => { const p = checkItem(workspaceDir, item, limit); if (p) problems.push(`item ${String(i + 1).padStart(2, "0")}: ${p}`); });
  const summary = readSummary(workspaceDir);
  const codeOk = typeof summary.code_word === "string" && summary.code_word.trim().toLowerCase() === CODE_WORD;
  const checks = { code_word: codeOk, batch_limit: summary.batch_limit === limit, item_count: summary.item_count === items.length, over_count: summary.over_count === overCount };
  const wrong = Object.keys(checks).filter((k) => !checks[k]);
  const correct = items.length - problems.length + (Object.keys(checks).length - wrong.length);
  const total = items.length + Object.keys(checks).length;
  const detail = `items ${items.length - problems.length}/${items.length}${problems.length ? ` (${problems.slice(0, 3).join("; ")})` : ""}; summary wrong: ${wrong.map((k) => `${k}=${JSON.stringify(summary[k])}`).join(", ") || "none"}`;
  return { pass: correct === total, score: correct / total, detail };
}

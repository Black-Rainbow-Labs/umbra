// Verifier: one check per customer file (the cleaned file must match the expected bytes exactly), one check that
// data/ holds exactly the 30 files and summary.json, one check for the exact bytes of summary.json and ten checks
// that each look at one kind of rule across all 30 files, so a partial result shows which rules were missed.

import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

/**
 * Reads a file as JSON and returns the value, or null when it is missing or not valid JSON.
 * @param file - the path to read
 */
function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    return null;
  }
}

/**
 * Builds the rule checks: each takes the list of { actual, expected } documents (null when unreadable)
 * and returns true when its projection is identical for every file.
 */
function ruleChecks() {
  const customers = (doc) => (doc && Array.isArray(doc.customers) ? doc.customers : null);
  const project = (fn) => (pairs) => pairs.every(({ actual, expected }) => {
    const a = customers(actual);
    const e = customers(expected);
    return a !== null && JSON.stringify(a.map(fn)) === JSON.stringify(e.map(fn));
  });
  return [
    ["top level keys, source, exportedAt and count", (pairs) => pairs.every(({ actual, expected }) => actual !== null && JSON.stringify(Object.entries(actual).filter(([k]) => k !== "customers")) === JSON.stringify(Object.entries(expected).filter(([k]) => k !== "customers")))],
    ["customer ids, merging and sort order", project((c) => c.id)],
    ["names", project((c) => c.name)],
    ["emails and countries", project((c) => [c.email, c.country])],
    ["phones", project((c) => c.phone)],
    ["signup dates", project((c) => c.signup)],
    ["tags", project((c) => c.tags)],
    ["balances", project((c) => c.balance)],
    ["last_login and the key order inside customers", project((c) => [c.last_login, Object.keys(c)])],
    ["text format of every file", (pairs, raws) => raws.every((raw) => raw !== null && !raw.startsWith("﻿") && !raw.includes("\r") && raw.endsWith("}\n") && !raw.endsWith("\n\n") && !/\\u[0-9a-fA-F]{4}/.test(raw) && raw.includes("\n  \"")) && pairs.length === raws.length],
  ];
}

/**
 * Compares every data file with the expected clean text.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including task and taskDir
 */
export async function verify(workspaceDir, runInfo) {
  try {
    const { buildDataset, buildSummary } = await import(pathToFileURL(path.join(runInfo.taskDir, "hidden", "dataset.mjs")).href);
    const files = buildDataset(runInfo.task?.seed ?? 2614);
    const dataDir = path.join(workspaceDir, "data");
    const present = fs.existsSync(dataDir) ? fs.readdirSync(dataDir).sort() : [];
    const failures = [];
    for (const file of files) {
      const full = path.join(dataDir, file.name);
      const actual = fs.existsSync(full) ? fs.readFileSync(full) : null;
      if (!actual || !actual.equals(Buffer.from(file.expected, "utf8"))) failures.push(file.name);
    }
    const wanted = [...files.map((f) => f.name), "summary.json"].sort();
    if (JSON.stringify(present) !== JSON.stringify(wanted)) failures.push("data/ does not hold exactly the 30 files and summary.json");
    const summaryFile = path.join(dataDir, "summary.json");
    const summary = fs.existsSync(summaryFile) ? fs.readFileSync(summaryFile) : null;
    if (!summary || !summary.equals(Buffer.from(buildSummary(files), "utf8"))) failures.push("summary.json");
    const pairs = files.map((f) => ({ actual: readJson(path.join(dataDir, f.name)), expected: JSON.parse(f.expected) }));
    const raws = files.map((f) => (fs.existsSync(path.join(dataDir, f.name)) ? fs.readFileSync(path.join(dataDir, f.name), "utf8") : null));
    const rules = ruleChecks();
    for (const [label, check] of rules) {
      let ok = false;
      try { ok = check(pairs, raws); } catch { ok = false; }
      if (!ok) failures.push(`rule: ${label}`);
    }
    const total = files.length + 2 + rules.length;
    const detail = failures.length === 0 ? `all ${total} checks passed` : `${failures.length} of ${total} checks failed: ${failures.slice(0, 6).join(", ")}`;
    return { pass: failures.length === 0, score: (total - failures.length) / total, detail };
  } catch (error) {
    return { pass: false, score: 0, detail: `verifier error: ${error.message}` };
  }
}

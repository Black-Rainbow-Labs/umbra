// Verifier: the TSV bytes, the Makefile structure, and the targets run with a Python stand in for make.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import {
  COLUMNS_AFTER, categoriesFile, expectedRows, expectedStats, expectedValueStats, expectedLowStock, expectedCategories, expectedLabels, expectedBundle,
  makefileLines, productRows, productsAfter, productsBefore, toolFiles,
} from "./fixture.mjs";

const SEED = 2614;

/**
 * Reads a file as a buffer, or returns an empty buffer when it is missing.
 * @param file - the file path
 */
function readOrEmpty(file) {
  return fs.existsSync(file) ? fs.readFileSync(file) : Buffer.alloc(0);
}

/**
 * Returns a problem text for the first row whose field at a column differs from the expected one, or "".
 * @param rows - the rows of the agent's file
 * @param want - the expected rows
 * @param column - the column position
 * @param name - the column name for the message
 */
function columnProblem(rows, want, column, name) {
  const bad = rows.findIndex((row, index) => row[column] !== want[index][column]);
  return bad < 0 ? "" : `row ${bad + 1}: ${name} ${JSON.stringify(rows[bad][column])}, expected ${JSON.stringify(want[bad][column])}`;
}

/**
 * Checks the TSV file and returns one problem text (or "") for each of the TSV checks.
 * @param buffer - the bytes of data/products.tsv
 */
function tsvProblems(buffer) {
  const text = buffer.toString("utf8");
  const lines = text.split("\n");
  const want = expectedRows(SEED);
  const rows = lines.slice(1, -1).map((line) => line.split("\t"));
  const problems = { header: "", label: "", price: "", value: "", status: "", tabs: "", ending: "" };
  if (lines[0] !== COLUMNS_AFTER.join("\t")) problems.header = `header is ${JSON.stringify(lines[0]).slice(0, 80)}`;
  if (rows.length !== want.length) problems.tabs = `${rows.length} data rows, expected ${want.length}`;
  const badWidth = rows.findIndex((row) => row.length !== COLUMNS_AFTER.length);
  if (!problems.tabs && badWidth >= 0) problems.tabs = `row ${badWidth + 1} has ${rows[badWidth].length} fields, expected ${COLUMNS_AFTER.length}`;
  if (!problems.tabs) {
    const moved = rows.findIndex((row, index) => [0, 1, 2, 5, 6].some((column) => row[column] !== want[index][column]));
    if (moved >= 0) problems.tabs = `row ${moved + 1}: a field other than label, price, value and status changed`;
    problems.label = columnProblem(rows, want, 3, "label");
    problems.price = columnProblem(rows, want, 4, "price");
    problems.value = columnProblem(rows, want, 7, "value");
    problems.status = columnProblem(rows, want, 8, "status");
  } else {
    problems.label = problems.price = problems.value = problems.status = "rows are not usable";
  }
  if (buffer.includes(0x0d)) problems.ending = "contains CR characters";
  else if (!text.endsWith("\n") || text.endsWith("\n\n")) problems.ending = "file must end with exactly one newline";
  else if (text !== productsAfter(SEED) && !Object.values(problems).some(Boolean)) problems.ending = "bytes differ from the expected file";
  return problems;
}

/**
 * Checks that every original Makefile line except the four that must change is still present, in order, and that it uses LF.
 * @param text - the Makefile text
 */
function preservedProblem(text) {
  if (text.includes("\r")) return "Makefile contains CR characters";
  const changeable = new Set([".PHONY: all stats check clean", "all: stats check", "COLUMNS = 6", "\t$(PYTHON) tools/tsvstat.py $(DATA) price_cents > $(OUT)/stats.txt", "\trm -rf $(OUT)"]);
  const lines = text.split("\n");
  let position = 0;
  for (const original of makefileLines()) {
    if (changeable.has(original) || original === "") continue;
    const found = lines.indexOf(original, position);
    if (found < 0) return `original line missing or changed: ${JSON.stringify(original)}`;
    position = found + 1;
  }
  return "";
}

/**
 * Creates a fresh project folder with the pristine tools, a data file and the agent's Makefile.
 * @param scratch - parent scratch folder
 * @param name - folder name
 * @param makefile - Makefile bytes
 * @param dataText - the text for data/products.tsv
 */
function makeProject(scratch, name, makefile, dataText) {
  const dir = path.join(scratch, name);
  for (const [relative, content] of Object.entries(toolFiles())) {
    fs.mkdirSync(path.dirname(path.join(dir, relative)), { recursive: true });
    fs.writeFileSync(path.join(dir, relative), content);
  }
  fs.mkdirSync(path.join(dir, "data"), { recursive: true });
  fs.writeFileSync(path.join(dir, "data", "products.tsv"), dataText);
  fs.writeFileSync(path.join(dir, "data", "old.tsv"), productsBefore(SEED));
  fs.writeFileSync(path.join(dir, "data", "categories.tsv"), categoriesFile());
  fs.writeFileSync(path.join(dir, "Makefile"), makefile);
  return dir;
}

/**
 * Runs the make stand in in a project folder.
 * @param taskDir - the task folder
 * @param dir - the project folder
 * @param args - make arguments
 */
function runMake(taskDir, dir, args) {
  const run = spawnSync("python", [path.join(taskDir, "hidden", "minimake.py"), "-C", dir, ...args], { encoding: "utf8", timeout: 60000, env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1", LC_ALL: "C" } });
  const output = `${run.stdout || ""}${run.stderr || ""}${run.error ? run.error.message : ""}`.trim().split("\n").slice(-2).join(" | ");
  return { code: run.status, output: output.slice(0, 200), stdout: run.stdout || "" };
}

/**
 * Returns "" when a file in the project has exactly the expected text, else a short reason.
 * @param dir - the project folder
 * @param relative - the file path inside the folder
 * @param expected - the expected text
 */
function fileProblem(dir, relative, expected) {
  const file = path.join(dir, relative);
  if (!fs.existsSync(file)) return `${relative} was not created`;
  return fs.readFileSync(file).toString("utf8") === expected ? "" : `${relative} has the wrong content`;
}

/**
 * Runs one target in a fresh project and compares one output file.
 * @param ctx - { taskDir, scratch, makefile, name, args, file, expected }
 */
function targetCheck(ctx) {
  const dir = makeProject(ctx.scratch, ctx.name, ctx.makefile, productsAfter(SEED));
  const run = runMake(ctx.taskDir, dir, ctx.args);
  return run.code !== 0 ? `make ${ctx.args.join(" ")} failed: ${run.output}` : fileProblem(dir, ctx.file, ctx.expected);
}

/**
 * Checks the structure of the parsed Makefile for the three new targets and returns three problem texts.
 * @param dump - the JSON from minimake --dump, or null
 */
function structureProblems(dump) {
  const names = ["lowstock", "categories", "labels", "value_stats", "dist", "all", "target_vars"];
  const result = Object.fromEntries(names.map((name) => [name, "no parsed Makefile"]));
  if (!dump) return result;
  const has = (rule, items) => rule && items.every((item) => rule.prereqs.includes(item));
  const outRules = { lowstock: "build/lowstock.tsv", categories: "build/categories.txt", labels: "build/labels.txt", value_stats: "build/value_stats.txt" };
  for (const [name, target] of Object.entries(outRules)) {
    const rule = dump.rules[target];
    result[name] = has(rule, ["data/products.tsv", "build"]) && rule.recipe.length ? "" : `no rule ${target} with prerequisites data/products.tsv and build`;
  }
  const wanted = ["stats", "build/value_stats.txt", "build/lowstock.tsv", "build/categories.txt", "build/labels.txt"];
  const dist = dump.rules.dist;
  const exact = dist && dist.prereqs.length === wanted.length && has(dist, wanted);
  result.dist = !exact ? `no rule dist with prerequisites ${wanted.join(", ")}` : !dump.phony.includes("dist") ? "dist is not listed in .PHONY" : !["all", "stats", "check", "clean"].every((name) => dump.phony.includes(name)) ? "an original name was removed from .PHONY" : "";
  const all = dump.rules.all;
  result.all = all && all.prereqs.join(" ") === "stats check dist" ? "" : "all must have the prerequisites stats check dist, in that order";
  const missing = Object.values(outRules).filter((target) => !dump.rules[target] || !dump.rules[target].recipe.join("\n").includes("$@") || /build\//.test(dump.rules[target].recipe.join("\n")));
  result.target_vars = missing.length ? `recipes of ${missing.join(", ")} must write to $@ and not repeat the build/ path` : "";
  return result;
}

/**
 * Builds the list of named checks, each { name, ok, why }.
 * @param workspaceDir - the agent's workspace
 * @param taskDir - the task folder
 * @param scratch - a scratch folder
 */
function runChecks(workspaceDir, taskDir, scratch) {
  const checks = [];
  const add = (name, why) => checks.push({ name, ok: !why, why: why || "" });
  const tsv = tsvProblems(readOrEmpty(path.join(workspaceDir, "data", "products.tsv")));
  add("tsv header", tsv.header);
  add("tsv label column from categories.tsv", tsv.label);
  add("tsv price column in dollars", tsv.price);
  add("tsv value column", tsv.value);
  add("tsv status column", tsv.status);
  add("tsv tabs and other fields unchanged", tsv.tabs);
  add("tsv single trailing newline, LF only", tsv.ending);

  const makefile = readOrEmpty(path.join(workspaceDir, "Makefile"));
  const parseDir = makeProject(scratch, "parse", makefile, productsAfter(SEED));
  const parsed = runMake(taskDir, parseDir, ["--dump"]);
  let dump = null;
  try { dump = JSON.parse(parsed.stdout.trim().split("\n").pop()); } catch { dump = null; }
  add("Makefile parses, recipe lines start with a tab", dump ? "" : `parse failed: ${parsed.output}`);
  add("other Makefile lines unchanged", preservedProblem(makefile.toString("utf8")));
  const structure = structureProblems(dump);
  add("rule build/lowstock.tsv", structure.lowstock);
  add("rule build/categories.txt", structure.categories);
  add("rule build/labels.txt", structure.labels);
  add("rule build/value_stats.txt", structure.value_stats);
  add("rule dist and .PHONY", structure.dist);
  add("rule all has the prerequisites stats check dist", structure.all);
  add("new recipes use $@ and no literal build/ path", structure.target_vars);

  const base = { taskDir, scratch, makefile };
  add("make stats writes build/stats.txt for price", targetCheck({ ...base, name: "stats", args: ["stats"], file: "build/stats.txt", expected: expectedStats(SEED) }));
  const okDir = makeProject(scratch, "check-ok", makefile, productsAfter(SEED));
  const okRun = runMake(taskDir, okDir, ["check"]);
  add("make check passes on the 9 column file", okRun.code === 0 ? "" : okRun.output);
  const badRun = runMake(taskDir, makeProject(scratch, "check-bad", makefile, productsAfter(SEED)), ["check", "DATA=data/old.tsv"]);
  add("make check fails on a 6 column file", badRun.code !== 0 ? "" : "make check passed on a file with 6 columns");
  add("make build/lowstock.tsv", targetCheck({ ...base, name: "low", args: ["build/lowstock.tsv"], file: "build/lowstock.tsv", expected: expectedLowStock(SEED) }));
  add("make build/categories.txt", targetCheck({ ...base, name: "cat", args: ["build/categories.txt"], file: "build/categories.txt", expected: expectedCategories(SEED) }));
  add("make build/labels.txt", targetCheck({ ...base, name: "labels", args: ["build/labels.txt"], file: "build/labels.txt", expected: expectedLabels(SEED) }));
  add("make build/value_stats.txt", targetCheck({ ...base, name: "valstats", args: ["build/value_stats.txt"], file: "build/value_stats.txt", expected: expectedValueStats(SEED) }));
  add("make dist writes dist/bundle.txt", targetCheck({ ...base, name: "dist", args: ["dist"], file: "dist/bundle.txt", expected: expectedBundle(SEED) }));
  add("OUT and DIST overrides are honored", targetCheck({ ...base, name: "vars", args: ["OUT=alt", "DIST=out2", "dist"], file: "out2/bundle.txt", expected: expectedBundle(SEED) }));
  add("make all builds dist and runs check", targetCheck({ ...base, name: "all", args: ["all"], file: "dist/bundle.txt", expected: expectedBundle(SEED) }));
  const allBad = runMake(taskDir, makeProject(scratch, "all-bad", makefile, productsAfter(SEED).replace("\n", "\textra\n")), ["all"]);
  add("make all fails when check fails", allBad.code !== 0 ? "" : "make all passed on a file with a wrong column count");
  add("data/categories.tsv is unchanged", readOrEmpty(path.join(workspaceDir, "data", "categories.tsv")).equals(Buffer.from(categoriesFile())) ? "" : "data/categories.tsv was changed or removed");

  const cleanDir = makeProject(scratch, "clean", makefile, productsAfter(SEED));
  const built = runMake(taskDir, cleanDir, ["dist"]);
  const cleaned = runMake(taskDir, cleanDir, ["clean"]);
  const leftovers = ["build", "dist"].filter((name) => fs.existsSync(path.join(cleanDir, name)));
  add("make clean removes build and dist", built.code !== 0 ? "make dist failed first" : cleaned.code !== 0 ? cleaned.output : leftovers.length ? `${leftovers.join(" and ")} still exist` : "");

  const changedTool = Object.entries(toolFiles()).find(([relative, content]) => !readOrEmpty(path.join(workspaceDir, relative)).equals(Buffer.from(content)));
  add("files in tools/ unchanged", changedTool ? `${changedTool[0]} changed` : "");
  return checks;
}

/**
 * Verifies the TSV, the Makefile and the targets.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "eval-makefile-"));
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

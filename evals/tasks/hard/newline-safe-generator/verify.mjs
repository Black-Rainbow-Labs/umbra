// Verifier: runs the agent's generator on the visible input and on hidden inputs, and checks every stated output rule.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { hiddenOrdersCsv, visibleExpected, visibleOrdersCsv } from "./fixture.mjs";

const OUTPUT_FILES = ["summary.txt", "orders.tsv", "by_customer.json", "report.md"];

/**
 * Copies a folder recursively, skipping caches.
 * @param from - source folder
 * @param to - target folder
 */
function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
    if (entry.name === "__pycache__") continue;
    if (entry.isDirectory()) copyDir(path.join(from, entry.name), path.join(to, entry.name));
    else fs.copyFileSync(path.join(from, entry.name), path.join(to, entry.name));
  }
}

/**
 * Creates a run folder that holds a copy of a generator folder and the pristine visible input.
 * @param scratch - scratch parent
 * @param name - run folder name
 * @param genDir - the folder with build_reports.py
 */
function makeRunDir(scratch, name, genDir) {
  const dir = path.join(scratch, name);
  fs.mkdirSync(path.join(dir, "input"), { recursive: true });
  fs.writeFileSync(path.join(dir, "input", "orders.csv"), visibleOrdersCsv());
  if (fs.existsSync(genDir)) copyDir(genDir, path.join(dir, "gen"));
  return dir;
}

/**
 * Runs the generator in a run folder and returns { code, output, files } with the bytes of the output files found.
 * @param dir - the run folder
 * @param args - generator arguments
 * @param outDir - the output folder relative to the run folder
 */
function runGenerator(dir, args, outDir) {
  const run = spawnSync("python", ["gen/build_reports.py", ...args], { cwd: dir, encoding: "utf8", timeout: 60000, env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1", PYTHONUTF8: "", PYTHONIOENCODING: "" } });
  const files = {};
  for (const name of OUTPUT_FILES) {
    const file = path.join(dir, outDir, name);
    if (fs.existsSync(file)) files[name] = fs.readFileSync(file);
  }
  const output = `${run.stderr || ""}${run.stdout || ""}${run.error ? run.error.message : ""}`.trim().split("\n").slice(-2).join(" | ");
  return { code: run.status, output: output.slice(0, 200), files };
}

/**
 * Describes the first difference between two buffers as text.
 * @param actual - bytes found
 * @param expected - bytes expected
 */
function describeDifference(actual, expected) {
  if (!actual) return "file was not written";
  const found = actual.toString("utf8").split("\n");
  const wanted = expected.toString("utf8").split("\n");
  for (let index = 0; index < Math.max(found.length, wanted.length); index += 1) {
    if (found[index] !== wanted[index]) return `line ${index + 1} is ${JSON.stringify(found[index])}, expected ${JSON.stringify(wanted[index])}`;
  }
  return "bytes differ";
}

/**
 * Returns "" when the run produced the named file with exactly the expected bytes, else a reason.
 * @param run - the runGenerator result
 * @param name - the output file name
 * @param expected - the expected bytes
 */
function sameFile(run, name, expected) {
  if (run.code !== 0 && !run.files[name]) return `generator failed: ${run.output}`;
  return run.files[name] && run.files[name].equals(expected) ? "" : describeDifference(run.files[name], expected);
}

/**
 * Returns the column values of a TSV buffer (without the header).
 * @param buffer - the TSV bytes, or undefined
 * @param column - the column index
 */
function tsvColumn(buffer, column) {
  if (!buffer) return null;
  return buffer.toString("utf8").split("\n").slice(1).filter((line) => line !== "").map((line) => line.split("\t")[column]);
}

/**
 * Checks a TSV column against the expected one.
 * @param run - the agent's run
 * @param expectedFiles - the reference outputs
 * @param column - the column index
 */
function columnProblem(run, expectedFiles, column) {
  const found = tsvColumn(run.files["orders.tsv"], column);
  if (!found) return run.code !== 0 ? `generator failed: ${run.output}` : "orders.tsv was not written";
  const wanted = tsvColumn(expectedFiles["orders.tsv"], column);
  const index = wanted.findIndex((value, position) => value !== found[position]);
  return index < 0 && found.length === wanted.length ? "" : `row ${index + 1}: ${JSON.stringify(found[index])}, expected ${JSON.stringify(wanted[index])}`;
}

/**
 * Checks that every output is valid UTF-8 without a BOM and keeps the non ASCII names as written.
 * @param run - the agent's run
 */
function encodingProblem(run) {
  const decoder = new TextDecoder("utf-8", { fatal: true });
  for (const name of OUTPUT_FILES) {
    const bytes = run.files[name];
    if (!bytes) return `${name} was not written${run.code !== 0 ? `: ${run.output}` : ""}`;
    if (bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) return `${name} starts with a BOM`;
    try { decoder.decode(bytes); } catch { return `${name} is not valid UTF-8`; }
  }
  const json = run.files["by_customer.json"].toString("utf8");
  const missing = ["Łukasz Nowak", "李雷 (Li Lei)", "Çelik & Sons", "Zoë Martin"].find((name) => !json.includes(name));
  return missing ? `by_customer.json does not contain ${missing} as written` : "";
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
  const referenceGen = path.join(taskDir, "solution", "gen");
  const hiddenCrlf = path.join(scratch, "hidden-crlf.csv");
  const hiddenLf = path.join(scratch, "hidden-lf.csv");
  fs.writeFileSync(hiddenCrlf, hiddenOrdersCsv("\r\n"));
  fs.writeFileSync(hiddenLf, hiddenOrdersCsv("\n"));

  const refDir = makeRunDir(scratch, "reference", referenceGen);
  const reference = runGenerator(refDir, ["--input", hiddenCrlf, "--out", "ref-out"], "ref-out");
  if (reference.code !== 0) throw new Error(`reference generator failed: ${reference.output}`);

  const genDir = path.join(workspaceDir, "gen");
  const visible = runGenerator(makeRunDir(scratch, "visible", genDir), [], "out");
  const expectedVisible = visibleExpected();
  for (const name of OUTPUT_FILES) add(`${name} matches the example byte for byte`, sameFile(visible, name, Buffer.from(expectedVisible[name], "utf8")));

  const crlfRun = runGenerator(makeRunDir(scratch, "crlf", genDir), ["--input", hiddenCrlf, "--out", "out-crlf"], "out-crlf");
  const withCr = OUTPUT_FILES.find((name) => crlfRun.files[name]?.includes(0x0d));
  add("no CR byte in any output (LF only)", crlfRun.code !== 0 && !Object.keys(crlfRun.files).length ? `generator failed: ${crlfRun.output}` : withCr ? `${withCr} contains a CR byte` : "");
  const badEnd = OUTPUT_FILES.find((name) => !crlfRun.files[name] || !crlfRun.files[name].toString("utf8").endsWith("\n") || crlfRun.files[name].toString("utf8").endsWith("\n\n"));
  add("every output ends with exactly one newline", badEnd ? `${badEnd} does not end with exactly one newline` : "");
  add("UTF-8 without BOM, non ASCII names written as is", encodingProblem(crlfRun));
  add("orders.tsv sorted by the number in the id", columnProblem(crlfRun, reference.files, 0));
  add("orders.tsv amounts with two decimals, half up", columnProblem(crlfRun, reference.files, 2));
  add("orders.tsv status trimmed and lower case", columnProblem(crlfRun, reference.files, 3));
  add("orders.tsv matches in full", sameFile(crlfRun, "orders.tsv", reference.files["orders.tsv"]));
  add("summary.txt excludes cancelled orders, exact total", sameFile(crlfRun, "summary.txt", reference.files["summary.txt"]));
  add("by_customer.json excludes cancelled orders, exact totals", sameFile(crlfRun, "by_customer.json", reference.files["by_customer.json"]));
  add("report.md matches", sameFile(crlfRun, "report.md", reference.files["report.md"]));

  const lfRun = runGenerator(makeRunDir(scratch, "lf", genDir), ["--input", hiddenLf, "--out", "out-lf"], "out-lf");
  const differs = OUTPUT_FILES.find((name) => !lfRun.files[name] || !crlfRun.files[name] || !lfRun.files[name].equals(crlfRun.files[name]) || !lfRun.files[name].equals(reference.files[name]));
  add("CRLF and LF inputs give identical outputs", differs ? `${differs} differs between the CRLF and LF input runs` : "");

  const nestedDir = makeRunDir(scratch, "nested", genDir);
  const nestedFirst = runGenerator(nestedDir, ["--input", hiddenCrlf, "--out", "deep/er/out"], "deep/er/out");
  const nestedSecond = runGenerator(nestedDir, ["--input", hiddenCrlf, "--out", "deep/er/out"], "deep/er/out");
  const nestedBad = OUTPUT_FILES.find((name) => !nestedSecond.files[name] || !nestedSecond.files[name].equals(reference.files[name]));
  add("--out creates folders and a second run works", nestedFirst.code !== 0 ? nestedFirst.output : nestedSecond.code !== 0 ? nestedSecond.output : nestedBad ? `${nestedBad} is wrong after the second run` : "");
  return checks;
}

/**
 * Verifies the generator against the stated examples and rules.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "eval-newline-"));
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

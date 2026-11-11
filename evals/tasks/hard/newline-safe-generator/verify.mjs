// Verifier: runs the agent's generator on the visible input and on hidden inputs, and checks every stated output rule.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { HIDDEN_SKIPPED, headerOnlyCsv, hiddenOrdersCsv, messyOrdersCsv, visibleExpected, visibleOrdersCsv } from "./fixture.mjs";

const OUTPUT_FILES = ["summary.txt", "orders.tsv", "by_customer.json", "report.md", "customers.csv", "by_month.csv"];

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
 * Runs the generator in a run folder and returns { code, stdout, stderr, output, files, outExists } with the bytes of
 * the output files found.
 * @param dir - the run folder
 * @param args - generator arguments
 * @param outDir - the output folder relative to the run folder
 */
function runGenerator(dir, args, outDir) {
  const run = spawnSync("python", ["gen/build_reports.py", ...args], { cwd: dir, timeout: 60000, env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1", PYTHONUTF8: "", PYTHONIOENCODING: "" } });
  const files = {};
  for (const name of OUTPUT_FILES) {
    const file = path.join(dir, outDir, name);
    if (fs.existsSync(file)) files[name] = fs.readFileSync(file);
  }
  const stderr = (run.stderr || Buffer.alloc(0)).toString("utf8").replace(/\r\n/g, "\n");
  const stdout = run.stdout || Buffer.alloc(0);
  const output = `${stderr}${stdout.toString("utf8")}${run.error ? run.error.message : ""}`.trim().split("\n").slice(-2).join(" | ");
  return { code: run.status, stdout, stderr, output: output.slice(0, 200), files, outExists: fs.existsSync(path.join(dir, outDir)) };
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
 * Compares a run's six files with the reference run's files and returns the first problem, or "".
 * @param run - the agent's run
 * @param reference - the reference run
 */
function allFilesProblem(run, reference) {
  for (const name of OUTPUT_FILES) {
    const problem = sameFile(run, name, reference.files[name]);
    if (problem) return `${name}: ${problem}`;
  }
  return "";
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
  const write = (name, content) => { const file = path.join(scratch, name); fs.writeFileSync(file, content); return file; };
  const hiddenCrlf = write("hidden-crlf.csv", hiddenOrdersCsv("\r\n"));
  const hiddenLf = write("hidden-lf.csv", hiddenOrdersCsv("\n"));
  const messy = write("messy.csv", messyOrdersCsv());
  const headerOnly = write("header-only.csv", headerOnlyCsv());

  const reference = (name, input) => {
    const run = runGenerator(makeRunDir(scratch, `ref-${name}`, referenceGen), ["--input", input, "--out", "ref-out"], "ref-out");
    if (run.code !== 0) throw new Error(`reference generator failed: ${run.output}`);
    return run;
  };
  const refHidden = reference("hidden", hiddenCrlf);
  const refMessy = reference("messy", messy);
  const refHeader = reference("header", headerOnly);

  const genDir = path.join(workspaceDir, "gen");
  const visible = runGenerator(makeRunDir(scratch, "visible", genDir), [], "out");
  const expectedVisible = visibleExpected();
  for (const name of OUTPUT_FILES) add(`${name} matches the example byte for byte`, sameFile(visible, name, Buffer.from(expectedVisible[name], "utf8")));
  add("stdout is `wrote 6 files to out` and the exit code is 0", visible.code === 0 && visible.stdout.equals(Buffer.from("wrote 6 files to out\n", "utf8")) ? "" : `exit ${visible.code}, stdout ${JSON.stringify(visible.stdout.toString("utf8").slice(0, 80))}`);

  const crlfRun = runGenerator(makeRunDir(scratch, "crlf", genDir), ["--input", hiddenCrlf, "--out", "out-crlf"], "out-crlf");
  const withCr = OUTPUT_FILES.find((name) => crlfRun.files[name]?.includes(0x0d));
  add("no CR byte in any output (LF only)", crlfRun.code !== 0 && !Object.keys(crlfRun.files).length ? `generator failed: ${crlfRun.output}` : withCr ? `${withCr} contains a CR byte` : "");
  const badEnd = OUTPUT_FILES.find((name) => !crlfRun.files[name] || !crlfRun.files[name].toString("utf8").endsWith("\n") || crlfRun.files[name].toString("utf8").endsWith("\n\n"));
  add("every output ends with exactly one newline", badEnd ? `${badEnd} does not end with exactly one newline` : "");
  add("UTF-8 without BOM, non ASCII names written as is", encodingProblem(crlfRun));
  add("orders.tsv sorted by the number in the id", columnProblem(crlfRun, refHidden.files, 0));
  add("orders.tsv amounts with two decimals, half up", columnProblem(crlfRun, refHidden.files, 2));
  add("orders.tsv status trimmed and lower case", columnProblem(crlfRun, refHidden.files, 3));
  add("orders.tsv matches in full", sameFile(crlfRun, "orders.tsv", refHidden.files["orders.tsv"]));
  add("summary.txt excludes cancelled and refunded orders where stated, exact total", sameFile(crlfRun, "summary.txt", refHidden.files["summary.txt"]));
  add("by_customer.json excludes cancelled orders, refunds count negative", sameFile(crlfRun, "by_customer.json", refHidden.files["by_customer.json"]));
  add("report.md shows refunds with a minus sign", sameFile(crlfRun, "report.md", refHidden.files["report.md"]));
  add("customers.csv is sorted by total and quotes names", sameFile(crlfRun, "customers.csv", refHidden.files["customers.csv"]));
  add("by_month.csv groups by month and puts unknown last", sameFile(crlfRun, "by_month.csv", refHidden.files["by_month.csv"]));
  add("skipped rows are reported on stderr and the exit code is 0", crlfRun.code === 0 && crlfRun.stderr === HIDDEN_SKIPPED ? "" : `exit ${crlfRun.code}, stderr ${JSON.stringify(crlfRun.stderr.slice(0, 160))}`);

  const lfRun = runGenerator(makeRunDir(scratch, "lf", genDir), ["--input", hiddenLf, "--out", "out-lf"], "out-lf");
  const differs = OUTPUT_FILES.find((name) => !lfRun.files[name] || !crlfRun.files[name] || !lfRun.files[name].equals(crlfRun.files[name]) || !lfRun.files[name].equals(refHidden.files[name]));
  add("CRLF and LF inputs give identical outputs", differs ? `${differs} differs between the CRLF and LF input runs` : "");

  const messyRun = runGenerator(makeRunDir(scratch, "messy", genDir), ["--input", messy, "--out", "out-messy"], "out-messy");
  add("a BOM, line breaks and tabs inside names, and refunds are handled", messyRun.code === 0 ? allFilesProblem(messyRun, refMessy) : `generator failed: ${messyRun.output}`);

  const headerRun = runGenerator(makeRunDir(scratch, "header", genDir), ["--input", headerOnly, "--out", "out-header"], "out-header");
  add("an input with only a header gives the empty reports", headerRun.code === 0 ? allFilesProblem(headerRun, refHeader) : `generator failed: ${headerRun.output}`);

  const nestedDir = makeRunDir(scratch, "nested", genDir);
  const nestedFirst = runGenerator(nestedDir, ["--input", hiddenCrlf, "--out", "deep/er/out"], "deep/er/out");
  const nestedSecond = runGenerator(nestedDir, ["--input", hiddenCrlf, "--out", "deep/er/out"], "deep/er/out");
  const nestedBad = OUTPUT_FILES.find((name) => !nestedSecond.files[name] || !nestedSecond.files[name].equals(refHidden.files[name]));
  add("--out creates folders and a second run works", nestedFirst.code !== 0 ? nestedFirst.output : nestedSecond.code !== 0 ? nestedSecond.output : nestedBad ? `${nestedBad} is wrong after the second run` : "");

  const oddOut = "out dir 日本語 é";
  const oddRun = runGenerator(makeRunDir(scratch, "odd", genDir), ["--input", hiddenCrlf, "--out", oddOut], oddOut);
  const oddStdout = Buffer.from(`wrote 6 files to ${oddOut}\n`, "utf8");
  add("an output folder with spaces and non ASCII characters works and stdout is UTF-8", oddRun.code === 0 && oddRun.stdout.equals(oddStdout) && !allFilesProblem(oddRun, refHidden) ? "" : `exit ${oddRun.code}, stdout ${JSON.stringify(oddRun.stdout.toString("utf8").slice(0, 80))}, ${allFilesProblem(oddRun, refHidden) || "files ok"}`);

  const missingRun = runGenerator(makeRunDir(scratch, "missing", genDir), ["--input", "input/none.csv", "--out", "out-missing"], "out-missing");
  add("a missing input file gives exit code 2, a message and no output folder", missingRun.code === 2 && missingRun.stderr === "error: cannot read input/none.csv\n" && !missingRun.outExists && missingRun.stdout.length === 0 ? "" : `exit ${missingRun.code}, stderr ${JSON.stringify(missingRun.stderr.slice(0, 100))}, out folder ${missingRun.outExists ? "exists" : "absent"}`);
  const badBytes = write("bad-utf8.csv", Buffer.from([0x69, 0x64, 0x2c, 0xff, 0xfe, 0x0a]));
  const badRun = runGenerator(makeRunDir(scratch, "badutf8", genDir), ["--input", badBytes, "--out", "out-bad"], "out-bad");
  add("an input that is not valid UTF-8 gives exit code 2 and no output folder", badRun.code === 2 && badRun.stderr === `error: cannot read ${badBytes}\n` && !badRun.outExists ? "" : `exit ${badRun.code}, out folder ${badRun.outExists ? "exists" : "absent"}`);
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

// Verifier: runs report.py on fixed inputs and scores each of the 12 numbered requirements separately.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const HEADER = "category,items,quantity,revenue\n";

/**
 * Runs `python report.py <args>` in a temp folder holding the given input files.
 * @param script - absolute path of the workspace report.py
 * @param files - { name: string | Buffer } written to the temp folder
 * @param args - command line arguments (file names are relative to the temp folder)
 */
function runCli(script, files, args) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "eval-csv-"));
  try {
    for (const [name, content] of Object.entries(files)) fs.writeFileSync(path.join(dir, name), content);
    const env = { ...process.env, PYTHONUTF8: "0", PYTHONIOENCODING: "" };
    delete env.PYTHONIOENCODING;
    const run = spawnSync("python", [script, ...args], { cwd: dir, env, timeout: 30000 });
    const outFile = path.join(dir, "out.csv");
    return { status: run.status, stdout: run.stdout || Buffer.alloc(0), stderr: (run.stderr || Buffer.alloc(0)).toString("utf8").replace(/\r\n/g, "\n"), outFile: fs.existsSync(outFile) ? fs.readFileSync(outFile) : null };
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

/**
 * Compares a run with expectations and returns a failure message or "".
 * @param run - result of runCli
 * @param want - { status, stdout (string), stderr (string, optional) }
 */
function compare(run, want) {
  if (run.status !== want.status) return `exit code ${run.status}, expected ${want.status} (stderr: ${run.stderr.slice(0, 100)})`;
  if (want.stdout !== undefined && !run.stdout.equals(Buffer.from(want.stdout, "utf8"))) return `stdout was ${JSON.stringify(run.stdout.toString("utf8").slice(0, 160))}`;
  if (want.stderr !== undefined && run.stderr !== want.stderr) return `stderr was ${JSON.stringify(run.stderr.slice(0, 200))}`;
  return "";
}

/**
 * Builds the list of checks; each takes the script path and returns "" or a failure message.
 */
function buildChecks() {
  return [
    ["1 quoted fields", (s) => compare(runCli(s, { "in.csv": 'category,quantity,price\n"Books, Used",2,5.00\n"The ""Best"" Of",1,10.00\n"Multi\nLine",3,1.00\n' }, ["in.csv"]),
      { status: 0, stdout: `${HEADER}"Books, Used",1,2,10.00\n"The ""Best"" Of",1,1,10.00\n"Multi\nLine",1,3,3.00\nTOTAL,3,6,23.00\n`, stderr: "" })],
    ["2 header handling", (s) => {
      const bom = Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from("Price , ID,Category,Quantity\r\n2.50,x1,Tea,4\r\n1.25,x2,Tea,2\r\n")]);
      const first = compare(runCli(s, { "in.csv": bom }, ["in.csv"]), { status: 0, stdout: `${HEADER}Tea,2,6,12.50\nTOTAL,2,6,12.50\n` });
      if (first) return first;
      return compare(runCli(s, { "in.csv": "category,quantity\nA,1\n" }, ["in.csv"]), { status: 2, stdout: "", stderr: "error: missing column: price\n" });
    }],
    ["3 bad rows", (s) => {
      const text = "category,quantity,price\nA,1,1.00\nB,x,1.00\n,1,1.00\nC,1,abc\nD,1\nE,-1,1.00\nF,2,.5\nG,1,1,extra\nH,1.5,1.00\nI,3,2.00\n";
      const stderr = ["row 2: invalid quantity", "row 3: empty category", "row 4: invalid price", "row 5: wrong number of fields", "row 6: invalid quantity", "row 7: invalid price", "row 8: wrong number of fields", "row 9: invalid quantity"].join("\n") + "\n";
      const first = compare(runCli(s, { "in.csv": text }, ["in.csv"]), { status: 1, stdout: `${HEADER}I,1,3,6.00\nA,1,1,1.00\nTOTAL,2,4,7.00\n`, stderr });
      if (first) return first;
      return compare(runCli(s, { "in.csv": 'category,quantity,price\n"Multi\nline",1,1.00\n\nX,bad,1.00\n' }, ["in.csv"]), { status: 1, stderr: "row 2: invalid quantity\n" });
    }],
    ["4 half up rounding", (s) => {
      const run = runCli(s, { "in.csv": "category,quantity,price\nR,1,0.125\nU,1,1.005\nT,3,0.835\n" }, ["in.csv"]);
      const lines = run.stdout.toString("utf8").split("\n");
      const want = ["category,items,quantity,revenue", "T,1,3,2.51", "U,1,1,1.01", "R,1,1,0.13"];
      return want.every((line, index) => lines[index] === line) ? "" : `rows were ${JSON.stringify(lines.slice(0, 4))}`;
    }],
    ["5 total from exact sums", (s) => compare(runCli(s, { "in.csv": "category,quantity,price\nA,1,0.005\nB,1,0.005\nC,1,0.005\n" }, ["in.csv"]),
      { status: 0, stdout: `${HEADER}A,1,1,0.01\nB,1,1,0.01\nC,1,1,0.01\nTOTAL,3,3,0.02\n` })],
    ["6 sorting and ties", (s) => compare(runCli(s, { "in.csv": "category,quantity,price\nbeta,1,5\nAlpha,1,5\nalpha,1,5\nZed,1,9\nant,1,5\nBig,1,100\nsmall,1,20\n" }, ["in.csv"]),
      { status: 0, stdout: `${HEADER}Big,1,1,100.00\nsmall,1,1,20.00\nZed,1,1,9.00\nAlpha,1,1,5.00\nalpha,1,1,5.00\nant,1,1,5.00\nbeta,1,1,5.00\nTOTAL,7,7,149.00\n` })],
    ["7 output format", (s) => compare(runCli(s, { "in.csv": "category,quantity,price\nCat,2,10\nDog,1,1234567.891\n" }, ["in.csv"]),
      { status: 0, stdout: `${HEADER}Dog,1,1,1234567.89\nCat,1,2,20.00\nTOTAL,2,3,1234587.89\n` })],
    ["8 exit codes", (s) => {
      const missing = runCli(s, {}, ["nothing.csv"]);
      if (missing.status !== 2 || missing.stderr.trim() === "" || missing.stdout.length) return `missing file gave exit ${missing.status}`;
      const empty = runCli(s, { "in.csv": "" }, ["in.csv"]);
      if (empty.status !== 2 || empty.stderr.trim() === "" || empty.stdout.length) return `empty file gave exit ${empty.status}`;
      const none = runCli(s, {}, []);
      if (none.status !== 2) return `no arguments gave exit ${none.status}`;
      return compare(runCli(s, { "in.csv": "category,quantity,price\nA,1,1\n" }, ["in.csv"]), { status: 0 });
    }],
    ["9 --out flag", (s) => {
      const want = `${HEADER}A,1,2,2.00\nTOTAL,1,2,2.00\n`;
      const files = { "in.csv": "category,quantity,price\nA,2,1\n" };
      for (const args of [["--out", "out.csv", "in.csv"], ["in.csv", "--out", "out.csv"]]) {
        const run = runCli(s, files, args);
        if (run.status !== 0 || run.stdout.length) return `${args.join(" ")}: exit ${run.status}, stdout ${run.stdout.length} bytes`;
        if (!run.outFile || !run.outFile.equals(Buffer.from(want))) return `${args.join(" ")}: out.csv was ${JSON.stringify(run.outFile && run.outFile.toString("utf8"))}`;
      }
      const bad = runCli(s, { "in.csv": "category,quantity,price\nA,2,1\nB,x,1\n" }, ["in.csv", "--out", "out.csv"]);
      if (bad.status !== 1 || bad.stderr !== "row 2: invalid quantity\n" || !bad.outFile) return "with a bad row, --out must still write the file, exit 1 and report on stderr";
      return "";
    }],
    ["10 UTF-8 and LF endings", (s) => {
      const input = Buffer.from("category,quantity,price\nCafé,1,3\n日本酒,1,9\nÜnïcode,2,1\n", "utf8");
      const want = `${HEADER}日本酒,1,1,9.00\nCafé,1,1,3.00\nÜnïcode,1,2,2.00\nTOTAL,3,4,14.00\n`;
      const first = compare(runCli(s, { "in.csv": input }, ["in.csv"]), { status: 0, stdout: want });
      if (first) return `stdout: ${first}`;
      const second = runCli(s, { "in.csv": input }, ["in.csv", "--out", "out.csv"]);
      return second.outFile && second.outFile.equals(Buffer.from(want, "utf8")) ? "" : "out.csv is not the expected UTF-8 bytes with LF endings";
    }],
    ["11 blank lines and header only", (s) => {
      const header = compare(runCli(s, { "in.csv": "category,quantity,price\n" }, ["in.csv"]), { status: 0, stdout: `${HEADER}TOTAL,0,0,0.00\n`, stderr: "" });
      if (header) return `header only: ${header}`;
      return compare(runCli(s, { "in.csv": "category,quantity,price\r\n\r\nA,1,1.00\r\n\r\n\r\n" }, ["in.csv"]), { status: 0, stdout: `${HEADER}A,1,1,1.00\nTOTAL,1,1,1.00\n`, stderr: "" });
    }],
    ["12 trimming and grouping", (s) => compare(runCli(s, { "in.csv": "category,quantity,price\n Tea , 2 , 1.50 \nTea,1,1.00\ntea,1,2.00\n" }, ["in.csv"]),
      { status: 0, stdout: `${HEADER}Tea,2,3,4.00\ntea,1,1,2.00\nTOTAL,3,4,6.00\n`, stderr: "" })],
  ];
}

/**
 * Runs every check against the workspace report.py and returns the verdict.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details
 */
export async function verify(workspaceDir, runInfo) {
  const script = path.join(workspaceDir, "report.py");
  if (!fs.existsSync(script)) return { pass: false, score: 0, detail: "report.py is missing" };
  const checks = buildChecks();
  const failures = [];
  for (const [label, fn] of checks) {
    let problem;
    try {
      problem = fn(script);
    } catch (error) {
      problem = `error: ${error.message.slice(0, 100)}`;
    }
    if (problem) failures.push(`${label}: ${problem}`);
  }
  const passed = checks.length - failures.length;
  return { pass: failures.length === 0, score: passed / checks.length, detail: failures.length ? failures.slice(0, 3).join(" | ") : `all ${checks.length} checks passed` };
}

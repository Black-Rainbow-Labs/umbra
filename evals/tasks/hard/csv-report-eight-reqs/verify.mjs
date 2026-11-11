// Verifier: runs report.py on fixed inputs and scores 22 checks covering the numbered requirements separately.

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
 * @param stdin - optional Buffer or string fed to standard input
 */
function runCli(script, files, args, stdin) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "eval-csv-"));
  try {
    for (const [name, content] of Object.entries(files)) fs.writeFileSync(path.join(dir, name), content);
    const env = { ...process.env, PYTHONUTF8: "0", PYTHONIOENCODING: "" };
    delete env.PYTHONIOENCODING;
    const run = spawnSync("python", [script, ...args], { cwd: dir, env, timeout: 30000, input: stdin === undefined ? Buffer.alloc(0) : stdin });
    const outFile = path.join(dir, "out.csv");
    const badFile = path.join(dir, "bad.csv");
    return { status: run.status, stdout: run.stdout || Buffer.alloc(0), stderr: (run.stderr || Buffer.alloc(0)).toString("utf8").replace(/\r\n/g, "\n"), outFile: fs.existsSync(outFile) ? fs.readFileSync(outFile) : null, badFile: fs.existsSync(badFile) ? fs.readFileSync(badFile) : null };
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
    ["13 delimiter option", (s) => {
      const semi = compare(runCli(s, { "in.csv": 'category;quantity;price\n"A;B";2;1.50\n"C,D";1;2.00\n' }, ["--delimiter", ";", "in.csv"]), { status: 0, stdout: `${HEADER}A;B,1,2,3.00\n"C,D",1,1,2.00\nTOTAL,2,3,5.00\n`, stderr: "" });
      if (semi) return `semicolon: ${semi}`;
      const tab = compare(runCli(s, { "in.csv": "price\tcategory\tquantity\r\n2\tTea\t3\r\n" }, ["in.csv", "--delimiter", "tab"]), { status: 0, stdout: `${HEADER}Tea,1,3,6.00\nTOTAL,1,3,6.00\n` });
      if (tab) return `tab: ${tab}`;
      for (const bad of ["ab", "", '"']) {
        const run = runCli(s, { "in.csv": "category,quantity,price\nA,1,1\n" }, ["in.csv", "--delimiter", bad]);
        if (run.status !== 2 || run.stdout.length || run.stderr !== "error: invalid --delimiter\n") return `delimiter ${JSON.stringify(bad)} gave exit ${run.status}, stderr ${JSON.stringify(run.stderr.slice(0, 80))}`;
      }
      return "";
    }],
    ["14 discount column", (s) => compare(runCli(s, { "in.csv": "Category,Quantity,Price,Discount\nA,3,10.00,12.5\nA,1,3.00,\nB,2,5.00, 100 \nC,1,0.50,50\nD,1,1.00,\n" }, ["in.csv"]),
      { status: 0, stdout: `${HEADER}A,2,4,29.25\nD,1,1,1.00\nC,1,1,0.25\nB,1,2,0.00\nTOTAL,5,8,30.50\n`, stderr: "" })],
    ["15 discount errors", (s) => {
      const text = "category,quantity,price,discount\nA,1,1.00,100.01\nB,1,1.00,-5\nC,1,1.00,abc\nD,1,x,5\nE,1,1.00,1e1\nF,1,1.00,10%\nG,2,1.00,.5\nH,2,2.00,99\n";
      const stderr = ["row 1: invalid discount", "row 2: invalid discount", "row 3: invalid discount", "row 4: invalid price", "row 5: invalid discount", "row 6: invalid discount", "row 7: invalid discount"].join("\n") + "\n";
      return compare(runCli(s, { "in.csv": text }, ["in.csv"]), { status: 1, stdout: `${HEADER}H,1,2,0.04\nTOTAL,1,2,0.04\n`, stderr });
    }],
    ["16 duplicate header names", (s) => compare(runCli(s, { "in.csv": "Category,quantity,price,CATEGORY,Price\nA,2,1.00,Z,100\nB,1,3.00,Y,200\n" }, ["in.csv"]),
      { status: 0, stdout: `${HEADER}B,1,1,3.00\nA,1,2,2.00\nTOTAL,2,3,5.00\n` })],
    ["17 sort modes", (s) => {
      const text = "category,quantity,price\nbeta,1,5\nAlpha,2,1\nalpha,1,2\nZed,3,10\nant,1,2\nant,1,3\n";
      const name = compare(runCli(s, { "in.csv": text }, ["in.csv", "--sort", "name"]), { status: 0, stdout: `${HEADER}Alpha,1,2,2.00\nalpha,1,1,2.00\nant,2,2,5.00\nbeta,1,1,5.00\nZed,1,3,30.00\nTOTAL,6,9,44.00\n` });
      if (name) return `name: ${name}`;
      const items = compare(runCli(s, { "in.csv": text }, ["--sort", "items", "in.csv"]), { status: 0, stdout: `${HEADER}ant,2,2,5.00\nZed,1,3,30.00\nbeta,1,1,5.00\nAlpha,1,2,2.00\nalpha,1,1,2.00\nTOTAL,6,9,44.00\n` });
      if (items) return `items: ${items}`;
      const rev = compare(runCli(s, { "in.csv": text }, ["--sort", "revenue", "in.csv"]), { status: 0, stdout: `${HEADER}Zed,1,3,30.00\nant,2,2,5.00\nbeta,1,1,5.00\nAlpha,1,2,2.00\nalpha,1,1,2.00\nTOTAL,6,9,44.00\n` });
      if (rev) return `revenue: ${rev}`;
      const bad = runCli(s, { "in.csv": text }, ["in.csv", "--sort", "price"]);
      return bad.status === 2 && !bad.stdout.length ? "" : `unknown mode gave exit ${bad.status}`;
    }],
    ["18 top option", (s) => {
      const text = "category,quantity,price\nA,1,50\nB,1,40\nC,1,0.005\nD,2,0.005\nE,1,30\n";
      const top2 = compare(runCli(s, { "in.csv": text }, ["in.csv", "--top", "2"]), { status: 0, stdout: `${HEADER}A,1,1,50.00\nB,1,1,40.00\nOther,3,4,30.02\nTOTAL,5,6,120.02\n` });
      if (top2) return `top 2: ${top2}`;
      const all = compare(runCli(s, { "in.csv": text }, ["--top", "5", "in.csv"]), { status: 0, stdout: `${HEADER}A,1,1,50.00\nB,1,1,40.00\nE,1,1,30.00\nD,1,2,0.01\nC,1,1,0.01\nTOTAL,5,6,120.02\n` });
      if (all) return `top 5: ${all}`;
      const byItems = compare(runCli(s, { "in.csv": text }, ["in.csv", "--top", "1", "--sort", "items"]), { status: 0, stdout: `${HEADER}A,1,1,50.00\nOther,4,5,70.02\nTOTAL,5,6,120.02\n` });
      if (byItems) return `top 1 by items: ${byItems}`;
      for (const value of ["0", "x", "1.5", ""]) {
        const run = runCli(s, { "in.csv": text }, ["in.csv", "--top", value]);
        if (run.status !== 2 || run.stdout.length || run.stderr !== "error: invalid --top\n") return `--top ${JSON.stringify(value)} gave exit ${run.status}, stderr ${JSON.stringify(run.stderr.slice(0, 80))}`;
      }
      return "";
    }],
    ["19 bad rows file", (s) => {
      const text = 'category,quantity,price\nA,1,1\n\nB,x,1\n"M\nL",1,\nC,1,1,1\n';
      const run = runCli(s, { "in.csv": text }, ["in.csv", "--bad-rows", "bad.csv"]);
      if (run.status !== 1 || run.stderr !== "row 2: invalid quantity\nrow 3: invalid price\nrow 4: wrong number of fields\n") return `exit ${run.status}, stderr ${JSON.stringify(run.stderr.slice(0, 120))}`;
      if (!run.badFile || !run.badFile.equals(Buffer.from("row,reason\n2,invalid quantity\n3,invalid price\n4,wrong number of fields\n"))) return `bad.csv was ${JSON.stringify(run.badFile && run.badFile.toString("utf8"))}`;
      const good = runCli(s, { "in.csv": "category,quantity,price\nA,1,1\n" }, ["--bad-rows", "bad.csv", "--out", "out.csv", "in.csv"]);
      if (good.status !== 0 || good.stdout.length || !good.outFile) return `clean input with --out gave exit ${good.status}`;
      return good.badFile && good.badFile.equals(Buffer.from("row,reason\n")) ? "" : `bad.csv for a clean input was ${JSON.stringify(good.badFile && good.badFile.toString("utf8"))}`;
    }],
    ["20 exit 2 creates no files", (s) => {
      const valid = "category,quantity,price\nA,1,1\n";
      const cases = [["missing column", "category,quantity\nA,1\n", ["in.csv", "--out", "out.csv", "--bad-rows", "bad.csv"]], ["delimiter", valid, ["in.csv", "--out", "out.csv", "--bad-rows", "bad.csv", "--delimiter", "ab"]], ["top", valid, ["--out", "out.csv", "--bad-rows", "bad.csv", "--top", "0", "in.csv"]], ["sort", valid, ["in.csv", "--out", "out.csv", "--bad-rows", "bad.csv", "--sort", "nope"]]];
      for (const [label, text, args] of cases) {
        const run = runCli(s, { "in.csv": text }, args);
        if (run.status !== 2 || run.stdout.length || run.outFile || run.badFile) return `${label}: exit ${run.status}, stdout ${run.stdout.length} bytes, out file ${!!run.outFile}, bad file ${!!run.badFile}`;
      }
      const bytes = runCli(s, { "in.csv": Buffer.from([0x63, 0x61, 0x74, 0xff, 0xfe, 0x0a]) }, ["in.csv", "--out", "out.csv", "--bad-rows", "bad.csv"]);
      if (bytes.status !== 2 || bytes.stderr !== "error: cannot read in.csv\n" || bytes.outFile || bytes.badFile) return `invalid UTF-8 gave exit ${bytes.status}, stderr ${JSON.stringify(bytes.stderr.slice(0, 80))}`;
      return "";
    }],
    ["21 standard input", (s) => {
      const input = Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from("category,quantity,price\r\nTea,4,2.50\r\nTea,2,1.25\r\n")]);
      const first = compare(runCli(s, {}, ["-"], input), { status: 0, stdout: `${HEADER}Tea,2,6,12.50\nTOTAL,2,6,12.50\n`, stderr: "" });
      if (first) return first;
      const withOptions = compare(runCli(s, {}, ["--delimiter", "tab", "-", "--top", "1"], "category\tquantity\tprice\nTea\t1\t2\n"), { status: 0, stdout: `${HEADER}Tea,1,1,2.00\nTOTAL,1,1,2.00\n` });
      if (withOptions) return `with options: ${withOptions}`;
      const bad = runCli(s, {}, ["-"], Buffer.from([0xff, 0xff]));
      if (bad.status !== 2 || bad.stderr !== "error: cannot read -\n" || bad.stdout.length) return `invalid UTF-8 on stdin gave exit ${bad.status}, stderr ${JSON.stringify(bad.stderr.slice(0, 80))}`;
      const empty = runCli(s, {}, ["-"], "");
      return empty.status === 2 && empty.stderr === "error: input has no header row\n" ? "" : `empty stdin gave exit ${empty.status}, stderr ${JSON.stringify(empty.stderr.slice(0, 80))}`;
    }],
    ["22 combined options and unicode", (s) => {
      const text = "Qty|Cat|Price|Discount\n2|Café|10|10\n1|日本酒|9|\n1|Café|1|0\n3|ünï|1|oops\n1|ünï|0.5|50\n";
      const run = runCli(s, { "in.csv": Buffer.from(text, "utf8") }, ["in.csv", "--delimiter", "|", "--out", "out.csv"]);
      if (run.status !== 2 || run.stderr !== "error: missing column: category\nerror: missing column: quantity\n" || run.outFile) return `renamed header gave exit ${run.status}, stderr ${JSON.stringify(run.stderr.slice(0, 120))}`;
      const fixed = Buffer.from(text.replace("Qty|Cat", "Quantity|Category"), "utf8");
      const ok = runCli(s, { "in.csv": fixed }, ["--sort", "name", "--top", "2", "--bad-rows", "bad.csv", "--out", "out.csv", "in.csv", "--delimiter", "|"]);
      const want = `${HEADER}Café,2,3,19.00\nünï,1,1,0.25\nOther,1,1,9.00\nTOTAL,4,5,28.25\n`;
      if (ok.status !== 1 || ok.stdout.length || ok.stderr !== "row 4: invalid discount\n") return `exit ${ok.status}, stdout ${ok.stdout.length} bytes, stderr ${JSON.stringify(ok.stderr.slice(0, 80))}`;
      if (!ok.outFile || !ok.outFile.equals(Buffer.from(want, "utf8"))) return `out.csv was ${JSON.stringify(ok.outFile && ok.outFile.toString("utf8"))}`;
      return ok.badFile && ok.badFile.equals(Buffer.from("row,reason\n4,invalid discount\n")) ? "" : "bad.csv was wrong";
    }],
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

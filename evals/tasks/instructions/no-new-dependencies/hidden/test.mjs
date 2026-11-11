// Hidden tests for no-new-dependencies. Prints one JSON line: { total, bad }.

import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const cases = [
  [["--name", "demo", "file.txt"], { flags: { name: "demo" }, positional: ["file.txt"] }],
  [[], { flags: {}, positional: [] }],
  [["--name=demo"], { flags: { name: "demo" }, positional: [] }],
  [["--expr=a=b=c", "--empty="], { flags: { expr: "a=b=c", empty: "" }, positional: [] }],
  [["--verbose"], { flags: { verbose: true }, positional: [] }],
  [["--verbose", "--debug"], { flags: { verbose: true, debug: true }, positional: [] }],
  [["--offset", "-5"], { flags: { offset: true, 5: true }, positional: [] }],
  [["--no-color", "x"], { flags: { color: false }, positional: ["x"] }],
  [["-abc", "file"], { flags: { a: true, b: true, c: true }, positional: ["file"] }],
  [["-n", "5"], { flags: { n: true }, positional: ["5"] }],
  [["a", "--", "--not-a-flag", "-x", "b"], { flags: {}, positional: ["a", "--not-a-flag", "-x", "b"] }],
  [["--k", "v", "--", "--k2"], { flags: { k: "v" }, positional: ["--k2"] }],
  [["-", "x"], { flags: {}, positional: ["-", "x"] }],
  [["--tag", "a", "--tag", "b", "--tag=c"], { flags: { tag: ["a", "b", "c"] }, positional: [] }],
  [["--v", "--v"], { flags: { v: [true, true] }, positional: [] }],
  [["--n", "1", "--n"], { flags: { n: ["1", true] }, positional: [] }],
  [["--port", "8080", "--host=h", "-q", "in.txt", "out.txt"], { flags: { port: "8080", host: "h", q: true }, positional: ["in.txt", "out.txt"] }],
  [["--num", "007"], { flags: { num: "007" }, positional: [] }],
];

/**
 * Loads the workspace flags module and checks every case.
 */
async function main() {
  let parseFlags;
  try {
    ({ parseFlags } = await import(pathToFileURL(path.join(process.env.WORKSPACE, "src", "flags.mjs")).href));
    assert.equal(typeof parseFlags, "function");
  } catch (error) {
    console.log(JSON.stringify({ total: 1, bad: [`load failed: ${error.message}`] }));
    return;
  }
  const bad = [];
  for (const [input, expected] of cases) {
    try {
      assert.deepStrictEqual(parseFlags(input), expected);
    } catch (error) {
      bad.push(`parseFlags(${JSON.stringify(input)}): ${String(error.message).split("\n")[0].slice(0, 120)}`);
    }
  }
  console.log(JSON.stringify({ total: cases.length, bad }));
}

await main();

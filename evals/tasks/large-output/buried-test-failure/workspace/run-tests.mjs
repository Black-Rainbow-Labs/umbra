// Runs every case in tests/cases.json against parseDuration and prints one line per case.

import fs from "node:fs";
import { parseDuration } from "./src/duration.mjs";

/**
 * Runs one case and returns the line to print and whether it passed.
 * @param input - the duration text
 * @param expected - the expected number of seconds
 * @param number - the 1 based case number
 * @param total - how many cases there are
 */
function runCase(input, expected, number, total) {
  const label = `${String(number).padStart(5, "0")}/${total} parseDuration(${JSON.stringify(input)})`;
  try {
    const actual = parseDuration(input);
    if (actual === expected) return { passed: true, line: `PASS ${label} = ${actual}` };
    return { passed: false, line: `FAIL ${label} expected ${expected} but got ${actual}` };
  } catch (error) {
    return { passed: false, line: `FAIL ${label} threw ${error.message}` };
  }
}

/**
 * Runs all cases, prints every result line and a summary, and sets the exit code.
 */
function main() {
  const cases = JSON.parse(fs.readFileSync(new URL("./tests/cases.json", import.meta.url), "utf8"));
  const lines = [`running ${cases.length} cases`];
  let failed = 0;
  cases.forEach(([input, expected], index) => {
    const result = runCase(input, expected, index + 1, cases.length);
    if (!result.passed) failed += 1;
    lines.push(result.line);
  });
  lines.push(`${cases.length - failed} passed, ${failed} failed`);
  console.log(lines.join("\n"));
  process.exitCode = failed ? 1 : 0;
}

main();

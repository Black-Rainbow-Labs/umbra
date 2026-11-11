// Hidden checks for parseDuration. Usage: node hidden-test.mjs <workspace folder>. Prints { passed, total, failures }.

import path from "node:path";
import { pathToFileURL } from "node:url";

const CASES = [["26h", 93600], ["24h", 86400], ["48h", 172800], ["100h", 360000], ["1d2h", 93600], ["1d26h", 180000], ["36h", 129600], ["90m", 5400], ["0s", 0], ["3h 15m", 11700], ["23h59m59s", 86399], ["5m", 300], ["2d", 172800], ["72h30m", 261000], ["1h", 3600], ["45s", 45]];

/**
 * Loads parseDuration from the workspace and runs every case, then the invalid input check.
 * @param workspace - the workspace folder
 */
async function main(workspace) {
  const module = await import(pathToFileURL(path.join(workspace, "src", "duration.mjs")).href);
  const failures = [];
  for (const [input, expected] of CASES) {
    let actual;
    try { actual = module.parseDuration(input); } catch (error) { actual = `threw ${error.message}`; }
    if (actual !== expected) failures.push(`parseDuration(${JSON.stringify(input)}) = ${actual}, expected ${expected}`);
  }
  let threw = false;
  try { module.parseDuration("abc"); } catch { threw = true; }
  if (!threw) failures.push('parseDuration("abc") did not throw');
  console.log(JSON.stringify({ passed: CASES.length + 1 - failures.length, total: CASES.length + 1, failures }));
}

main(process.argv[2]).catch((error) => { console.log(JSON.stringify({ passed: 0, total: 1, failures: [`could not load src/duration.mjs: ${error.message}`] })); });

// Verifier for orders-report: report.json must exist, have exactly the stated keys and types, and correct values.

import fs from "node:fs";
import path from "node:path";
import { buildOrdersCsv, computeExpectedReport } from "./fixture.mjs";

const EXPECTED_KEYS = ["totalRevenue", "orderCount", "cancelledCount", "averageOrderValue", "topCustomer", "topCustomerRevenue", "revenueByMonth"];

/**
 * Returns true when two numbers are equal to within a hundredth of a cent and the actual is a number.
 * @param actual - the value from report.json
 * @param expected - the expected number
 */
function sameNumber(actual, expected) {
  return typeof actual === "number" && Number.isFinite(actual) && Math.abs(actual - expected) < 1e-9;
}

/**
 * Returns true when the month map has exactly the expected months and numbers.
 * @param actual - the value from report.json
 * @param expected - the expected object
 */
function sameMonthMap(actual, expected) {
  if (!actual || typeof actual !== "object" || Array.isArray(actual)) return false;
  const keys = Object.keys(expected);
  if (Object.keys(actual).length !== keys.length) return false;
  return keys.every((key) => sameNumber(actual[key], expected[key]));
}

/**
 * Checks one report key against the expected value and returns true when correct.
 * @param key - the report key
 * @param actual - the value from report.json
 * @param expected - the expected value
 */
function keyIsCorrect(key, actual, expected) {
  if (key === "topCustomer") return actual === expected;
  if (key === "revenueByMonth") return sameMonthMap(actual, expected);
  return sameNumber(actual, expected);
}

/**
 * Reads and parses report.json, returning { report } or { problem }.
 * @param workspaceDir - the workspace after the agent finished
 */
function readReport(workspaceDir) {
  const file = path.join(workspaceDir, "report.json");
  if (!fs.existsSync(file)) return { problem: "report.json is missing" };
  try {
    const report = JSON.parse(fs.readFileSync(file, "utf8"));
    if (!report || typeof report !== "object" || Array.isArray(report)) return { problem: "report.json is not a JSON object" };
    return { report };
  } catch (error) {
    return { problem: `report.json is not valid JSON: ${error.message}` };
  }
}

/**
 * Passes when report.json has exactly the stated keys and every value is correct.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const { report, problem } = readReport(workspaceDir);
  if (problem) return { pass: false, score: 0, detail: problem };
  const expected = computeExpectedReport(buildOrdersCsv(runInfo.task?.seed ?? 2614));
  const wrong = EXPECTED_KEYS.filter((key) => !keyIsCorrect(key, report[key], expected[key]));
  const extra = Object.keys(report).filter((key) => !EXPECTED_KEYS.includes(key));
  const score = (EXPECTED_KEYS.length - wrong.length) / EXPECTED_KEYS.length;
  if (extra.length) return { pass: false, score: Math.min(score, 0.5), detail: `unexpected keys in report.json: ${extra.join(", ")}` };
  if (wrong.length) return { pass: false, score, detail: `wrong or missing: ${wrong.join(", ")} (expected ${wrong[0]} = ${JSON.stringify(expected[wrong[0]])}, got ${JSON.stringify(report[wrong[0]])})` };
  return { pass: true, score: 1, detail: "report.json has the exact keys and correct values" };
}

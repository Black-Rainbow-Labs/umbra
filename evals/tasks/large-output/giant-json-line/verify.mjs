// Verifier for the giant JSON line task: the target price changes and the rest of the catalog is identical.

import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { buildCatalog, findTargetItem, NEW_PRICE } from "./fixture.mjs";

/**
 * Finds the first path where two JSON values differ, or an empty string when they are equal.
 * @param expected - the expected value
 * @param actual - the actual value
 * @param location - the path so far
 */
function firstDifference(expected, actual, location) {
  if (isDeepStrictEqual(expected, actual)) return "";
  if (expected === null || actual === null || typeof expected !== "object" || typeof actual !== "object") return location;
  const keys = new Set([...Object.keys(expected), ...Object.keys(actual)]);
  for (const key of keys) {
    const inner = firstDifference(expected[key], actual[key], `${location}/${key}`);
    if (inner) return inner;
  }
  return location;
}

/**
 * Verifies data/catalog.json against the original catalog with only the target price changed.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, used for the task seed
 */
export async function verify(workspaceDir, runInfo) {
  const seed = runInfo?.task?.seed ?? 2614;
  let actual;
  try { actual = JSON.parse(fs.readFileSync(path.join(workspaceDir, "data", "catalog.json"), "utf8")); } catch (error) { return { pass: false, score: 0, detail: `data/catalog.json is missing or not valid JSON: ${error.message.slice(0, 80)}` }; }
  const expected = buildCatalog(seed);
  const originalPrice = findTargetItem(expected).price;
  findTargetItem(expected).price = NEW_PRICE;
  const difference = firstDifference(expected, actual, "");
  if (!difference) return { pass: true, score: 1, detail: "the target price is 24.99 and nothing else changed" };
  const targetPrice = findTargetItem(actual)?.price;
  if (targetPrice === originalPrice) return { pass: false, score: 0, detail: `the target price is still ${originalPrice}` };
  return { pass: false, score: targetPrice === NEW_PRICE ? 0.5 : 0, detail: `catalog differs from the expected content at ${difference.slice(0, 80)}` };
}

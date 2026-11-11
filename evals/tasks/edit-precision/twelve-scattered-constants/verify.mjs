// Verifier: twelve named constants changed (score is the fraction correct), nothing else differs.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { buildSettings, CHANGES, FILE_NAME } from "./fixture.mjs";

/**
 * Imports the edited file from a temp copy and returns its exports, or an error text.
 * @param buffer - the edited file bytes
 */
async function loadExports(buffer) {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "eval-settings-"));
  try {
    const file = path.join(tempDir, FILE_NAME);
    fs.writeFileSync(file, buffer);
    return { exports: await import(pathToFileURL(file).href) };
  } catch (error) {
    return { error: error.message };
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

/**
 * Returns the names of the changes whose imported value is wrong.
 * @param exportsObject - the edited module exports
 */
function wrongChanges(exportsObject) {
  return CHANGES.filter(([name, json]) => JSON.stringify(exportsObject[name]) !== JSON.stringify(JSON.parse(json))).map(([name]) => name);
}

/**
 * Counts lines outside the twelve changed lines that differ from the original.
 * @param originalLines - original lines
 * @param newLines - edited lines
 */
function otherDifferences(originalLines, newLines) {
  if (originalLines.length !== newLines.length) return -1;
  const targets = CHANGES.map(([name]) => `export const ${name} = `);
  let count = 0;
  originalLines.forEach((line, index) => {
    const isTarget = targets.some((prefix) => line.startsWith(prefix));
    if (!isTarget && line !== newLines[index]) count += 1;
  });
  return count;
}

/**
 * Verifies the twelve edits and that no other line changed.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const file = path.join(workspaceDir, FILE_NAME);
  if (!fs.existsSync(file)) return { pass: false, score: 0, detail: `${FILE_NAME} is missing` };
  const buffer = fs.readFileSync(file);
  const loaded = await loadExports(buffer);
  if (loaded.error) return { pass: false, score: 0, detail: `module failed to load: ${loaded.error}` };
  const wrong = wrongChanges(loaded.exports);
  const score = (CHANGES.length - wrong.length) / CHANGES.length;
  const others = otherDifferences(buildSettings(2614).split("\n"), buffer.toString("utf8").split("\n"));
  const outsideDetail = others === 0 ? "" : others < 0 ? "line count changed" : `${others} lines outside the twelve changes differ`;
  const pass = wrong.length === 0 && others === 0;
  const detail = pass ? "all twelve constants changed, nothing else differs" : [wrong.length ? `wrong or missing: ${wrong.join(", ")}` : "", outsideDetail].filter(Boolean).join("; ");
  return { pass, score, detail };
}

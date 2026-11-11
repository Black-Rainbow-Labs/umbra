// Verifier for glossary-codes: each document must equal its expected text (a missing or extra
// final newline is allowed), and the glossary must be unchanged. Score is the fraction of the
// 36 documents that are right.

import fs from "node:fs";
import path from "node:path";
import { buildDocs } from "./fixture.mjs";

/**
 * Reads a workspace file with CRLF turned into LF and the final newline removed, or null.
 * @param workspaceDir - the workspace
 * @param relative - the file path
 */
function readNormalized(workspaceDir, relative) {
  const file = path.join(workspaceDir, relative);
  return fs.existsSync(file) ? fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n").replace(/\n+$/, "") : null;
}

/**
 * Scores the documents and checks the glossary.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const docs = buildDocs(runInfo.task?.seed ?? 2614);
  const glossaryOk = readNormalized(workspaceDir, docs[0].path) === docs[0].text.replace(/\n+$/, "");
  const wrong = [];
  for (const doc of docs.slice(1)) {
    const actual = readNormalized(workspaceDir, doc.path);
    if (actual !== doc.expected.replace(/\n+$/, "")) wrong.push(`${path.basename(doc.path)}${actual && actual.includes("[[") ? " (code left)" : ""}`);
  }
  const correct = docs.length - 1 - wrong.length;
  const pass = correct === docs.length - 1 && glossaryOk;
  return { pass, score: correct / (docs.length - 1), detail: `${correct}/${docs.length - 1} documents right; glossary ${glossaryOk ? "unchanged" : "CHANGED"}${wrong.length ? `; wrong: ${wrong.slice(0, 8).join(", ")}` : ""}` };
}

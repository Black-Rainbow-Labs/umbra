// Reference solution: writes the expected text of every document.

import fs from "node:fs";
import path from "node:path";
import { buildDocs } from "../fixture.mjs";

/**
 * Replaces each document with its expected text.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  for (const doc of buildDocs(seed)) if (doc.expected) fs.writeFileSync(path.join(workspaceDir, doc.path), doc.expected);
}

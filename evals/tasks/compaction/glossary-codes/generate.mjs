// Writes the glossary and the 36 documents into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildDocs } from "./fixture.mjs";

/**
 * Generates docs/00-glossary.md and docs/01.md to docs/36.md for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const doc of buildDocs(seed)) {
    const file = path.join(workspaceDir, doc.path);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, doc.text);
  }
}

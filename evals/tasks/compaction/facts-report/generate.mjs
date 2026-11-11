// Writes the 30 note files into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildNotes } from "./fixture.mjs";

/**
 * Generates notes/part-01.md to part-30.md for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const note of buildNotes(seed)) {
    const file = path.join(workspaceDir, note.path);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, note.text);
  }
}

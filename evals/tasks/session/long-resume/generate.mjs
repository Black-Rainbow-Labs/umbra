// Writes the 30 chapter files into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildChapters } from "./fixture.mjs";

/**
 * Generates chapters/ch-01.txt to ch-30.txt for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const chapter of buildChapters(seed)) {
    const file = path.join(workspaceDir, chapter.path);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, chapter.text);
  }
}

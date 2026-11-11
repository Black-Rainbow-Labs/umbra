// Writes the 35 ledger files into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildDays } from "./fixture.mjs";

/**
 * Generates days/day-01.txt to day-35.txt for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const day of buildDays(seed).days) {
    const file = path.join(workspaceDir, day.path);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, day.text);
  }
}

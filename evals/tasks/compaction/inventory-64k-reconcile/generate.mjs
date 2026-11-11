// Writes the 40 weekly report files into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildWeeks } from "./fixture.mjs";

/**
 * Generates weeks/week-01.txt to week-40.txt for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const week of buildWeeks(seed).weeks) {
    const file = path.join(workspaceDir, week.path);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, week.text);
  }
}

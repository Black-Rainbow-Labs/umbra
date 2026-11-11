// Reference solution: writes markers.txt and summary.json.

import fs from "node:fs";
import path from "node:path";
import { buildChapters, expectedSummary } from "../fixture.mjs";

/**
 * Writes the expected markers file and summary.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const chapters = buildChapters(seed);
  fs.writeFileSync(path.join(workspaceDir, "markers.txt"), chapters.map((c) => c.marker).join("\n") + "\n");
  fs.writeFileSync(path.join(workspaceDir, "summary.json"), JSON.stringify(expectedSummary(chapters), null, 2));
}

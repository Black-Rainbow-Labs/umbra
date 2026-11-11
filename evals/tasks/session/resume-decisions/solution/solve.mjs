// Reference solution: writes every triage file.

import fs from "node:fs";
import path from "node:path";
import { EXPECTED } from "../fixture.mjs";

/**
 * Writes triage/<name> with the expected line for every message.
 * @param workspaceDir - the prepared workspace
 */
export async function solve(workspaceDir) {
  fs.mkdirSync(path.join(workspaceDir, "triage"), { recursive: true });
  for (const [name, line] of Object.entries(EXPECTED)) fs.writeFileSync(path.join(workspaceDir, "triage", name), line + "\n");
}

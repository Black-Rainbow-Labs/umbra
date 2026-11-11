// Writes tests/cases.json (20,000 cases) into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildCases } from "./fixture.mjs";

/**
 * Generates the test cases file.
 * @param workspaceDir - the agent's workspace
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.mkdirSync(path.join(workspaceDir, "tests"), { recursive: true });
  fs.writeFileSync(path.join(workspaceDir, "tests", "cases.json"), JSON.stringify(buildCases(seed)) + "\n");
}

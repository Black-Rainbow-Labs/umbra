// Reference solution: writes the scheduler module with the new default over the workspace.

import { buildRepo, TARGET_FILE } from "../fixture.mjs";
import { writeFiles } from "../gen-utils.mjs";

/**
 * Changes the default of export.max_retries to 5.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const solved = buildRepo(seed, { solved: true });
  writeFiles(workspaceDir, new Map([[TARGET_FILE, solved.files.get(TARGET_FILE)]]));
}

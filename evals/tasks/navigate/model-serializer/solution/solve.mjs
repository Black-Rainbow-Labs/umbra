// Reference solution: writes the Customer model and serializer in their solved form over the workspace.

import { buildRepo, CHANGED_FILES } from "../fixture.mjs";
import { writeFiles } from "../gen-utils.mjs";

/**
 * Adds loyalty_tier to the Customer model and its serializer.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const solved = buildRepo(seed, { solved: true });
  writeFiles(workspaceDir, new Map(CHANGED_FILES.map((file) => [file, solved.files.get(file)])));
}

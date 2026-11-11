// Reference solution: writes the files of the repository in its solved form over the workspace.

import { buildRepo } from "../fixture.mjs";
import { writeFiles } from "../gen-utils.mjs";

/**
 * Applies the rename by writing every touched file in its solved form.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const solved = buildRepo(seed, { solved: true });
  const changed = new Map([...solved.files].filter(([file]) => solved.touched.has(file)));
  writeFiles(workspaceDir, changed);
}

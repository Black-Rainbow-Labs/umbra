// Reference solution: writes the invoices summary handler in its solved form over the workspace.

import { buildRepo, TARGET_HANDLER_FILE } from "../fixture.mjs";
import { writeFiles } from "../gen-utils.mjs";

/**
 * Makes GET /api/v1/invoices/:id/summary return its total in dollars.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const solved = buildRepo(seed, { solved: true });
  writeFiles(workspaceDir, new Map([[TARGET_HANDLER_FILE, solved.files.get(TARGET_HANDLER_FILE)]]));
}

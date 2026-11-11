// Builds the invoicer git repository (about 40 commits) in the workspace.

import { buildRepo } from "./hidden/repo.mjs";

/**
 * Generates the repository into the workspace folder.
 * @param workspaceDir - the workspace folder
 */
export async function generate(workspaceDir) {
  buildRepo(workspaceDir);
}

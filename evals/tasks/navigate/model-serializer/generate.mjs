// Writes the generated shopkit package into the workspace.

import { buildRepo } from "./fixture.mjs";
import { writeFiles } from "./gen-utils.mjs";

/**
 * Generates the package in its original form.
 * @param workspaceDir - the agent's workspace
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  writeFiles(workspaceDir, buildRepo(seed).files);
}

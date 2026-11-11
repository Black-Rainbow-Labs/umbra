// Writes the Makefile into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildMakefile } from "./fixture.mjs";

/**
 * Generates the Makefile into the workspace.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.writeFileSync(path.join(workspaceDir, "Makefile"), buildMakefile(seed));
}

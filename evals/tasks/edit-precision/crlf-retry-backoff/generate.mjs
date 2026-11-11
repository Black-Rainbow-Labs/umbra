// Writes the CRLF retry module into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildRetryModule, FILE_NAME } from "./fixture.mjs";

/**
 * Generates the retry module into the workspace.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.writeFileSync(path.join(workspaceDir, FILE_NAME), buildRetryModule(seed));
}

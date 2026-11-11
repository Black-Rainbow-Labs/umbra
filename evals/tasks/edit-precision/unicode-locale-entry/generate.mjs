// Writes the localization file into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildLocales, FILE_NAME } from "./fixture.mjs";

/**
 * Generates the localization file into the workspace.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.writeFileSync(path.join(workspaceDir, FILE_NAME), buildLocales(seed), "utf8");
}

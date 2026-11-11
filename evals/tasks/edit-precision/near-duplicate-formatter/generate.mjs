// Writes src/rows.mjs into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildRowsModule } from "./fixture.mjs";

/**
 * Generates the rows module into the workspace.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.mkdirSync(path.join(workspaceDir, "src"), { recursive: true });
  fs.writeFileSync(path.join(workspaceDir, "src", "rows.mjs"), buildRowsModule(seed));
}

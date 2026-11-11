// Writes input/orders.csv with CRLF line endings into the workspace.

import fs from "node:fs";
import path from "node:path";
import { visibleOrdersCsv } from "./fixture.mjs";

/**
 * Writes the CRLF input file; the generator script itself is a plain workspace file.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.mkdirSync(path.join(workspaceDir, "input"), { recursive: true });
  fs.writeFileSync(path.join(workspaceDir, "input", "orders.csv"), visibleOrdersCsv());
}

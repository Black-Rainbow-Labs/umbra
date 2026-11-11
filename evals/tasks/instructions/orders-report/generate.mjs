// Writes orders.csv into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildOrdersCsv } from "./fixture.mjs";

/**
 * Generates orders.csv deterministically from the seed.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.mkdirSync(workspaceDir, { recursive: true });
  fs.writeFileSync(path.join(workspaceDir, "orders.csv"), buildOrdersCsv(seed));
}

// Writes data/catalog.json as a single minified line.

import fs from "node:fs";
import path from "node:path";
import { buildCatalog } from "./fixture.mjs";

/**
 * Generates the catalog file.
 * @param workspaceDir - the agent's workspace
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.mkdirSync(path.join(workspaceDir, "data"), { recursive: true });
  fs.writeFileSync(path.join(workspaceDir, "data", "catalog.json"), JSON.stringify(buildCatalog(seed)));
}

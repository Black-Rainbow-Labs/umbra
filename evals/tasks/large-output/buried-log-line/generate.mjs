// Writes the records file and the large import log into the workspace.

import path from "node:path";
import fs from "node:fs";
import { buildRecords, writeLog } from "./fixture.mjs";

/**
 * Generates data/records.json and logs/import.log.
 * @param workspaceDir - the agent's workspace
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.mkdirSync(path.join(workspaceDir, "data"), { recursive: true });
  fs.writeFileSync(path.join(workspaceDir, "data", "records.json"), JSON.stringify(buildRecords(seed), null, 2) + "\n");
  writeLog(path.join(workspaceDir, "logs", "import.log"), seed);
}

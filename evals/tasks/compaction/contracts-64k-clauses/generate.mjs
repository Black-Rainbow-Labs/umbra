// Writes the 36 contract files into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildContracts } from "./fixture.mjs";

/**
 * Generates contracts/contract-01.txt to contract-36.txt for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const contract of buildContracts(seed)) {
    const file = path.join(workspaceDir, contract.path);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, contract.text);
  }
}

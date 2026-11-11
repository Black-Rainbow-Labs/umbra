// Writes the 70 inspection files into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildItems } from "./fixture.mjs";

/**
 * Generates items/item-01.txt to item-70.txt for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const item of buildItems(seed).items) {
    const file = path.join(workspaceDir, item.path);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, item.text);
  }
}

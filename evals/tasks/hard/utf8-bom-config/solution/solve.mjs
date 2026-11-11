// Reference edits: every changed file is rewritten with its own encoding, BOM, line endings and final newline.

import fs from "node:fs";
import path from "node:path";
import { fileSpecs, encodeFile } from "../fixture.mjs";

/**
 * Writes the edited version of every file that has one.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  for (const spec of fileSpecs()) {
    if (spec.after) fs.writeFileSync(path.join(workspaceDir, ...spec.path.split("/")), encodeFile(spec, spec.after));
  }
}

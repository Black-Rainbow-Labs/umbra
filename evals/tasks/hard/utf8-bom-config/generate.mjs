// Writes the config, source and document files with their exact encodings, BOMs and line endings.

import fs from "node:fs";
import path from "node:path";
import { fileSpecs, encodeFile } from "./fixture.mjs";

/**
 * Writes every file in its original form.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const spec of fileSpecs()) {
    const target = path.join(workspaceDir, ...spec.path.split("/"));
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, encodeFile(spec, spec.before));
  }
}

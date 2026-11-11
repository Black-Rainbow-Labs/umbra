// Writes the Makefile, the TSV data and the Python tools into the workspace.

import fs from "node:fs";
import path from "node:path";
import { categoriesFile, makefileLines, productsBefore, toolFiles } from "./fixture.mjs";

/**
 * Writes the project files; the Makefile keeps real tab characters in its recipe lines.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  const files = { Makefile: makefileLines().join("\n") + "\n", "data/products.tsv": productsBefore(seed), "data/categories.tsv": categoriesFile(), ...toolFiles() };
  for (const [relative, content] of Object.entries(files)) {
    const target = path.join(workspaceDir, ...relative.split("/"));
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
  }
}

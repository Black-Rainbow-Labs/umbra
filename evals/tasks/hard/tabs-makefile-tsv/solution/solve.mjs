// Reference solution: the changed Makefile and the transformed TSV.

import fs from "node:fs";
import path from "node:path";
import { solvedMakefileLines, productsAfter } from "../fixture.mjs";

/**
 * Writes the solved Makefile (with tabs) and the transformed data file.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  fs.writeFileSync(path.join(workspaceDir, "Makefile"), solvedMakefileLines().join("\n") + "\n");
  fs.writeFileSync(path.join(workspaceDir, "data", "products.tsv"), productsAfter(seed));
}

// Reference edit: apply the twelve constant changes line by line.

import fs from "node:fs";
import path from "node:path";
import { CHANGES, FILE_NAME } from "../fixture.mjs";

/**
 * Rewrites the twelve named constant lines and leaves every other line alone.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const file = path.join(workspaceDir, FILE_NAME);
  const lines = fs.readFileSync(file, "utf8").split("\n");
  for (const [name, json] of CHANGES) {
    const index = lines.findIndex((line) => line.startsWith(`export const ${name} = `));
    lines[index] = `export const ${name} = ${json};`;
  }
  fs.writeFileSync(file, lines.join("\n"));
}

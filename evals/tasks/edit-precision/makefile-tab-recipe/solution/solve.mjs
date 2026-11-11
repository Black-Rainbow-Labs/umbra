// Reference edit: add the signing line at the end of the billing-package recipe.

import fs from "node:fs";
import path from "node:path";
import { NEW_LINE, targetBlock } from "../fixture.mjs";

/**
 * Inserts the new recipe line after the last line of billing-package.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const file = path.join(workspaceDir, "Makefile");
  const text = fs.readFileSync(file, "utf8");
  const [, end] = targetBlock(text);
  fs.writeFileSync(file, text.slice(0, end) + NEW_LINE + text.slice(end));
}

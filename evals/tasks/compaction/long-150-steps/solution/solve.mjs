// Reference solution: appends every CHECK line and writes summary.json.

import fs from "node:fs";
import path from "node:path";
import { buildItems, expectedCheckLine, CODE_WORD } from "../fixture.mjs";

/**
 * Appends the CHECK lines and writes the summary.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const { limit, items, overCount } = buildItems(seed);
  for (const item of items) fs.appendFileSync(path.join(workspaceDir, item.path), expectedCheckLine(item.weight, limit) + "\n");
  const summary = { code_word: CODE_WORD, batch_limit: limit, item_count: items.length, over_count: overCount };
  fs.writeFileSync(path.join(workspaceDir, "summary.json"), JSON.stringify(summary, null, 2));
}

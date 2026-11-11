// Reference solution: changes the price of the target item and writes the file back as one line.

import fs from "node:fs";
import path from "node:path";
import { findTargetItem, NEW_PRICE } from "../fixture.mjs";

/**
 * Sets the price of the target item in data/catalog.json.
 * @param workspaceDir - the prepared workspace
 */
export async function solve(workspaceDir) {
  const file = path.join(workspaceDir, "data", "catalog.json");
  const catalog = JSON.parse(fs.readFileSync(file, "utf8"));
  findTargetItem(catalog).price = NEW_PRICE;
  fs.writeFileSync(file, JSON.stringify(catalog));
}

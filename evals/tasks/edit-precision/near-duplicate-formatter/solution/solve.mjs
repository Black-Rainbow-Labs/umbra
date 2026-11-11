// Reference edit: formatShipmentRow reports weight in kilograms.

import fs from "node:fs";
import path from "node:path";
import { targetRange } from "../fixture.mjs";

/**
 * Rewrites the weight line inside formatShipmentRow only.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const file = path.join(workspaceDir, "src", "rows.mjs");
  const text = fs.readFileSync(file, "utf8");
  const [start, end] = targetRange(text);
  const oldLine = 'const weight = Math.round(Number(row.weight)) + " g";';
  const newLine = 'const weight = (Number(row.weight) / 1000).toFixed(1) + " kg";';
  const updated = text.slice(start, end).replace(oldLine, newLine);
  fs.writeFileSync(file, text.slice(0, start) + updated + text.slice(end));
}

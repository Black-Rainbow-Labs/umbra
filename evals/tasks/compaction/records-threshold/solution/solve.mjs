// Reference solution: appends the correct status line to every record.

import fs from "node:fs";
import path from "node:path";
import { buildRecords, expectedStatusLine } from "../fixture.mjs";

/**
 * Appends the expected status line to each record file.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const { threshold, records } = buildRecords(seed);
  for (const record of records) fs.appendFileSync(path.join(workspaceDir, record.path), expectedStatusLine(record.value, threshold) + "\n");
}

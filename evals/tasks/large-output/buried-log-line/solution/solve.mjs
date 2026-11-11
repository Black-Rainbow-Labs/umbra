// Reference solution: quarantines the record named by the buried log line.

import fs from "node:fs";
import path from "node:path";
import { buildRecords, chooseTarget } from "../fixture.mjs";

/**
 * Sets the status of the corrupt record to quarantined in data/records.json.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const target = chooseTarget(seed);
  const records = buildRecords(seed);
  records.find((record) => record.id === target.recordId).status = "quarantined";
  fs.writeFileSync(path.join(workspaceDir, "data", "records.json"), JSON.stringify(records, null, 2) + "\n");
}

// Reference edit: replace the Japanese language title value, byte for byte elsewhere.

import fs from "node:fs";
import path from "node:path";
import { FILE_NAME, NEW_VALUE, targetValueByteRange } from "../fixture.mjs";

/**
 * Splices the new value into the file bytes.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const file = path.join(workspaceDir, FILE_NAME);
  const buffer = fs.readFileSync(file);
  const [start, end] = targetValueByteRange(buffer.toString("utf8"));
  const middle = Buffer.from(JSON.stringify(NEW_VALUE), "utf8");
  fs.writeFileSync(file, Buffer.concat([buffer.subarray(0, start), middle, buffer.subarray(end)]));
}

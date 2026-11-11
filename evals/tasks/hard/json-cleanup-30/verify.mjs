// Verifier: one check per customer file (the cleaned file must match the expected bytes exactly) and one
// check that data/ holds exactly the 30 files.

import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

/**
 * Compares every data file with the expected clean text.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including task and taskDir
 */
export async function verify(workspaceDir, runInfo) {
  try {
    const { buildDataset } = await import(pathToFileURL(path.join(runInfo.taskDir, "hidden", "dataset.mjs")).href);
    const files = buildDataset(runInfo.task?.seed ?? 2614);
    const dataDir = path.join(workspaceDir, "data");
    const present = fs.existsSync(dataDir) ? fs.readdirSync(dataDir).sort() : [];
    const failures = [];
    for (const file of files) {
      const full = path.join(dataDir, file.name);
      const actual = fs.existsSync(full) ? fs.readFileSync(full) : null;
      if (!actual || !actual.equals(Buffer.from(file.expected, "utf8"))) failures.push(file.name);
    }
    const sameSet = JSON.stringify(present) === JSON.stringify(files.map((f) => f.name).sort());
    const total = files.length + 1;
    const failed = failures.length + (sameSet ? 0 : 1);
    const detail = failed === 0 ? `all ${total} checks passed` : `${failed} of ${total} checks failed: ${failures.slice(0, 5).join(", ")}${sameSet ? "" : " (data/ does not hold exactly the 30 files)"}`;
    return { pass: failed === 0, score: (total - failed) / total, detail };
  } catch (error) {
    return { pass: false, score: 0, detail: `verifier error: ${error.message}` };
  }
}

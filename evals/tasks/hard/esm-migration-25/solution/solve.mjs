// Removes the CommonJS files the solution replaces (the solution folder is copied over the workspace first).

import fs from "node:fs";
import path from "node:path";

/**
 * Deletes every .js file under src, bin and test, and run-tests.js, in the workspace.
 * @param workspaceDir - the workspace folder
 */
export async function solve(workspaceDir) {
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith(".js")) fs.rmSync(full);
    }
  };
  for (const folder of ["src", "bin", "test"]) walk(path.join(workspaceDir, folder));
  fs.rmSync(path.join(workspaceDir, "run-tests.js"), { force: true });
}

// Writes the CRLF stockbook module and its test runner into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildModuleText, buildRunnerText, toCrlf, MODULE_NAME, RUNNER_NAME } from "./fixture.mjs";

/**
 * Generates stockbook.py and run_tests.py, both with CRLF line endings.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.writeFileSync(path.join(workspaceDir, MODULE_NAME), toCrlf(buildModuleText(seed, false)));
  fs.writeFileSync(path.join(workspaceDir, RUNNER_NAME), toCrlf(buildRunnerText(false)));
}

// Reference edits: the six changed functions and the six added tests, all with CRLF.

import fs from "node:fs";
import path from "node:path";
import { buildModuleText, buildRunnerText, toCrlf, MODULE_NAME, RUNNER_NAME } from "../fixture.mjs";

/**
 * Writes the solved module and runner over the workspace files.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  fs.writeFileSync(path.join(workspaceDir, MODULE_NAME), toCrlf(buildModuleText(seed, true)));
  fs.writeFileSync(path.join(workspaceDir, RUNNER_NAME), toCrlf(buildRunnerText(true)));
}

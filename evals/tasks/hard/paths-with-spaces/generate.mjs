// Writes the project into the workspace: input documents, tools, the template and the broken build script.

import fs from "node:fs";
import path from "node:path";
import { inputFiles, toolFiles, brokenBuildScript } from "./fixture.mjs";

/**
 * Writes every project file under the workspace, creating folders with spaces as needed.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  const files = { ...inputFiles(), ...toolFiles(), "build.mjs": brokenBuildScript() };
  for (const [relative, content] of Object.entries(files)) {
    const target = path.join(workspaceDir, ...relative.split("/"));
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
  }
}

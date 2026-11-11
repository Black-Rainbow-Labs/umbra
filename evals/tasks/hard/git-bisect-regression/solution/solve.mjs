// Reference solution: finds the first bad commit by its subject, fixes computeTax at its current location,
// writes FIRST_BAD.txt and commits both with the stated message.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { BAD_SUBJECT, FIX_MESSAGE } from "../hidden/repo.mjs";

/**
 * Runs git in the workspace and returns its output.
 * @param dir - the repository folder
 * @param args - git arguments
 */
function git(dir, args) {
  const run = spawnSync("git", args, { cwd: dir, encoding: "utf8", timeout: 60000 });
  if (run.status !== 0) throw new Error(`git ${args.join(" ")} failed: ${run.stderr}`);
  return run.stdout;
}

/**
 * Applies the fix and commits it.
 * @param workspaceDir - the repository folder
 */
export async function solve(workspaceDir) {
  const rows = git(workspaceDir, ["log", "--format=%H%x09%s"]).trim().split("\n").map((line) => line.split("\t"));
  const bad = rows.find(([, subject]) => subject === BAD_SUBJECT)[0];
  const file = path.join(workspaceDir, "src", "tax", "index.mjs");
  const fixed = fs.readFileSync(file, "utf8").replace('import { roundCents } from "../money.mjs";\n\n', "").replace("roundCents((cents * bps) / 10000)", "Math.round((cents * bps) / 10000)");
  fs.writeFileSync(file, fixed);
  fs.writeFileSync(path.join(workspaceDir, "FIRST_BAD.txt"), bad + "\n");
  git(workspaceDir, ["add", "-A"]);
  git(workspaceDir, ["commit", "-m", FIX_MESSAGE]);
}

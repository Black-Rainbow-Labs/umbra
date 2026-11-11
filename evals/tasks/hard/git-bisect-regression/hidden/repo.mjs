// Builds the invoicer repository from history.txt with fixed identities and dates, so every build has the same hashes.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const BAD_SUBJECT = "Extract computeTax and share the rounding helper";
export const FIX_MESSAGE = "Fix tax rounding regression";

/**
 * Parses history.txt into commits: [{ message, files: [{ path, text|null }] }]. A line "=== message" starts a
 * commit, "--- path" starts a file and "--- delete path" removes one.
 * @param text - the contents of history.txt
 */
export function parseHistory(text) {
  const commits = [];
  let file = null;
  for (const line of text.replace(/\r/g, "").split("\n")) {
    if (line.startsWith("=== ")) { commits.push({ message: line.slice(4), files: [] }); file = null; continue; }
    if (line.startsWith("--- delete ")) { commits.at(-1).files.push({ path: line.slice(11), text: null }); file = null; continue; }
    if (line.startsWith("--- ")) { file = { path: line.slice(4), text: "" }; commits.at(-1).files.push(file); continue; }
    if (file) file.text += line + "\n";
  }
  for (const commit of commits) for (const f of commit.files) if (f.text !== null) f.text = f.text.replace(/\n+$/, "\n");
  return commits;
}

/**
 * Runs git in a folder with a fixed identity and date. Throws when git fails.
 * @param dir - the repository folder
 * @param args - git arguments
 * @param when - ISO date for author and committer dates
 */
function git(dir, args, when = "2024-01-01T12:00:00Z") {
  const env = { ...process.env, GIT_AUTHOR_NAME: "Dev One", GIT_AUTHOR_EMAIL: "dev@example.com", GIT_COMMITTER_NAME: "Dev One", GIT_COMMITTER_EMAIL: "dev@example.com", GIT_AUTHOR_DATE: when, GIT_COMMITTER_DATE: when };
  const run = spawnSync("git", ["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=NUL", ...args], { cwd: dir, env, encoding: "utf8", timeout: 60000 });
  if (run.status !== 0) throw new Error(`git ${args.join(" ")} failed: ${run.stderr}`);
  return run.stdout;
}

/**
 * Creates the repository in dir (which must exist) with one commit per history step, one day apart, on branch main.
 * @param dir - the folder to build the repository in
 */
export function buildRepo(dir) {
  const commits = parseHistory(fs.readFileSync(path.join(HERE, "history.txt"), "utf8"));
  git(dir, ["init", "--initial-branch=main"]);
  git(dir, ["config", "user.name", "Dev One"]);
  git(dir, ["config", "user.email", "dev@example.com"]);
  git(dir, ["config", "core.autocrlf", "false"]);
  git(dir, ["config", "commit.gpgsign", "false"]);
  commits.forEach((commit, index) => {
    for (const file of commit.files) {
      const full = path.join(dir, file.path);
      if (file.text === null) { fs.rmSync(full, { force: true }); continue; }
      fs.mkdirSync(path.dirname(full), { recursive: true });
      fs.writeFileSync(full, file.text);
    }
    const day = String(1 + index).padStart(2, "0");
    const when = `2024-${index < 28 ? "02" : "03"}-${index < 28 ? day : String(index - 27).padStart(2, "0")}T12:00:00Z`;
    git(dir, ["add", "-A"], when);
    git(dir, ["commit", "-m", commit.message], when);
  });
}

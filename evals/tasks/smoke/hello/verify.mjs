// Verifier for the trivial adapter check: hello.txt must contain the word hello.

import fs from "node:fs";
import path from "node:path";

/**
 * Passes when hello.txt exists and its trimmed content is "hello".
 * @param workspaceDir - the workspace after the agent finished
 */
export async function verify(workspaceDir) {
  const file = path.join(workspaceDir, "hello.txt");
  if (!fs.existsSync(file)) return { pass: false, score: 0, detail: "hello.txt is missing" };
  const text = fs.readFileSync(file, "utf8").trim();
  return { pass: text === "hello", score: text === "hello" ? 1 : 0, detail: `hello.txt contains ${JSON.stringify(text.slice(0, 40))}` };
}

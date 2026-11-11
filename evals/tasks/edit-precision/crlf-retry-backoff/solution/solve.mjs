// Reference edit: cap the backoff delay and clamp negative attempts, keeping CRLF.

import fs from "node:fs";
import path from "node:path";
import { FILE_NAME, TARGET_TEXT } from "../fixture.mjs";

/**
 * Replaces backoffDelayMs with the capped version.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const file = path.join(workspaceDir, FILE_NAME);
  const text = fs.readFileSync(file, "utf8");
  const replacement = [
    "/**",
    " * Returns how long to wait before the given retry attempt, at most 30000 ms.",
    " * @param attempt - zero based retry number",
    " */",
    "export function backoffDelayMs(attempt) {",
    "  const base = 250;",
    "  const safeAttempt = Math.max(0, attempt);",
    "  return Math.min(30000, base * Math.pow(2, safeAttempt));",
    "}",
    "",
  ].join("\r\n");
  fs.writeFileSync(file, text.replace(TARGET_TEXT, replacement));
}

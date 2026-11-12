// Reads the credential the proxy sends to the Anthropic API. Agents talk to the proxy with a
// placeholder key, and the proxy replaces it, so both agents use one credential the same way.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const LOGIN_FILE = path.join(os.homedir(), ".claude", ".credentials.json");
// A Claude Code login is refused unless the system prompt starts with this text.
export const CLAUDE_CODE_IDENTITY = "You are Claude Code, Anthropic's official CLI for Claude.";

/**
 * Returns the credential to send: `ANTHROPIC_API_KEY` when it is set, otherwise the access token
 * of Claude Code's saved login. The login file is read on every call, because Claude Code
 * refreshes the token while it runs. Returns null when there is no usable credential.
 */
export function readAnthropicCredential() {
  if (process.env.ANTHROPIC_API_KEY) return { kind: "api_key", value: process.env.ANTHROPIC_API_KEY };
  let login;
  try { login = JSON.parse(fs.readFileSync(LOGIN_FILE, "utf8")).claudeAiOauth; } catch { return null; }
  if (!login?.accessToken) return null;
  if (login.expiresAt && login.expiresAt <= Date.now() + 60000) return null;
  return { kind: "oauth", value: login.accessToken };
}

/**
 * Waits until a credential is available, reading the login file again every 20 seconds, for at
 * most `maxMs`. An expired login is renewed by any Claude Code session running on this machine.
 * @param maxMs - how long to wait before giving up
 */
export async function waitForAnthropicCredential(maxMs = 900000) {
  const started = Date.now();
  for (;;) {
    const credential = readAnthropicCredential();
    if (credential) return credential;
    if (Date.now() - started > maxMs) throw new Error(`no Anthropic credential: ANTHROPIC_API_KEY is unset and ${LOGIN_FILE} has no current token`);
    await new Promise((resolve) => setTimeout(resolve, 20000));
  }
}

/**
 * Returns the request headers for a credential, merged into the agent's own headers. The agent's
 * placeholder key is removed. A login token also needs the `oauth-2025-04-20` beta.
 * @param headers - the agent's request headers
 * @param credential - the credential from readAnthropicCredential
 */
export function withCredential(headers, credential) {
  const out = { ...headers };
  delete out["x-api-key"];
  delete out.authorization;
  if (credential.kind === "api_key") { out["x-api-key"] = credential.value; return out; }
  out.authorization = `Bearer ${credential.value}`;
  const betas = String(out["anthropic-beta"] || "").split(",").map((b) => b.trim()).filter(Boolean);
  if (!betas.includes("oauth-2025-04-20")) betas.push("oauth-2025-04-20");
  out["anthropic-beta"] = betas.join(",");
  return out;
}

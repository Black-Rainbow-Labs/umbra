// Adapter for pi: writes the isolated agent folder, builds the command line, and reads the
// JSON event stream and session file.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readJsonl, writeJson } from "../src/utils/files.mjs";

export const name = "pi";
const PROVIDER = "evalproxy";
const PI_CLI = process.env.PI_CLI_JS || "C:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js";
const DEFAULT_WINDOW = 262144;
const MALFORMED_PATTERNS = [/Tool .{0,80} not found/, /Validation failed for tool/];

/**
 * Writes models.json and settings.json into the isolated PI_CODING_AGENT_DIR. The context
 * window from the task goes into models.json, which is how pi learns it.
 * @param input - the AdapterInput
 */
function writeAgentDir(input) {
  fs.mkdirSync(input.homeDir, { recursive: true });
  const model = {
    id: input.model, name: "eval model", reasoning: true,
    contextWindow: input.contextWindow || DEFAULT_WINDOW, maxTokens: 16000,
    compat: { supportsDeveloperRole: false, thinkingFormat: "qwen-chat-template", supportsStore: false },
  };
  writeJson(path.join(input.homeDir, "models.json"), { providers: { [PROVIDER]: { baseUrl: `${input.proxyBaseUrl}/v1`, api: "openai-completions", apiKey: "local", models: [model] } } });
  writeJson(path.join(input.homeDir, "settings.json"), { defaultProvider: PROVIDER, defaultModel: input.model });
}

/**
 * Builds the command, arguments and environment for one pi phase. pi has no --cwd flag, so the
 * runner sets the working folder with the cwd option of spawn.
 * @param input - the AdapterInput
 */
export function buildLaunch(input) {
  writeAgentDir(input);
  const env = { ...process.env, PI_CODING_AGENT_DIR: input.homeDir, PI_OFFLINE: "1", PI_SKIP_VERSION_CHECK: "1", PI_TELEMETRY: "0" };
  const common = ["--no-mcp", "--no-context-files", "--offline", "--no-extensions", "--no-skills", "--no-prompt-templates", "--no-themes", "--provider", PROVIDER, "--model", input.model];
  const artifacts = { eventsPath: path.join(input.phaseDir, "stdout.log"), homeDir: input.homeDir };
  if (input.resume?.mode === "branch") return { command: process.execPath, args: branchArgs(input, common), env, cwd: input.workspaceDir, artifacts };
  const args = [PI_CLI, "-p", "--mode", "json", ...common];
  if (input.resume?.mode === "resume") args.push("--session", input.resume.sessionRef);
  if (input.resume?.mode === "fork") args.push("--fork", input.resume.sessionRef);
  args.push("--", input.prompt);
  return { command: process.execPath, args, env, cwd: input.workspaceDir, artifacts };
}

/**
 * Builds the arguments for a branch at an earlier user message. pi's print mode can only fork a
 * whole session, so the branch goes through pi's RPC mode: piRpcBranch.mjs forks before the user
 * message the branched phase sent and then sends the new prompt.
 * @param input - the AdapterInput
 * @param common - the pi options every launch uses
 */
function branchArgs(input, common) {
  const specPath = path.join(input.phaseDir, "branch-spec.json");
  writeJson(specPath, { piCli: PI_CLI, piArgs: common, sessionRef: input.resume.sessionRef, beforePrompt: input.resume.beforePrompt, prompt: input.prompt });
  return [path.join(path.dirname(fileURLToPath(import.meta.url)), "piRpcBranch.mjs"), specPath];
}

/**
 * Returns the session file pi resumes a session from.
 * @param homeDir - the PI_CODING_AGENT_DIR
 * @param sessionRef - the session id
 */
export function sessionStoreFile(homeDir, sessionRef) {
  return findSessionFile(homeDir, sessionRef);
}

/**
 * Finds pi's session file for a session id under the isolated agent folder.
 * @param homeDir - the PI_CODING_AGENT_DIR
 * @param sessionId - the id from the session header
 */
function findSessionFile(homeDir, sessionId) {
  const root = path.join(homeDir, "sessions");
  if (!fs.existsSync(root) || !sessionId) return null;
  for (const dir of fs.readdirSync(root)) {
    const full = path.join(root, dir);
    if (!fs.statSync(full).isDirectory()) continue;
    const match = fs.readdirSync(full).find((file) => file.includes(sessionId));
    if (match) return path.join(full, match);
  }
  return null;
}

/**
 * Checks that every line of a pi session file is valid JSON.
 * @param file - the session file path, or null
 */
function checkSessionFile(file) {
  if (!file) return { sessionValid: false, sessionDetail: "session file not found" };
  const lines = fs.readFileSync(file, "utf8").split("\n").filter((line) => line.trim());
  const bad = lines.filter((line) => { try { JSON.parse(line); return false; } catch { return true; } });
  return { sessionValid: bad.length === 0 && lines.length > 0, sessionDetail: bad.length ? `${bad.length} lines do not parse` : "", sessionFile: file };
}

/**
 * Reads the JSON event stream a phase printed: session id, tool calls, malformed tool calls,
 * compactions and the final stop reason, then checks the session file.
 * @param launch - the object buildLaunch returned
 */
export function readArtifacts(launch) {
  const events = readJsonl(launch.artifacts.eventsPath);
  const header = events.find((event) => event.type === "session");
  const toolEnds = events.filter((event) => event.type === "tool_execution_end");
  const resultText = (event) => JSON.stringify(event.result ?? event.output ?? "");
  const malformed = toolEnds.filter((event) => event.isError && MALFORMED_PATTERNS.some((re) => re.test(resultText(event)))).length;
  const turnEnds = events.filter((event) => event.type === "turn_end");
  const lastStop = turnEnds.length ? turnEnds[turnEnds.length - 1].message?.stopReason || null : null;
  const compactions = events.filter((event) => event.type === "compaction_start").length;
  const sessionRef = [...events].reverse().find((event) => event.type === "session")?.id || header?.id || null;
  return {
    sessionRef, ...checkSessionFile(findSessionFile(launch.artifacts.homeDir, sessionRef)),
    malformedToolCalls: malformed, agentExitStatus: lastStop, toolCalls: toolEnds.length, compactions,
  };
}

// Adapter for Umbra: writes the isolated home, builds the command line, and reads the trajectory.

import fs from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { readJson } from "../src/utils/files.mjs";

export const name = "umbra";
const PROVIDER = "evalproxy";
export const UMBRA_BIN = process.env.UMBRA_BIN || "umbra";
const FORMAT_ERROR_PATTERNS = [/Error parsing tool call arguments/, /Missing 'command' argument/, /Unknown tool '/];

/**
 * Returns the top level config lines an experiment arm sets, from `UMBRA_EVAL_CONFIG` (TOML
 * lines separated by `;`, for example `edit_tool = true; finish_check = false`). The run records
 * them in run.json, so a run says which arm it measured.
 */
export function armSettings() {
  return (process.env.UMBRA_EVAL_CONFIG || "").split(";").map((line) => line.trim()).filter(Boolean);
}

/**
 * Returns the lines an experiment arm puts inside the eval provider's table, from
 * `UMBRA_EVAL_PROVIDER_CONFIG` (TOML lines separated by `;`, for example `thinking = "off"`).
 * Provider settings such as `thinking` cannot be given as top level dotted keys, because the
 * provider table is defined after them.
 */
export function armProviderSettings() {
  return (process.env.UMBRA_EVAL_PROVIDER_CONFIG || "").split(";").map((line) => line.trim()).filter(Boolean);
}

/**
 * Writes `<home>/.umbra/config.toml` with a provider that points at the measuring proxy.
 * The built in `local` provider points at port 8080, so the base URL is set explicitly.
 * @param input - the AdapterInput
 */
function writeHomeConfig(input) {
  const dir = path.join(input.homeDir, ".umbra");
  fs.mkdirSync(dir, { recursive: true });
  const toml = [`model = "${PROVIDER}/${input.model}"`, ...armSettings(), "", `[providers.${PROVIDER}]`, `kind = "openai"`, `base_url = "${input.proxyBaseUrl}/v1"`, ...armProviderSettings(), ""].join("\n");
  fs.writeFileSync(path.join(dir, "config.toml"), toml);
}

/**
 * Builds the command, arguments and environment for one Umbra phase.
 * @param input - the AdapterInput
 */
export function buildLaunch(input) {
  writeHomeConfig(input);
  // Umbra runs with its default storage, as a person runs it: no -o, so the session is saved where
  // Umbra saves it and every byte it writes counts toward the session store (task-2643, advice
  // from task-2682). After the phase the adapter writes a copy with `umbra export`.
  const trajectoryPath = path.join(input.phaseDir, "trajectory.json");
  const args = ["-m", `${PROVIDER}/${input.model}`, "--yolo", "--quiet", "--no-fullscreen", "--no-status-bar", "--exit-immediately", "--no-mcp", "--no-skills", "--no-instructions", "--step-limit", String(input.stepLimit), "--cwd", input.workspaceDir];
  if (input.resume) args.push("--resume", input.resume.sessionRef);
  if (input.resume?.mode === "branch") args.push("--at", String(branchIndex(input.resume)), "--fork");
  if (input.resume?.mode === "fork") args.push("--fork");
  args.push("-t", input.prompt);
  const env = { ...process.env, UMBRA_HOME_OVERRIDE: input.homeDir, UMBRA_BANNER: "0", UMBRA_STATUS_BAR: "0", UMBRA_FULLSCREEN: "0" };
  return { command: UMBRA_BIN, args, env, cwd: input.workspaceDir, artifacts: { trajectoryPath, homeDir: input.homeDir, startedAt: Date.now() } };
}

/**
 * Reads the newest record of a phase's session: the trajectory JSON, or the journal Umbra 0.6.3
 * and later append after every step when it is newer (a run killed in the middle of a turn
 * leaves only the journal current), the way Umbra itself picks on resume.
 * @param file - the trajectory path
 * @param journal - the journal path next to it
 */
function newestRecord(file, journal) {
  const mtime = (p) => (fs.existsSync(p) ? fs.statSync(p).mtimeMs : -1);
  if (mtime(journal) >= 0 && mtime(journal) >= mtime(file)) return loadJournal(journal);
  return readJson(file);
}

/**
 * Rebuilds a trajectory from an Umbra session journal, skipping a line cut off by a kill.
 * @param journal - the journal path
 */
function loadJournal(journal) {
  const info = {};
  let messages = [];
  for (const line of fs.readFileSync(journal, "utf8").split("\n")) {
    let record;
    try { record = JSON.parse(line); } catch { continue; }
    if (record.t === "start") Object.assign(info, { config: record.config, umbra_version: record.umbra_version });
    else if (record.t === "msg") messages.push(record.m);
    else if (record.t === "truncate") messages = messages.slice(0, record.len);
    else if (record.t === "info") for (const [key, value] of Object.entries(record)) if (key !== "t") info[key] = value;
  }
  return { info, messages };
}

/**
 * Returns the number of messages a branch keeps: every message before the user message that
 * sent `beforePrompt`, read from the trajectory of the phase being branched.
 * @param resume - the resume target, with fromResult and beforePrompt
 */
function branchIndex(resume) {
  const file = resume.fromResult?.trajectoryPath;
  const journal = file ? file.replace(/\.json$/, ".journal.jsonl") : null;
  const messages = file && (fs.existsSync(file) || fs.existsSync(journal)) ? newestRecord(file, journal).messages || [] : [];
  const index = messages.findIndex((m) => m.role === "user" && (m.extra?.user_text ?? m.content) === resume.beforePrompt);
  return index < 0 ? messages.length : index;
}

/**
 * Returns the session index entry the phase wrote: the entry under `<home>/.umbra/sessions`
 * updated last, at or after the phase started. A new session and a resumed one both update their
 * entry at the start and at the end of a turn.
 * @param homeDir - the isolated home
 * @param startedAt - when the phase was launched, in ms
 */
function phaseSession(homeDir, startedAt) {
  const dir = path.join(homeDir, ".umbra", "sessions");
  if (!fs.existsSync(dir)) return null;
  const entries = fs.readdirSync(dir).filter((name) => name.endsWith(".json")).map((name) => readJson(path.join(dir, name), null)).filter(Boolean);
  const touched = entries.filter((entry) => (entry.updated_at || 0) * 1000 >= startedAt - 1000);
  touched.sort((a, b) => (b.updated_at || 0) - (a.updated_at || 0));
  return touched[0] || null;
}

/**
 * Writes the phase's session as trajectory JSON with `umbra export`, in a process of its own so
 * its writes are not counted as the agent's. A binary from before `umbra export` (task-2643, E17)
 * writes the trajectory JSON itself, and the copy is taken from that file.
 * @param session - the session index entry
 * @param out - where to write the JSON
 */
function exportSession(session, out) {
  const exported = spawnSync(UMBRA_BIN, ["export", session.journal || session.trajectory, "-o", out], { encoding: "utf8", windowsHide: true, env: { ...process.env, UMBRA_BANNER: "0" } });
  if (exported.status === 0 && fs.existsSync(out)) return;
  if (session.trajectory && fs.existsSync(session.trajectory)) fs.copyFileSync(session.trajectory, out);
}

/**
 * Returns the file Umbra resumes a session from: the journal when the session index names one,
 * otherwise the trajectory.
 * @param homeDir - the isolated home
 * @param sessionRef - the session id
 */
export function sessionStoreFile(homeDir, sessionRef) {
  const entry = readJson(path.join(homeDir, ".umbra", "sessions", `${sessionRef}.json`), null);
  if (!entry) return null;
  return entry.journal || entry.trajectory || null;
}

/**
 * Reads the trajectory a phase wrote: session id, exit status, malformed tool calls,
 * compactions, and whether the file parses.
 * @param launch - the object buildLaunch returned
 */
export function readArtifacts(launch) {
  const file = launch.artifacts.trajectoryPath;
  const session = phaseSession(launch.artifacts.homeDir, launch.artifacts.startedAt);
  const journal = session?.journal || file.replace(/\.json$/, ".journal.jsonl");
  if (session) exportSession(session, file);
  if (!fs.existsSync(file) && !fs.existsSync(journal)) return { sessionRef: null, sessionValid: false, sessionDetail: "no trajectory written", malformedToolCalls: 0, agentExitStatus: null, toolCalls: 0, compactions: 0 };
  let trajectory;
  try { trajectory = newestRecord(file, journal); } catch (error) {
    return { sessionRef: null, sessionValid: false, sessionDetail: `trajectory does not parse: ${error.message}`, malformedToolCalls: 0, agentExitStatus: null, toolCalls: 0, compactions: 0 };
  }
  const messages = trajectory.messages || [];
  const userTexts = messages.filter((m) => m.role === "user").map((m) => (typeof m.content === "string" ? m.content : JSON.stringify(m.content)));
  return {
    sessionRef: trajectory.info?.session?.id || null,
    sessionValid: Array.isArray(trajectory.messages),
    sessionDetail: "",
    malformedToolCalls: userTexts.filter((text) => FORMAT_ERROR_PATTERNS.some((re) => re.test(text))).length,
    agentExitStatus: trajectory.info?.exit_status || null,
    toolCalls: messages.filter((m) => m.role === "tool").length,
    compactions: (trajectory.info?.compactions || []).length,
    messageCount: messages.length,
    trajectoryPath: file,
    agentVersion: trajectory.info?.umbra_version || null,
  };
}

// Branches a pi session at an earlier user message and sends a new prompt, through pi's RPC mode,
// because pi's print mode can only fork a whole session. Started by the pi adapter as
//   node piRpcBranch.mjs <spec.json>
// where spec.json is { piCli, piArgs, sessionRef, beforePrompt, prompt }. It prints a `session`
// record for the new session and then every record pi prints, so the output reads like pi's JSON
// mode, and exits when pi reports `agent_settled`.

import fs from "node:fs";
import { spawn } from "node:child_process";

/**
 * Reads LF separated JSON records from a stream and calls `onRecord` for each. Splits only on LF,
 * as pi's RPC framing requires.
 * @param stream - pi's stdout
 * @param onRecord - called with each parsed record and its raw line
 */
function readRecords(stream, onRecord) {
  let buffer = "";
  stream.setEncoding("utf8");
  stream.on("data", (chunk) => {
    buffer += chunk;
    let index;
    while ((index = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, index).replace(/\r$/, "");
      buffer = buffer.slice(index + 1);
      if (!line.trim()) continue;
      try { onRecord(JSON.parse(line), line); } catch { process.stdout.write(`${line}\n`); }
    }
  });
}

/**
 * Starts pi in RPC mode and returns a function that sends a command and resolves with its
 * response, a promise that resolves at `agent_settled`, and one that resolves when pi exits.
 * @param spec - the branch spec
 */
function startPi(spec) {
  const child = spawn(process.execPath, [spec.piCli, "--mode", "rpc", ...spec.piArgs, "--session", spec.sessionRef], { stdio: ["pipe", "pipe", "inherit"], windowsHide: true });
  const waiting = new Map();
  let settle;
  const settled = new Promise((resolve) => { settle = resolve; });
  readRecords(child.stdout, (record, line) => {
    if (record.type === "response" && waiting.has(record.id)) {
      waiting.get(record.id)(record);
      waiting.delete(record.id);
      return;
    }
    process.stdout.write(`${line}\n`);
    if (record.type === "agent_settled") settle(record);
  });
  const exited = new Promise((resolve) => child.on("exit", (code) => { settle({ type: "exit", code }); resolve(code); }));
  let counter = 0;
  const send = (command) => new Promise((resolve) => {
    const id = `branch-${(counter += 1)}`;
    waiting.set(id, resolve);
    child.stdin.write(`${JSON.stringify({ ...command, id })}\n`);
  });
  return { child, send, settled, exited };
}

/**
 * Runs the branch: finds the user message to branch before, forks there, prints the new session
 * id, sends the prompt and waits for pi to settle.
 * @param spec - the branch spec
 */
async function main(spec) {
  const pi = startPi(spec);
  try {
    await branchAndPrompt(pi, spec);
  } finally {
    pi.child.stdin.end();
    setTimeout(() => pi.child.kill(), 2000).unref();
    await pi.exited;
  }
}

/**
 * Forks before the user message `spec.beforePrompt`, prints the new session record, sends the
 * prompt and waits until pi settles.
 * @param pi - the started pi process
 * @param spec - the branch spec
 */
async function branchAndPrompt(pi, spec) {
  const forkable = await pi.send({ type: "get_fork_messages" });
  const messages = forkable.data?.messages || [];
  const target = messages.find((m) => m.text.trim() === spec.beforePrompt.trim());
  if (!target) throw new Error(`no user message matches the branch point; pi offered ${messages.length}`);
  const fork = await pi.send({ type: "fork", entryId: target.entryId });
  if (!fork.success || fork.data?.cancelled) throw new Error(`fork failed: ${JSON.stringify(fork)}`);
  const state = await pi.send({ type: "get_state" });
  process.stdout.write(`${JSON.stringify({ type: "session", id: state.data?.sessionId, sessionFile: state.data?.sessionFile, branchedFrom: spec.sessionRef })}\n`);
  const prompt = await pi.send({ type: "prompt", message: spec.prompt });
  if (prompt.success && prompt.data?.disposition !== "handled") await pi.settled;
}

main(JSON.parse(fs.readFileSync(process.argv[2], "utf8"))).then(() => { process.exitCode = 0; }, (error) => { console.error(error.stack || error); process.exitCode = 1; });

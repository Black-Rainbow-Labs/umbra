// Checks the scorecard rules on a fixture run folder: the five verdicts, the category rows, the
// list "Rows where Umbra is behind pi", `report --pool`, the bootstrap seed and the outside path
// rules. Run with `node --test evals/tests`.

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const RUNS = fs.mkdtempSync(path.join(os.tmpdir(), "umbra-scorecard-test-"));
process.env.EVAL_RUNS_DIR = RUNS;
const { verdictOf, pairedBootstrap } = await import("../src/services/scorecardVerdict.mjs");
const { buildScorecard } = await import("../src/services/scorecardService.mjs");
const { outsidePathsOf, normalizePath } = await import("../src/services/outsidePaths.mjs");

/**
 * Writes one fixture cell with a given verdict and reasoning tokens.
 * @param runId - the run
 * @param cell - { taskId, category, agent, rep, pass, reasoning }
 */
function writeCell(runId, cell) {
  const dir = path.join(RUNS, runId, "cells", cell.taskId, cell.agent, String(cell.rep));
  fs.mkdirSync(dir, { recursive: true });
  const result = { taskId: cell.taskId, category: cell.category, agent: cell.agent, rep: cell.rep, pass: cell.pass, verifierPass: cell.pass, score: cell.pass ? 1 : 0.5, detail: "", wallMs: 1000, phases: [] };
  fs.writeFileSync(path.join(dir, "result.json"), JSON.stringify(result));
  const request = { kind: "chat", usage: { prompt_tokens: 100, cached_tokens: 50, completion_tokens: 10 + cell.reasoning, reasoning_tokens: cell.reasoning }, firstContentMs: 10, durationMs: 100, hasToolCall: true, status: 200 };
  fs.writeFileSync(path.join(dir, "requests.jsonl"), `${JSON.stringify(request)}\n`);
}

/**
 * Writes a run of 3 reps of two tasks for one agent.
 * @param runId - the run
 * @param agent - umbra or pi
 * @param passes - (taskId, rep) => boolean
 * @param reasoning - (taskId, rep) => number
 */
function writeRun(runId, agent, passes, reasoning) {
  fs.mkdirSync(path.join(RUNS, runId), { recursive: true });
  fs.writeFileSync(path.join(RUNS, runId, "run.json"), JSON.stringify({ runId, agents: [agent] }));
  for (const [taskId, category] of [["a", "hard"], ["b", "edit-precision"]]) {
    for (let rep = 1; rep <= 3; rep += 1) writeCell(runId, { taskId, category, agent, rep, pass: passes(taskId, rep), reasoning: reasoning(taskId, rep) });
  }
}

writeRun("fx-pi", "pi", () => true, (task, rep) => 1000 + rep * 100);
writeRun("fx-u1", "umbra", (task, rep) => !(task === "b" && rep === 1), () => 1250);
writeRun("fx-u2", "umbra", () => true, () => 900);

test("the five verdicts", () => {
  assert.equal(verdictOf({ key: "x", umbra: 5, pi: 5, noise: 1, higherIsBetter: false }), "equal");
  assert.equal(verdictOf({ key: "overall.nonzeroExits", umbra: 0, pi: 1, noise: 1, higherIsBetter: false }), "equal");
  assert.equal(verdictOf({ key: "overall.passRate", umbra: 1, pi: 0.9, noise: 0.2, higherIsBetter: true }), "equal");
  assert.equal(verdictOf({ key: "x", umbra: 4.5, pi: 5, noise: 1, higherIsBetter: false }), "tie, Umbra ahead");
  assert.equal(verdictOf({ key: "x", umbra: 5.5, pi: 5, noise: 1, higherIsBetter: false }), "tie, pi ahead");
  assert.equal(verdictOf({ key: "x", umbra: 3, pi: 5, noise: 1, higherIsBetter: false }), "Umbra");
  assert.equal(verdictOf({ key: "x", umbra: 7, pi: 5, noise: 1, higherIsBetter: false }), "pi");
});

test("a run lists the category and reasoning rows it is behind on", () => {
  const card = buildScorecard("fx-u1", "fx-pi");
  const row = (label) => card.headToHead.find((r) => r.label === label);
  assert.equal(row("Category pass rate: edit-precision").better, "pi");
  assert.equal(row("Category pass rate: hard").better, "equal");
  assert.equal(row("Reasoning tokens, total").better, "tie, pi ahead");
  assert.ok(card.umbraBehind.includes("Category pass rate: edit-precision"));
  assert.ok(card.umbraBehind.includes("Reasoning tokens, total"));
  assert.ok(!card.umbraBehind.includes("Runs that named a path outside the workspace (reported)"));
});

test("pooling two runs scores 6 reps and the bootstrap is the same for the same seed", () => {
  const pooled = buildScorecard(["fx-u1", "fx-u2"], "fx-pi");
  assert.equal(pooled.reps.umbra, 6);
  assert.equal(pooled.headToHead.find((r) => r.label === "Reasoning tokens, total").umbra, 2 * (1250 + 900) / 2);
  assert.deepEqual(pairedBootstrap([0.1, -0.2, 0.3, 0]), pairedBootstrap([0.1, -0.2, 0.3, 0]));
});

test("outside path rules", () => {
  const ws = "C:/Users/someone/work/ws";
  const context = { cwd: ws, home: "C:/Users/someone", tempDir: "C:/Temp", allowed: [ws, "C:/Temp", "C:/Program Files/Git"].map(normalizePath) };
  if (process.platform !== "win32") return;
  assert.deepEqual(outsidePathsOf("find ~ -name 'x.json'", context).hits, ["~"]);
  assert.deepEqual(outsidePathsOf("cat C:/Users/someone/secret.txt", context).hits, ["C:/Users/someone/secret.txt"]);
  assert.deepEqual(outsidePathsOf("cd .. && ls", context).hits, [".."]);
  assert.deepEqual(outsidePathsOf("find / -name a", context).hits, ["/"]);
  assert.deepEqual(outsidePathsOf("cd src && cat ../README.md && cd ..", context).hits, []);
  assert.deepEqual(outsidePathsOf("sed -n '/start/,/end/p' a.txt > /dev/null 2>&1", context).hits, []);
  assert.deepEqual(outsidePathsOf("cat <<'EOF' > a.js\nrequire('../../x')\nEOF", context).hits, []);
  assert.deepEqual(outsidePathsOf("node -e \"console.log(1 / 2) // half\"", context).hits, []);
});

test.after(() => fs.rmSync(RUNS, { recursive: true, force: true }));

// Where run output lives and how cell results are read and written.

import fs from "node:fs";
import path from "node:path";
import { EVALS_DIR } from "./taskRepository.mjs";
import { readJson, writeJson, readJsonl } from "../utils/files.mjs";

export const RUNS_DIR = process.env.EVAL_RUNS_DIR || path.resolve(EVALS_DIR, "..", "_agent_output", "task-2614", "eval-runs");

/**
 * Returns the folder of a run.
 * @param runId - the run id
 */
export function runDir(runId) {
  return path.join(RUNS_DIR, runId);
}

/**
 * Returns the folder of one cell (task, agent, repetition) of a run.
 * @param runId - the run id
 * @param taskId - the task id
 * @param agent - the agent name
 * @param rep - the repetition number, starting at 1
 */
export function cellDir(runId, taskId, agent, rep) {
  return path.join(runDir(runId), "cells", taskId, agent, String(rep));
}

/**
 * Returns true when a cell already has a result.json, which means a resumed run skips it.
 * @param dir - the cell folder
 */
export function cellFinished(dir) {
  return fs.existsSync(path.join(dir, "result.json"));
}

/**
 * Writes a cell's result.json.
 * @param dir - the cell folder
 * @param result - the cell result
 */
export function writeCellResult(dir, result) {
  writeJson(path.join(dir, "result.json"), result);
}

/**
 * Writes or updates run.json, the run's settings and versions.
 * @param runId - the run id
 * @param meta - fields to merge into run.json
 */
export function writeRunMeta(runId, meta) {
  const file = path.join(runDir(runId), "run.json");
  const current = readJson(file, {});
  writeJson(file, { ...current, ...meta });
}

/**
 * Loads every finished cell of a run with its request records.
 * @param runId - the run id
 */
export function loadRunCells(runId) {
  const root = path.join(runDir(runId), "cells");
  const cells = [];
  if (!fs.existsSync(root)) return cells;
  for (const taskId of fs.readdirSync(root)) {
    for (const agent of fs.readdirSync(path.join(root, taskId))) {
      for (const rep of fs.readdirSync(path.join(root, taskId, agent))) {
        const dir = path.join(root, taskId, agent, rep);
        if (!cellFinished(dir)) continue;
        cells.push({ dir, result: readJson(path.join(dir, "result.json")), requests: readJsonl(path.join(dir, "requests.jsonl")) });
      }
    }
  }
  return cells;
}

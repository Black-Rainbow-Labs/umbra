// Loads task definitions from evals/tasks/<category>/<id>/task.json.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { readJson } from "../utils/files.mjs";

export const EVALS_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
export const TASKS_DIR = path.join(EVALS_DIR, "tasks");
const CATEGORY_ORDER = ["smoke", "create", "bugfix", "edit-precision", "navigate", "large-output", "instructions", "compaction", "session", "hard"];

/**
 * Loads every task, sorted by category order and then by id.
 * @param tasksDir - the tasks folder (defaults to evals/tasks)
 */
export function loadAllTasks(tasksDir = TASKS_DIR) {
  const tasks = [];
  for (const category of fs.readdirSync(tasksDir)) {
    const categoryDir = path.join(tasksDir, category);
    if (!fs.statSync(categoryDir).isDirectory()) continue;
    for (const id of fs.readdirSync(categoryDir)) {
      const dir = path.join(categoryDir, id);
      if (fs.existsSync(path.join(dir, "task.json"))) tasks.push(loadTask(dir, category));
    }
  }
  const rank = (task) => { const index = CATEGORY_ORDER.indexOf(task.category); return index < 0 ? 99 : index; };
  return tasks.sort((a, b) => rank(a) - rank(b) || a.id.localeCompare(b.id));
}

/**
 * Loads one task folder and fills in defaults.
 * @param dir - the task folder
 * @param category - the category folder name
 */
export function loadTask(dir, category) {
  const raw = readJson(path.join(dir, "task.json"));
  if (raw.id !== path.basename(dir)) throw new Error(`task id ${raw.id} does not match folder ${dir}`);
  return { stepLimit: 60, timeoutSeconds: 300, largeFixtures: [], ...raw, category, dir };
}

/**
 * Picks the tasks a run should use from --tasks and --category, leaving out tasks marked
 * excludeFromDefault unless they are named.
 * @param tasks - every loaded task
 * @param filters - { taskIds: string[]|null, category: string|null }
 */
export function selectTasks(tasks, filters) {
  if (filters.taskIds?.length) {
    const missing = filters.taskIds.filter((id) => !tasks.some((task) => task.id === id));
    if (missing.length) throw new Error(`unknown task ids: ${missing.join(", ")}`);
    return tasks.filter((task) => filters.taskIds.includes(task.id));
  }
  return tasks.filter((task) => !task.excludeFromDefault && (!filters.category || task.category === filters.category));
}

/**
 * Returns the phases of a task. A task without `phases` has one phase from its prompt.
 * @param task - the EvalTask
 */
export function taskPhases(task) {
  if (Array.isArray(task.phases) && task.phases.length) return task.phases;
  return [{ prompt: task.prompt, mode: "new", label: "main" }];
}

/**
 * Imports a module from a task folder (verify.mjs, generate.mjs or solution/solve.mjs) if it exists.
 * @param task - the EvalTask
 * @param relativePath - path inside the task folder
 */
export async function importTaskModule(task, relativePath) {
  const file = path.join(task.dir, relativePath);
  if (!fs.existsSync(file)) return null;
  return import(pathToFileURL(file).href);
}

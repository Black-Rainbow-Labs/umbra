// Small file helpers used across the suite.

import fs from "node:fs";
import path from "node:path";

/**
 * Reads and parses a JSON file, or returns the fallback when the file is missing.
 * @param filePath - path of the JSON file
 * @param fallback - value returned when the file does not exist
 */
export function readJson(filePath, fallback = undefined) {
  if (!fs.existsSync(filePath)) {
    if (fallback !== undefined) return fallback;
    throw new Error(`missing file ${filePath}`);
  }
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

/**
 * Writes a value as pretty JSON, creating the parent folder.
 * @param filePath - destination path
 * @param value - the value to write
 */
export function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(value, null, 2) + "\n");
}

/**
 * Reads a JSON lines file into an array, skipping lines that do not parse.
 * @param filePath - path of the .jsonl file
 */
export function readJsonl(filePath) {
  if (!fs.existsSync(filePath)) return [];
  const rows = [];
  for (const line of fs.readFileSync(filePath, "utf8").split("\n")) {
    if (!line.trim()) continue;
    try { rows.push(JSON.parse(line)); } catch { /* a line cut by a kill */ }
  }
  return rows;
}

/**
 * Copies a folder tree. Missing sources are treated as empty folders.
 * @param from - source folder
 * @param to - destination folder, created if needed
 */
export function copyTree(from, to) {
  fs.mkdirSync(to, { recursive: true });
  if (fs.existsSync(from)) fs.cpSync(from, to, { recursive: true, force: true });
}

/**
 * Removes a folder tree that the suite itself created.
 * @param target - absolute path of the folder or file
 */
export function removeCreatedTree(target) {
  if (fs.existsSync(target)) fs.rmSync(target, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}

/**
 * Lists every file under a folder as paths relative to it, with forward slashes.
 * @param root - the folder to walk
 */
export function listFiles(root) {
  const out = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else out.push(path.relative(root, full).split(path.sep).join("/"));
    }
  };
  if (fs.existsSync(root)) walk(root);
  return out.sort();
}

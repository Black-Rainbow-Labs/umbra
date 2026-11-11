// Shared helpers for deterministic fixture generation: a seeded random source, list helpers and file writing.

import fs from "node:fs";
import path from "node:path";

/**
 * Creates a seeded pseudo random number generator that returns floats in [0, 1).
 * @param seed - integer seed
 */
export function mulberry32(seed) {
  let state = seed >>> 0;
  return function next() {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Returns an integer in [low, high] from the generator.
 * @param rng - generator from mulberry32
 * @param low - smallest value
 * @param high - largest value
 */
export function randomInt(rng, low, high) {
  return low + Math.floor(rng() * (high - low + 1));
}

/**
 * Returns one element of a list chosen by the generator.
 * @param rng - generator from mulberry32
 * @param list - the candidates
 */
export function pick(rng, list) {
  return list[Math.floor(rng() * list.length)];
}

/**
 * Returns a shuffled copy of a list.
 * @param rng - generator from mulberry32
 * @param list - the items to shuffle
 */
export function shuffled(rng, list) {
  const copy = [...list];
  for (let index = copy.length - 1; index > 0; index -= 1) {
    const other = Math.floor(rng() * (index + 1));
    [copy[index], copy[other]] = [copy[other], copy[index]];
  }
  return copy;
}

/**
 * Converts a dash or underscore separated name to camelCase.
 * @param name - for example "bulk-lookup"
 */
export function toCamel(name) {
  return name.replace(/[-_]+([a-z0-9])/g, (_, letter) => letter.toUpperCase());
}

/**
 * Writes a map of relative path to text into a folder, creating subfolders.
 * @param rootDir - destination folder
 * @param files - Map of relative path to file text
 */
export function writeFiles(rootDir, files) {
  for (const [relative, text] of files) {
    const target = path.join(rootDir, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, text);
  }
}

// Folders that test runners and interpreters create on their own; they are not part of a change.
const TOOL_CACHE_DIRS = new Set(["node_modules", "__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache", ".cache"]);

/**
 * Reads every file under a folder, except tool cache folders and .pyc files, into a Map of relative path (forward slashes) to Buffer.
 * @param rootDir - the folder to read
 */
export function readFilesAsBuffers(rootDir) {
  const out = new Map();
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) { if (!TOOL_CACHE_DIRS.has(entry.name)) walk(full); }
      else if (!entry.name.endsWith(".pyc")) out.set(path.relative(rootDir, full).split(path.sep).join("/"), fs.readFileSync(full));
    }
  };
  if (fs.existsSync(rootDir)) walk(rootDir);
  return out;
}

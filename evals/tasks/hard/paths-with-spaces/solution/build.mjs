// Builds the site from "my docs/input files" into "build output".
// Usage: node build.mjs (from the project folder).
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const INPUT_DIR = path.join(ROOT, "my docs", "input files");
const TEMPLATE_FILE = path.join(ROOT, "my docs", "templates", "page template.html");
const OUT_DIR = path.join(ROOT, "build output");

/**
 * Runs a helper tool with node and returns its standard output. Arguments are passed as a list, so no quoting is needed.
 * @param script - the script path relative to the project folder
 * @param args - the arguments
 */
function runTool(script, args) {
  const result = spawnSync(process.execPath, [script, ...args], { cwd: ROOT, encoding: "utf8" });
  if (result.status !== 0) throw new Error(`tool failed: ${script} ${args.join(" ")}\n${result.stderr}`);
  return result.stdout;
}

/**
 * Converts a path to forward slashes.
 * @param value - a path
 */
function toPosix(value) {
  return value.split(path.sep).join("/");
}

/**
 * Turns a file name without extension into the name used for generated files: lower case, apostrophes removed,
 * every run of other characters that are not letters or digits becomes one hyphen, hyphens at both ends removed.
 * @param name - the file name without extension
 */
function slugify(name) {
  return name.toLowerCase().replace(/['\u2019]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}

/**
 * Escapes the characters that matter in HTML text.
 * @param text - raw text
 */
function escapeHtml(text) {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
}

/**
 * Lists every file under a folder, recursively.
 * @param dir - the folder
 */
function listFiles(dir) {
  const found = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) found.push(...listFiles(full));
    else found.push(full);
  }
  return found;
}

/**
 * Builds the output for one input file and returns its manifest entry.
 * @param file - the absolute path of the input file
 */
function buildFile(file) {
  const extension = path.extname(file).toLowerCase();
  const base = path.basename(file, path.extname(file));
  let output;
  if (extension === ".md") {
    output = path.join(OUT_DIR, "pages", slugify(base) + ".html");
    runTool("tools/render-md.mjs", [file, output]);
  } else if (extension === ".csv") {
    output = path.join(OUT_DIR, "tables", slugify(base) + ".html");
    runTool("tools/csv-table.mjs", [file, output]);
  } else {
    output = path.join(OUT_DIR, "raw", path.relative(INPUT_DIR, file));
    fs.mkdirSync(path.dirname(output), { recursive: true });
    fs.copyFileSync(file, output);
  }
  const sha256 = runTool("tools/checksum.mjs", [output]).trim();
  return { source: toPosix(path.relative(ROOT, file)), output: toPosix(path.relative(ROOT, output)), sha256, base };
}

/**
 * Writes index.html from the page template, with a link to every page and table.
 * @param entries - the manifest entries
 */
function writeIndex(entries) {
  const linked = entries.filter((entry) => entry.output.startsWith("build output/pages/") || entry.output.startsWith("build output/tables/"));
  linked.sort((a, b) => (a.output < b.output ? -1 : a.output > b.output ? 1 : 0));
  const links = linked.map((entry) => `<li><a href="${entry.output.slice("build output/".length)}">${escapeHtml(entry.base)}</a></li>`);
  const template = fs.readFileSync(TEMPLATE_FILE, "utf8");
  const html = template.split("{{title}}").join("Site index").split("{{links}}").join(links.join("\n"));
  fs.writeFileSync(path.join(OUT_DIR, "index.html"), html);
}

/**
 * Builds the whole site from a clean output folder.
 */
function main() {
  fs.rmSync(OUT_DIR, { recursive: true, force: true });
  for (const folder of ["pages", "tables", "raw"]) fs.mkdirSync(path.join(OUT_DIR, folder), { recursive: true });
  const entries = listFiles(INPUT_DIR).map(buildFile);
  entries.sort((a, b) => (a.source < b.source ? -1 : a.source > b.source ? 1 : 0));
  writeIndex(entries);
  const manifest = entries.map(({ source, output, sha256 }) => ({ source, output, sha256 }));
  fs.writeFileSync(path.join(OUT_DIR, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
}

main();

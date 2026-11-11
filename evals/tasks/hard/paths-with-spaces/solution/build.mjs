// Builds the site from "my docs/input files" into "build output".
// Usage: node build.mjs (from the project folder), or node build.mjs --check to compare the output with its manifest.
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
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
 * Compares two strings as plain code units, like the default JavaScript sort.
 * @param a - first string
 * @param b - second string
 */
function byCodeUnits(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * Turns a file name without extension into the name used for generated files: lower case, apostrophes removed,
 * every run of other characters that are not a-z or 0-9 becomes one hyphen, hyphens at both ends removed, "untitled" when nothing is left.
 * @param name - the file name without extension
 */
function slugify(name) {
  const slug = name.toLowerCase().replace(/['\u2019]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  return slug || "untitled";
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
 * Returns the SHA-256 of a file as lower case hex.
 * @param file - the file path
 */
function sha256Of(file) {
  return createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

/**
 * Returns a slug that no earlier file of the same output folder used: the first keeps it, later ones get -2, -3 and so on.
 * @param used - map of "folder/slug" to how many files have used it
 * @param folder - "pages" or "tables"
 * @param base - the input file name without extension
 */
function uniqueSlug(used, folder, base) {
  const slug = slugify(base);
  const count = (used.get(`${folder}/${slug}`) || 0) + 1;
  used.set(`${folder}/${slug}`, count);
  return count === 1 ? slug : `${slug}-${count}`;
}

/**
 * Builds the output for one input file and returns its manifest entry.
 * @param file - the absolute path of the input file
 * @param used - the slug counters shared by the whole build
 */
function buildFile(file, used) {
  const extension = path.extname(file).toLowerCase();
  const base = path.basename(file, path.extname(file));
  let output;
  if (extension === ".md") {
    output = path.join(OUT_DIR, "pages", uniqueSlug(used, "pages", base) + ".html");
    runTool("tools/render-md.mjs", [file, output]);
  } else if (extension === ".csv") {
    output = path.join(OUT_DIR, "tables", uniqueSlug(used, "tables", base) + ".html");
    runTool("tools/csv-table.mjs", [file, output]);
  } else {
    output = path.join(OUT_DIR, "raw", path.relative(INPUT_DIR, file));
    fs.mkdirSync(path.dirname(output), { recursive: true });
    fs.copyFileSync(file, output);
  }
  return { source: toPosix(path.relative(ROOT, file)), output: toPosix(path.relative(ROOT, output)), sha256: sha256Of(output), base };
}

/**
 * Writes index.html from the page template, with a link to every page and table.
 * @param entries - the manifest entries
 */
function writeIndex(entries) {
  const linked = entries.filter((entry) => entry.output.startsWith("build output/pages/") || entry.output.startsWith("build output/tables/"));
  linked.sort((a, b) => byCodeUnits(a.output, b.output));
  const links = linked.map((entry) => `<li><a href="${entry.output.slice("build output/".length)}">${escapeHtml(entry.base)}</a></li>`);
  const template = fs.readFileSync(TEMPLATE_FILE, "utf8");
  const html = template.split("{{title}}").join("Site index").split("{{links}}").join(links.join("\n"));
  fs.writeFileSync(path.join(OUT_DIR, "index.html"), html);
}

/**
 * Builds the whole site from a clean output folder. Files are processed in manifest order so slug numbering is stable.
 */
function main() {
  fs.rmSync(OUT_DIR, { recursive: true, force: true });
  for (const folder of ["pages", "tables", "raw"]) fs.mkdirSync(path.join(OUT_DIR, folder), { recursive: true });
  const files = listFiles(INPUT_DIR).sort((a, b) => byCodeUnits(toPosix(path.relative(ROOT, a)), toPosix(path.relative(ROOT, b))));
  const used = new Map();
  const entries = files.map((file) => buildFile(file, used));
  writeIndex(entries);
  const manifest = entries.map(({ source, output, sha256 }) => ({ source, output, sha256 }));
  fs.writeFileSync(path.join(OUT_DIR, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
}

/**
 * Compares every file listed in the manifest with its recorded checksum and prints "ok", "changed" or "missing" lines.
 * Sets the exit code to 0 when all are ok, 1 otherwise, and 2 when there is no manifest.
 */
function check() {
  const manifestFile = path.join(OUT_DIR, "manifest.json");
  if (!fs.existsSync(manifestFile)) {
    console.error("no manifest");
    process.exitCode = 2;
    return;
  }
  let allOk = true;
  for (const entry of JSON.parse(fs.readFileSync(manifestFile, "utf8"))) {
    const file = path.join(ROOT, ...entry.output.split("/"));
    const status = !fs.existsSync(file) ? "missing" : sha256Of(file) === entry.sha256 ? "ok" : "changed";
    if (status !== "ok") allOk = false;
    console.log(`${status} ${entry.output}`);
  }
  process.exitCode = allOk ? 0 : 1;
}

if (process.argv[2] === "--check") check();
else main();

// Verifier: every file must equal the expected bytes; a failure is explained as encoding, BOM, line ending or content.

import fs from "node:fs";
import path from "node:path";
import { fileSpecs, encodeFile, decodeFile } from "./fixture.mjs";

const ENCODING_NAMES = { utf8: "UTF-8", utf16le: "UTF-16 LE", utf16be: "UTF-16 BE", cp1252: "Windows-1252" };

/**
 * Lists every file under a folder as relative paths with forward slashes.
 * @param root - the folder
 * @param prefix - relative prefix used in recursion
 */
function listTree(root, prefix = "") {
  if (!fs.existsSync(root)) return [];
  const found = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) found.push(...listTree(path.join(root, entry.name), relative));
    else found.push(relative);
  }
  return found.sort();
}

/**
 * Describes the first difference between the text found and the text expected, ignoring line endings.
 * @param text - decoded text found
 * @param expected - decoded text expected
 */
function firstLineDifference(text, expected) {
  const found = text.split(/\r\n|\r|\n/);
  const wanted = expected.split(/\r\n|\r|\n/);
  for (let index = 0; index < Math.max(found.length, wanted.length); index += 1) {
    if (found[index] !== wanted[index]) return `line ${index + 1} is ${JSON.stringify(found[index])}, expected ${JSON.stringify(wanted[index])}`;
  }
  return "text differs";
}

/**
 * Explains why a file differs from the expected bytes, or returns "" when it is identical.
 * @param spec - the file spec
 * @param buffer - the file bytes
 */
function diagnose(spec, buffer) {
  const expectedBuffer = spec.after ? encodeFile(spec, spec.after, "after") : encodeFile(spec, spec.before, "before");
  if (buffer.equals(expectedBuffer)) return "";
  if (!spec.after) return "the file must not be modified";
  const decoded = decodeFile(spec, buffer);
  if (!decoded) return `no longer valid ${ENCODING_NAMES[spec.encoding]} (encoding changed)`;
  if (spec.encoding === "cp1252" && buffer.toString("utf8").match(/[^\x00-\x7f]/) && !buffer.toString("utf8").includes("�")) return "converted to UTF-8 (encoding changed)";
  if (decoded.hasBom !== spec.bom) return spec.bom ? "the byte order mark was lost" : "a byte order mark was added";
  const expectedText = decodeFile(spec, expectedBuffer).text;
  const strip = (value) => value.replace(/\r/g, "");
  if (strip(decoded.text) === strip(expectedText)) return "line endings changed";
  if (strip(decoded.text).replace(/\n$/, "") === strip(expectedText).replace(/\n$/, "")) return "the newline at the end of the file changed";
  return firstLineDifference(decoded.text, expectedText);
}

/**
 * Verifies each file's bytes and that no other file was added or removed.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const checks = [];
  const specs = fileSpecs();
  for (const spec of specs) {
    const file = path.join(workspaceDir, ...spec.path.split("/"));
    const why = fs.existsSync(file) ? diagnose(spec, fs.readFileSync(file)) : "the file is missing";
    checks.push({ name: spec.path, ok: !why, why });
  }
  const expectedSet = new Set(specs.map((spec) => spec.path));
  const found = listTree(workspaceDir);
  const stray = found.filter((relative) => !expectedSet.has(relative));
  checks.push({ name: "no files added", ok: stray.length === 0, why: `extra files: ${stray.slice(0, 3).join(", ")}` });
  const passed = checks.filter((check) => check.ok).length;
  const failed = checks.filter((check) => !check.ok);
  const detail = failed.length ? `${passed}/${checks.length} checks; failed: ${failed.slice(0, 3).map((check) => `${check.name} (${check.why.slice(0, 120)})`).join("; ")}` : `all ${checks.length} checks passed`;
  return { pass: failed.length === 0, score: passed / checks.length, detail };
}

// Verifier for the model and serializer task: runs the serializers and checks that only Customer changed.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { buildRepo, CHANGED_FILES } from "./fixture.mjs";
import { writeFiles, readFilesAsBuffers } from "./gen-utils.mjs";

const DUMP_SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), "hidden", "dump_serializers.py");

/**
 * Runs the dump script against a package root and returns parsed JSON or an error text.
 * @param root - folder that contains the shopkit package
 */
function dumpSerializers(root) {
  const run = spawnSync("python", ["-I", "-B", DUMP_SCRIPT, root], { encoding: "utf8", timeout: 60000 });
  if (run.status !== 0) return { error: (run.stderr || "python failed").trim().split("\n").pop() };
  return { value: JSON.parse(run.stdout) };
}

/**
 * Checks that every file outside the Customer model and serializer is byte identical.
 * @param original - the original build
 * @param actual - Map of path to Buffer from the workspace
 */
function checkFiles(original, actual) {
  if ([...original.files.keys()].sort().join("\n") !== [...actual.keys()].sort().join("\n")) return describeFileSetChange(original.files, actual);
  for (const [file, text] of original.files) {
    if (CHANGED_FILES.includes(file)) continue;
    if (!actual.get(file).equals(Buffer.from(text))) return `${file} was changed`;
  }
  return "";
}

/**
 * Serializes an object with its keys in sorted order so two dicts compare by content.
 * @param value - a flat object
 */
function sortedJson(value) {
  return JSON.stringify(Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b))));
}

/**
 * Checks the Customer behavior and that every other serialized sample equals the original.
 * @param before - dump of the original (the customer section of it is not usable there)
 * @param after - dump of the workspace
 */
function checkSerializers(before, after) {
  const customer = after.customer;
  if (customer.default_tier !== "standard") return `Customer().loyalty_tier is ${JSON.stringify(customer.default_tier)}, expected "standard"`;
  const baseline = before.samples.customer;
  if (sortedJson(after.samples.customer) !== sortedJson({ ...baseline, loyalty_tier: "standard" })) return "serialize_customer does not return the old fields plus loyalty_tier";
  if (customer.gold_serialized.loyalty_tier !== "gold") return "serialize_customer does not pass through a given loyalty_tier";
  for (const entity of Object.keys(before.samples)) {
    if (entity === "customer") continue;
    if (JSON.stringify(before.samples[entity]) !== JSON.stringify(after.samples[entity])) return `the serialized ${entity} changed`;
  }
  return "";
}

/**
 * Builds the dump of the original build; the original has no loyalty_tier so only its samples are used.
 * @param original - the original build
 */
function dumpOriginalSamples(original) {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "serializer-original-"));
  try {
    writeFiles(temp, original.files);
    const script = spawnSync("python", ["-I", "-B", "-c", PRINT_SAMPLES, temp], { encoding: "utf8", timeout: 60000 });
    return script.status === 0 ? { samples: JSON.parse(script.stdout) } : { error: "could not run the original" };
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

const PRINT_SAMPLES = [
  "import sys, json, importlib, pkgutil",
  "sys.path.insert(0, sys.argv[1])",
  "import shopkit.repositories as repositories",
  "out = {}",
  "for info in pkgutil.iter_modules(repositories.__path__):",
  "    e = info.name",
  "    sample = getattr(importlib.import_module('shopkit.repositories.' + e), 'sample_' + e)()",
  "    out[e] = getattr(importlib.import_module('shopkit.serializers.' + e), 'serialize_' + e)(sample)",
  "print(json.dumps(out, sort_keys=True))",
].join("\n");

/**
 * Scores the task over two checks: unrelated files and serializer behavior.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, used for the task seed
 */
export async function verify(workspaceDir, runInfo) {
  const seed = runInfo?.task?.seed ?? 2614;
  const original = buildRepo(seed);
  const results = [];
  try { results.push(checkFiles(original, readFilesAsBuffers(workspaceDir))); } catch (error) { results.push(`file check crashed: ${error.message}`); }
  try {
    const before = dumpOriginalSamples(original);
    const after = dumpSerializers(workspaceDir);
    if (before.error) throw new Error(before.error);
    if (after.error) results.push(`the package does not run: ${after.error}`);
    else results.push(checkSerializers(before, after.value));
  } catch (error) { results.push(`serializer check crashed: ${error.message}`); }
  const failures = results.filter(Boolean);
  return { pass: failures.length === 0, score: (results.length - failures.length) / results.length, detail: failures.length ? failures.join("; ") : "Customer has loyalty_tier and nothing else changed" };
}

/**
 * Names the files that were added or removed compared with the original repository.
 * @param originalFiles - Map of the original paths
 * @param actual - Map of the paths in the workspace
 */
function describeFileSetChange(originalFiles, actual) {
  const added = [...actual.keys()].filter((p) => !originalFiles.has(p));
  const removed = [...originalFiles.keys()].filter((p) => !actual.has(p));
  return `the set of files changed: added ${added.slice(0, 5).join(", ") || "none"}; removed ${removed.slice(0, 5).join(", ") || "none"}`;
}

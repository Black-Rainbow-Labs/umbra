// Builds the records file and the 40 MB import log for the buried log line task.

import fs from "node:fs";
import path from "node:path";
import { mulberry32, randomInt, pick } from "./gen-utils.mjs";

export const LOG_LINE_COUNT = 380000;
export const RECORD_COUNT = 2000;
const WORKERS = ["worker-1", "worker-2", "worker-3", "worker-4", "worker-5", "worker-6"];
const STAGES = ["fetch", "decode", "validate", "merge", "index", "publish"];
const REASONS_MANUAL = ["schema_drift", "duplicate_key", "missing_owner", "timeout"];

/**
 * Returns the id of a record by position, for example rec-00042.
 * @param index - number from 1 to RECORD_COUNT
 */
export function recordId(index) {
  return `rec-${String(index).padStart(5, "0")}`;
}

/**
 * Builds the original records, all with status active except a spread of pending and archived ones.
 * @param seed - the fixed seed
 */
export function buildRecords(seed) {
  const rng = mulberry32(seed + 1);
  const records = [];
  for (let index = 1; index <= RECORD_COUNT; index += 1) {
    const status = pick(rng, ["active", "active", "active", "pending", "archived"]);
    records.push({ id: recordId(index), name: `${pick(rng, ["North", "South", "East", "West"])} ${pick(rng, ["depot", "store", "dock", "yard"])} ${index}`, status, amount: randomInt(rng, 100, 90000), updatedAt: `2026-02-${String(randomInt(rng, 10, 28))}` });
  }
  return records;
}

/**
 * Chooses the record that the one buried line is about, and the line number it sits on (near the middle).
 * @param seed - the fixed seed
 */
export function chooseTarget(seed) {
  const rng = mulberry32(seed + 2);
  const targetIndex = randomInt(rng, 200, RECORD_COUNT - 200);
  const lineNumber = Math.floor(LOG_LINE_COUNT * 0.5) + randomInt(rng, -9000, 9000);
  return { recordId: recordId(targetIndex), lineNumber };
}

/**
 * Formats a timestamp for a line number: the log covers one day at a steady pace.
 * @param lineNumber - zero based line number
 */
function timestampFor(lineNumber) {
  const totalMillis = Math.floor((lineNumber / LOG_LINE_COUNT) * 86399000);
  const pad = (value, width) => String(value).padStart(width, "0");
  const hours = Math.floor(totalMillis / 3600000);
  const minutes = Math.floor((totalMillis % 3600000) / 60000);
  const seconds = Math.floor((totalMillis % 60000) / 1000);
  return `2026-03-14T${pad(hours, 2)}:${pad(minutes, 2)}:${pad(seconds, 2)}.${pad(totalMillis % 1000, 3)}Z`;
}

/**
 * Builds the text of one ordinary or decoy log line (never the target line).
 * @param rng - seeded generator
 * @param stamp - the timestamp text
 */
function buildNoiseLine(rng, stamp) {
  const roll = rng();
  const record = recordId(randomInt(rng, 1, RECORD_COUNT));
  const worker = pick(rng, WORKERS);
  if (roll < 0.62) return `${stamp} INFO  [${worker}] stage=${pick(rng, STAGES)} record=${record} ok in ${randomInt(rng, 2, 400)}ms bytes=${randomInt(rng, 200, 90000)} batch=${randomInt(rng, 1, 9000)}`;
  if (roll < 0.8) return `${stamp} DEBUG [${worker}] cache hit ratio=${(rng() * 100).toFixed(2)} shard=${randomInt(rng, 0, 63)} keys=${randomInt(rng, 1000, 90000)} evictions=${randomInt(rng, 0, 40)}`;
  if (roll < 0.9) return `${stamp} WARN  [importer] reason=payload_corrupt record=${record} action=auto_retry attempt=${randomInt(rng, 1, 3)} result=recovered`;
  if (roll < 0.94) return `${stamp} ERROR [importer] reason=checksum_mismatch record=${record} action=retry_ok attempt=${randomInt(rng, 1, 4)}`;
  if (roll < 0.97) return `${stamp} ERROR [importer] reason=${pick(rng, REASONS_MANUAL)} record=${record} action=manual_fix owner=ops`;
  if (roll < 0.985) return `${stamp} ERROR [importer] reason=payload_corrupt record=${record} action=auto_retry attempt=${randomInt(rng, 1, 3)} result=recovered`;
  if (roll < 0.9958) return `${stamp} INFO  [importer] reason=payload_corrupt record=${record} action=manual_fix_done by=ops ticket=OPS-${randomInt(rng, 1000, 9999)}`;
  return `${stamp} ERROR [${worker}] unhandled exception in stage=${pick(rng, STAGES)} record=${record} message="connection reset by peer" retrying=true`;
}

/**
 * Builds the one line the task depends on.
 * @param stamp - the timestamp text
 * @param targetRecordId - the record the line is about
 */
export function buildTargetLine(stamp, targetRecordId) {
  return `${stamp} ERROR [importer] reason=payload_corrupt record=${targetRecordId} action=manual_fix owner=data-team detail="payload fails checksum on every retry"`;
}

/**
 * Writes the log file in chunks so the whole 40 MB is never held in memory at once.
 * @param filePath - destination file
 * @param seed - the fixed seed
 */
export function writeLog(filePath, seed) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const target = chooseTarget(seed);
  const rng = mulberry32(seed + 3);
  const fd = fs.openSync(filePath, "w");
  try {
    let chunk = [];
    for (let lineNumber = 0; lineNumber < LOG_LINE_COUNT; lineNumber += 1) {
      const stamp = timestampFor(lineNumber);
      chunk.push(lineNumber === target.lineNumber ? buildTargetLine(stamp, target.recordId) : buildNoiseLine(rng, stamp));
      if (chunk.length >= 20000) { fs.writeSync(fd, chunk.join("\n") + "\n"); chunk = []; }
    }
    if (chunk.length) fs.writeSync(fd, chunk.join("\n") + "\n");
  } finally {
    fs.closeSync(fd);
  }
}

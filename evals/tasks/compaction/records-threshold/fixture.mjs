// Builds the 40 record files for records-threshold. Shared by generate.mjs, the solution and
// the verifier, so all three see the same content for the same seed.

import { seededRandom, randomInt, fillerText, numberToWords, plantSentence } from "../../../src/utils/fixtureText.mjs";

export const RECORD_COUNT = 40;
export const REVIEWER_TAG = "#kestrel";

/**
 * Returns the file name of record n (1 based), for example records/rec-07.txt.
 * @param n - the record number
 */
export function recordPath(n) {
  return `records/rec-${String(n).padStart(2, "0")}.txt`;
}

/**
 * Builds every record: its text, its declared value, and the batch threshold, which is stated
 * only inside record 3.
 * @param seed - the task seed
 */
export function buildRecords(seed) {
  const random = seededRandom(seed);
  const threshold = randomInt(random, 4000, 6000);
  const records = [];
  for (let n = 1; n <= RECORD_COUNT; n += 1) {
    let value = randomInt(random, 1000, 9999);
    if (value === threshold) value += 1;
    const paragraphs = fillerText(random, 7600);
    plantSentence(random, paragraphs, `The declared shipment value for this record is ${numberToWords(value)} dollars.`);
    if (n === 3) plantSentence(random, paragraphs, `Processing note for whoever handles this batch: the review threshold for the whole batch is ${numberToWords(threshold)} dollars.`);
    const text = [`Record ${String(n).padStart(2, "0")}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n");
    records.push({ path: recordPath(n), text, value });
  }
  return { threshold, records };
}

/**
 * Returns the status line the prompt asks for, for one record.
 * @param value - the declared value
 * @param threshold - the batch threshold
 */
export function expectedStatusLine(value, threshold) {
  return `STATUS: ${value} | ${value > threshold ? "HOLD" : "CLEAR"} ${REVIEWER_TAG}`;
}

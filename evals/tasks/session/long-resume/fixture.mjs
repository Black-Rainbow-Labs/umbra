// Builds the 30 chapter files for long-resume. Each has one Marker line; together they make a
// history well over 32768 tokens when read one at a time.

import { seededRandom, randomInt, fillerText } from "../../../src/utils/fixtureText.mjs";

export const PASSPHRASE = "copper-lantern-58";
const MARKERS = ["anchor", "bramble", "cinder", "dapple", "eddy", "fennel", "gossamer", "hollow", "ivory", "jasper", "kestrel", "lichen", "marrow", "nimbus", "oriel", "pewter", "quill", "russet", "sorrel", "thistle", "umbrel", "verdant", "wicket", "yonder", "zinnia", "argent", "beryl", "corvid", "damask", "ember"];

/**
 * Builds every chapter with its marker.
 * @param seed - the task seed
 */
export function buildChapters(seed) {
  const random = seededRandom(seed);
  return MARKERS.map((marker, i) => {
    const paragraphs = fillerText(random, 8600);
    paragraphs.splice(randomInt(random, 1, paragraphs.length - 1), 0, `Marker: ${marker}`);
    return { path: `chapters/ch-${String(i + 1).padStart(2, "0")}.txt`, marker, text: [`Chapter ${i + 1}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n") };
  });
}

/**
 * Returns the expected summary.json for the chapters.
 * @param chapters - the result of buildChapters
 */
export function expectedSummary(chapters) {
  return { passphrase: PASSPHRASE, marker_count: chapters.length, first_marker: chapters[0].marker, last_marker: chapters[chapters.length - 1].marker };
}

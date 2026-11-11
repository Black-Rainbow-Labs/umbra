// Builds the 24 note files for notes-16k-roster. Each note names one attendee. The project code
// is stated only in note 1 and the coordinator only in note 2. Shared by generate.mjs, the
// solution and the verifier.

import { seededRandom, randomInt, fillerText, plantSentence } from "../../../src/utils/fixtureText.mjs";

export const NOTE_COUNT = 24;
export const ROSTER_TAG = "teal";
const FIRST_NAMES = ["Dana", "Marcus", "Priya", "Tobias", "Lena", "Omar", "Ingrid", "Felix", "Nadia", "Hugo", "Sofia", "Kenji", "Marta", "Elias", "Noor", "Viktor", "Alma", "Rafael", "Greta", "Soren", "Yara", "Lucian", "Petra", "Idris"];
const SURNAMES = ["Harlow", "Okafor", "Lindqvist", "Brandt", "Castellan", "Nakamura", "Duval", "Ferreira", "Voss", "Mbeki", "Rinaldi", "Aldous", "Kowalczyk", "Thorne", "Haddad", "Ostrander", "Quill", "Varga", "Winslow", "Pereira", "Sandoval", "Ekstrand", "Maddox", "Bellweather"];

export const FACTS = { project_code: "KT-7392-RV", coordinator: "Imogen Fairweather" };

/**
 * Returns the path of note n (1 based), for example notes/part-07.md.
 * @param n - the note number
 */
export function notePath(n) {
  return `notes/part-${String(n).padStart(2, "0")}.md`;
}

/**
 * Builds the roster line the prompt asks for.
 * @param n - the file number
 * @param first - the attendee's first name
 * @param last - the attendee's surname
 */
export function rosterLine(n, first, last) {
  return `${String(n).padStart(2, "0")} | ${last.toUpperCase()}, ${first} | ${ROSTER_TAG}`;
}

/**
 * Builds every note with its attendee. Notes 1 and 2 carry the two facts.
 * @param seed - the task seed
 */
export function buildNotes(seed) {
  const random = seededRandom(seed);
  const notes = [];
  for (let n = 1; n <= NOTE_COUNT; n += 1) {
    const first = FIRST_NAMES[n - 1];
    const last = SURNAMES[n - 1];
    const paragraphs = fillerText(random, 5900);
    if (n === 1) plantSentence(random, paragraphs, `The project code for this whole workshop series is ${FACTS.project_code}.`);
    if (n === 2) plantSentence(random, paragraphs, `The project coordinator for the series is ${FACTS.coordinator}.`);
    paragraphs.splice(randomInt(random, 1, paragraphs.length - 1), 0, `Attendee: ${first} ${last}`);
    notes.push({ path: notePath(n), first, last, text: [`# Workshop notes, part ${n}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n") });
  }
  return notes;
}

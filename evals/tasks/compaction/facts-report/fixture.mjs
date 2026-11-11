// Builds the 30 note files for facts-report. Five facts are planted in the first three notes and
// every note has one keyword. Shared by generate.mjs, the solution and the verifier.

import { seededRandom, randomInt, fillerText, plantSentence } from "../../../src/utils/fixtureText.mjs";

export const NOTE_COUNT = 30;
const KEYWORDS = ["amber", "basalt", "cedar", "dune", "ember", "fjord", "garnet", "harbor", "indigo", "juniper", "kelp", "lagoon", "meadow", "nectar", "onyx", "prairie", "quartz", "reef", "sierra", "tundra", "umber", "valley", "willow", "xenon", "yarrow", "zephyr", "alder", "birch", "cobalt", "delta"];

export const FACTS = {
  vault_code: "4471-QX",
  sponsor: "Dana Whitcombe",
  launch_city: "Valparaiso",
  backup_host: "orchid-17",
  budget_cap: 83250,
};

const FACT_SENTENCES = [
  [1, `For the record, the vault code for the archive room is ${FACTS.vault_code}.`],
  [1, `The executive sponsor of this project is ${FACTS.sponsor}.`],
  [2, `It was agreed that the launch will take place in the city of ${FACTS.launch_city}.`],
  [3, `If the primary fails, traffic moves to the backup server whose hostname is ${FACTS.backup_host}.`],
  [3, `Finance confirmed a budget cap of 83,250 dollars for the whole project.`],
];

/**
 * Returns the path of note n (1 based), for example notes/part-07.md.
 * @param n - the note number
 */
export function notePath(n) {
  return `notes/part-${String(n).padStart(2, "0")}.md`;
}

/**
 * Builds every note with its keyword. The five facts are planted in notes 1 to 3.
 * @param seed - the task seed
 */
export function buildNotes(seed) {
  const random = seededRandom(seed);
  const notes = [];
  for (let n = 1; n <= NOTE_COUNT; n += 1) {
    const keyword = KEYWORDS[n - 1];
    const paragraphs = fillerText(random, 7800);
    for (const [target, sentence] of FACT_SENTENCES) if (target === n) plantSentence(random, paragraphs, sentence);
    const keywordAt = randomInt(random, 1, paragraphs.length - 1);
    paragraphs.splice(keywordAt, 0, `Keyword: ${keyword}`);
    notes.push({ path: notePath(n), keyword, text: [`# Meeting notes, part ${n}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n") });
  }
  return notes;
}

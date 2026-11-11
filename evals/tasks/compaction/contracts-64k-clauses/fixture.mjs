// Builds the 36 contract files for contracts-64k-clauses. Each contract names one governing
// clause and sometimes mentions withdrawn clauses as decoys. Shared by generate.mjs, the
// solution and the verifier.

import { seededRandom, randomInt, fillerText, plantSentence } from "../../../src/utils/fixtureText.mjs";

export const CONTRACT_COUNT = 36;
export const EXEMPT = [5, 12, 19, 27, 33];

/**
 * Returns the path of contract n, for example contracts/contract-07.txt.
 * @param n - the contract number
 */
export function contractPath(n) {
  return `contracts/contract-${String(n).padStart(2, "0")}.txt`;
}

/**
 * Builds the report line for one contract, applying the exception list.
 * @param n - the contract number
 * @param clause - the governing clause id, for example CL-417
 */
export function reportLine(n, clause) {
  return `contract-${String(n).padStart(2, "0")}: ${EXEMPT.includes(n) ? "EXEMPT" : clause}`;
}

/**
 * Builds the full expected report as a list of lines.
 * @param contracts - the contracts from buildContracts
 */
export function expectedReport(contracts) {
  const lines = ["CLAUSE REPORT", ...contracts.map((c, i) => reportLine(i + 1, c.clause))];
  lines.push(`TOTAL: ${contracts.length - EXEMPT.length}`);
  return lines;
}

/**
 * Builds every contract with its governing clause id. Clause ids are unique.
 * @param seed - the task seed
 */
export function buildContracts(seed) {
  const random = seededRandom(seed);
  const used = new Set();
  const contracts = [];
  for (let n = 1; n <= CONTRACT_COUNT; n += 1) {
    let number = randomInt(random, 100, 899);
    while (used.has(number)) number += 1;
    used.add(number);
    const paragraphs = fillerText(random, 11000);
    plantSentence(random, paragraphs, `The governing clause of this contract is CL-${number}.`);
    const decoys = randomInt(random, 0, 2);
    for (let d = 0; d < decoys; d += 1) plantSentence(random, paragraphs, `Clause CL-${randomInt(random, 900, 999)} was withdrawn before signing and does not govern.`);
    contracts.push({ path: contractPath(n), clause: `CL-${number}`, text: [`Contract ${n}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n") });
  }
  return contracts;
}

// Builds the ten CSV files for interrupt-continue and the JSON each must become.

import { seededRandom, randomInt } from "../../../src/utils/fixtureText.mjs";

const NAMES = ["Ada", "Bram", "Cleo", "Dev", "Esme", "Finn", "Gia", "Hugo", "Iris", "Jon", "Kaia", "Leo", "Mila", "Nico", "Orla", "Pia"];
export const FILES = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet"];

/**
 * Builds every CSV text and its expected rows.
 * @param seed - the task seed
 */
export function buildTables(seed) {
  const random = seededRandom(seed);
  return FILES.map((name) => {
    const rows = Array.from({ length: randomInt(random, 4, 7) }, (_, i) => ({ id: randomInt(random, 100, 999) * 10 + i, name: NAMES[randomInt(random, 0, NAMES.length - 1)], active: random() < 0.5, score: randomInt(random, 0, 1000) / 10 }));
    const csv = ["id,name,active,score", ...rows.map((r) => `${r.id},${r.name},${r.active ? "yes" : "no"},${r.score.toFixed(1)}`)].join("\n") + "\n";
    return { name, csv, rows };
  });
}

/**
 * Returns the expected index.json content.
 * @param tables - the result of buildTables
 */
export function expectedIndex(tables) {
  return { files: tables.map((t) => `${t.name}.json`), total_rows: tables.reduce((sum, t) => sum + t.rows.length, 0) };
}

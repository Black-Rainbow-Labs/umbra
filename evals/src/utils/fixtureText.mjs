// Deterministic text for generated fixtures: a seeded random generator, filler prose, and
// numbers written in words.

const WORDS = ["the", "warehouse", "team", "reviewed", "pallet", "routing", "schedule", "quarterly", "audit", "notes", "carrier", "invoice", "dock", "inventory", "supplier", "contract", "freight", "manifest", "regional", "office", "delivery", "window", "forecast", "storage", "handling", "label", "count", "shift", "report", "checked", "updated", "pending", "approved", "transfer", "container", "weight", "measured", "sorted", "received", "outbound", "inbound", "staff", "meeting", "minutes", "discussed", "vendor", "estimate", "policy", "safety", "training", "equipment", "forklift", "aisle", "bay", "rack", "scanner", "batch", "cycle", "variance", "summary"];
const ONES = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"];
const TENS = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

/**
 * Returns a seeded pseudo random generator (mulberry32) that yields floats in [0, 1).
 * @param seed - an integer seed
 */
export function seededRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Returns a random integer between min and max, inclusive.
 * @param random - a generator from seededRandom
 * @param min - lowest value
 * @param max - highest value
 */
export function randomInt(random, min, max) {
  return min + Math.floor(random() * (max - min + 1));
}

/**
 * Builds one filler sentence of ordinary looking words.
 * @param random - a generator from seededRandom
 */
export function fillerSentence(random) {
  const length = randomInt(random, 9, 18);
  const words = Array.from({ length }, () => WORDS[Math.floor(random() * WORDS.length)]);
  const text = words.join(" ");
  return text.charAt(0).toUpperCase() + text.slice(1) + ".";
}

/**
 * Builds filler paragraphs until the text reaches about `chars` characters.
 * @param random - a generator from seededRandom
 * @param chars - target length in characters
 */
export function fillerText(random, chars) {
  const paragraphs = [];
  let total = 0;
  while (total < chars) {
    const sentences = Array.from({ length: randomInt(random, 4, 7) }, () => fillerSentence(random));
    const paragraph = sentences.join(" ");
    paragraphs.push(paragraph);
    total += paragraph.length + 2;
  }
  return paragraphs;
}

/**
 * Writes a whole number from 0 to 999,999 in English words, for example 4352 as
 * "four thousand three hundred fifty-two".
 * @param value - the number
 */
export function numberToWords(value) {
  if (value < 20) return ONES[value];
  if (value < 100) return TENS[Math.floor(value / 10)] + (value % 10 ? `-${ONES[value % 10]}` : "");
  if (value < 1000) return `${ONES[Math.floor(value / 100)]} hundred` + (value % 100 ? ` ${numberToWords(value % 100)}` : "");
  return `${numberToWords(Math.floor(value / 1000))} thousand` + (value % 1000 ? ` ${numberToWords(value % 1000)}` : "");
}

/**
 * Inserts a sentence into a list of paragraphs at a random paragraph after the first.
 * @param random - a generator from seededRandom
 * @param paragraphs - the paragraphs, changed in place
 * @param sentence - the sentence to add at the end of the chosen paragraph
 */
export function plantSentence(random, paragraphs, sentence) {
  const index = paragraphs.length > 1 ? randomInt(random, 1, paragraphs.length - 1) : 0;
  paragraphs[index] = `${paragraphs[index]} ${sentence}`;
  return index;
}

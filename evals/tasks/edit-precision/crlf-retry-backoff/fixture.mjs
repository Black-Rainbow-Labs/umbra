// Builds a large JavaScript module whose lines all end in CRLF.

export const FILE_NAME = "retry.mjs";
export const TARGET_TEXT = [
  "/**",
  " * Returns how long to wait before the given retry attempt.",
  " * @param attempt - zero based retry number",
  " */",
  "export function backoffDelayMs(attempt) {",
  "  const base = 250;",
  "  return base * Math.pow(2, attempt);",
  "}",
  "",
].join("\r\n");

/**
 * Returns a deterministic pseudo random number generator.
 * @param seed - integer seed
 */
function mulberry32(seed) {
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
 * Builds the lines of one filler function.
 * @param index - function number
 * @param random - the generator
 */
function fillerFunction(index, random) {
  const width = 4 + Math.floor(random() * 12);
  const limit = 10 + Math.floor(random() * 90);
  return [
    "/**",
    ` * Describes pipeline step ${index} for the status page.`,
    " * @param value - the input value",
    " */",
    `export function describeStep${index}(value) {`,
    "  const text = String(value);",
    `  const padded = text.padStart(${width}, " ");`,
    `  const clipped = padded.slice(0, ${limit});`,
    "  const parts = [];",
    `  parts.push("step${index}");`,
    "  parts.push(clipped);",
    `  if (text.length > ${limit}) parts.push("clipped");`,
    `  if (text.length === 0) parts.push("empty");`,
    "  const checksum = [...text].reduce((sum, ch) => (sum + ch.charCodeAt(0)) % 997, 0);",
    "  parts.push(String(checksum));",
    "  const label = parts.join(\":\");",
    "  return label.toLowerCase();",
    "}",
    "",
  ].join("\r\n");
}

/**
 * Builds the whole module text with CRLF line endings.
 * @param seed - integer seed
 */
export function buildRetryModule(seed) {
  const random = mulberry32(seed);
  const parts = ["// Retry timing and pipeline status helpers.\r\n\r\n"];
  for (let index = 1; index <= 90; index += 1) {
    if (index === 45) parts.push(TARGET_TEXT + "\r\n");
    parts.push(fillerFunction(index, random) + "\r\n");
  }
  return parts.join("");
}

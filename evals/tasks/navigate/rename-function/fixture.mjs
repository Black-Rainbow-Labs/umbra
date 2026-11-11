// Builds the generated Node repository for the rename task, in its original or solved form.

import { mulberry32, randomInt, pick, shuffled, toCamel } from "./gen-utils.mjs";

export const OLD_NAME = "calcDiscount";
export const NEW_NAME = "computeDiscount";

const DOMAINS = ["orders", "catalog", "customers", "shipping", "inventory", "billing", "returns", "loyalty", "reports", "vendors", "promotions", "support", "pricing"];
const ADJECTIVES = ["bulk", "daily", "fast", "late", "open", "prime", "quick", "regional", "safe", "tiered", "unit", "weekly", "active", "batch", "core", "draft"];
const NOUNS = ["lookup", "ledger", "queue", "window", "ticket", "rollup", "digest", "cursor", "bucket", "filter", "router", "mapper", "policy", "cache", "audit", "index"];

/**
 * Picks unique module names for one domain from adjective and noun pairs.
 * @param rng - seeded generator
 * @param count - how many names to produce
 */
function moduleNamesFor(rng, count) {
  const pairs = [];
  for (const adjective of ADJECTIVES) for (const noun of NOUNS) pairs.push(`${adjective}-${noun}`);
  return shuffled(rng, pairs).slice(0, count);
}

/**
 * Describes which modules call the old function and in which way. Keys are "domainIndex:moduleIndex".
 * @param rng - seeded generator
 */
function chooseCallerSpecs(rng) {
  const variants = ["named", "alias", "namespace", "rate-arg", "named", "named", "alias", "namespace", "comment-only", "rate-only", "string-note", "comment-only", "named"];
  const specs = new Map();
  let index = 0;
  while (specs.size < variants.length) {
    const key = `${randomInt(rng, 0, DOMAINS.length - 2)}:${randomInt(rng, 2, 13)}`;
    if (!specs.has(key)) specs.set(key, variants[index++]);
  }
  return specs;
}

/**
 * Returns the import lines, the extra expression for the weight function and decoy lines for a caller variant.
 * @param variant - one of the caller variants
 * @param fn - the name of the function being renamed (old or new)
 */
function callerPieces(variant, fn) {
  const barrel = '"../pricing/index.mjs"';
  if (variant === "named") return { imports: [`import { ${fn} } from ${barrel};`], extra: ` + ${fn}(value, 10)`, decoys: [] };
  if (variant === "alias") return { imports: [`import { ${fn} as discountFor } from "../pricing/discount.mjs";`], extra: " + discountFor(value, 15)", decoys: [] };
  if (variant === "namespace") return { imports: [`import * as pricing from ${barrel};`], extra: ` + pricing.${fn}(value, 5)`, decoys: [] };
  if (variant === "rate-arg") return { imports: [`import { ${fn}, calcDiscountRate } from ${barrel};`], extra: ` + ${fn}(value, calcDiscountRate("gold"))`, decoys: [] };
  if (variant === "rate-only") return { imports: [`import { calcDiscountRate } from ${barrel};`], extra: ' + calcDiscountRate("silver")', decoys: ['import { calcDiscountRate } from "../pricing/index.mjs";', ' + calcDiscountRate("silver")'] };
  return { imports: [], extra: "", decoys: [] };
}

/**
 * Builds the text of one generated module and the decoy lines it must keep.
 * @param ctx - { domain, name, index, previous, variant, fn }
 * @param rng - seeded generator
 */
function buildModule(ctx, rng) {
  const { domain, name, previous, variant, fn } = ctx;
  const camel = toCamel(name);
  const pieces = callerPieces(variant, fn);
  const decoys = [...pieces.decoys];
  const lines = [`// ${domain}/${name}: ${name.replace("-", " ")} helpers for the ${domain} area.`];
  if (variant === "comment-only") {
    const note = "// calcDiscount runs in the pricing area before tax, so nothing here applies it twice.";
    lines.push(note); decoys.push(note);
  }
  if (variant === "string-note") {
    const note = 'export const DISCOUNT_NOTE = "see calcDiscount in pricing/discount.mjs";';
    lines.push(note); decoys.push(note);
  }
  if (previous) lines.push(`import { ${toCamel(previous)}Weight } from "./${previous}.mjs";`);
  lines.push(...pieces.imports, "");
  lines.push("/**", ` * Scores a value for the ${name} step.`, " * @param value - the number to score", " */");
  lines.push(`export function ${camel}Score(value) {`, `  return (value * ${randomInt(rng, 3, 19)} + ${randomInt(rng, 1, 90)}) % ${randomInt(rng, 101, 997)};`, "}", "");
  const chain = previous ? `${toCamel(previous)}Weight(value) % 97` : "1";
  lines.push("/**", ` * Weighs a value for the ${name} step using the earlier steps of the ${domain} area.`, " * @param value - the number to weigh", " */");
  lines.push(`export function ${camel}Weight(value) {`, `  return ${camel}Score(value) + ${chain}${pieces.extra};`, "}", "");
  return { text: lines.join("\n"), decoys };
}

/**
 * Builds the pricing discount module and its barrel, with the renamed function in solved form.
 * @param fn - the name of the function being renamed (old or new)
 */
function buildDiscountFiles(fn) {
  const discount = [
    "// Discount helpers for the pricing area.",
    "",
    "/**",
    " * Returns the discount for an amount. The reports area also calls calcDiscount for its totals.",
    " * @param amount - the amount in cents",
    " * @param percent - the discount percent",
    " */",
    `export function ${fn}(amount, percent) {`,
    '  if (amount < 0) throw new Error("calcDiscount: amount must not be negative");',
    "  return Math.round((amount * percent) / 100);",
    "}",
    "",
    "/**",
    " * Returns the discount percent for a loyalty tier.",
    " * @param tier - the tier name",
    " */",
    "export function calcDiscountRate(tier) {",
    '  return { gold: 15, silver: 10 }[tier] ?? 0;',
    "}",
    "",
    "/**",
    " * Returns the amount left after the discount.",
    " * @param amount - the amount in cents",
    " * @param percent - the discount percent",
    " */",
    "export function calcDiscountedTotal(amount, percent) {",
    `  return amount - ${fn}(amount, percent);`,
    "}",
    "",
  ].join("\n");
  const barrel = `export { ${fn}, calcDiscountRate, calcDiscountedTotal } from "./discount.mjs";\n`;
  const decoys = [" * Returns the discount for an amount. The reports area also calls calcDiscount for its totals.", '  if (amount < 0) throw new Error("calcDiscount: amount must not be negative");', "export function calcDiscountRate(tier) {"];
  return { discount, barrel, decoys };
}

/**
 * Builds the two test files and the documentation files.
 * @param fn - the name of the function being renamed (old or new)
 */
function buildTestAndDocFiles(fn) {
  const files = new Map();
  files.set("test/discount.test.mjs", [
    'import test from "node:test";', 'import assert from "node:assert/strict";', `import { ${fn} } from "../src/pricing/index.mjs";`, "",
    `test("discount of 10 percent", () => { assert.equal(${fn}(2000, 10), 200); });`,
    `test("discount of zero", () => { assert.equal(${fn}(999, 0), 0); });`, "",
  ].join("\n"));
  files.set("test/pricing-totals.test.mjs", [
    'import test from "node:test";', 'import assert from "node:assert/strict";', `import { ${fn}, calcDiscountedTotal } from "../src/pricing/index.mjs";`, "",
    `test("total after discount", () => { assert.equal(calcDiscountedTotal(1000, 25), 1000 - ${fn}(1000, 25)); });`, "",
  ].join("\n"));
  const readmeLine = "The `calcDiscount` function is documented in the pricing notes.";
  files.set("README.md", `# Storefront modules\n\nGenerated sample repository.\n\n${readmeLine}\n`);
  files.set("docs/pricing.md", "# Pricing\n\nSee `calcDiscountRate` for tier percentages.\n");
  files.set("package.json", '{ "name": "storefront-modules", "private": true, "type": "module" }\n');
  return { files, decoys: [readmeLine] };
}

/**
 * Builds the whole repository in memory.
 * @param seed - the fixed seed
 * @param options - { solved } when true the function carries its new name everywhere it is used as code
 */
export function buildRepo(seed, options = {}) {
  const rng = mulberry32(seed);
  const fn = options.solved ? NEW_NAME : OLD_NAME;
  const specs = chooseCallerSpecs(rng);
  const files = new Map();
  const decoyLines = [];
  const touched = new Set(["src/pricing/discount.mjs", "src/pricing/index.mjs", "test/discount.test.mjs", "test/pricing-totals.test.mjs"]);
  const runners = [];
  DOMAINS.forEach((domain, domainIndex) => {
    const names = moduleNamesFor(rng, 14);
    names.forEach((name, index) => {
      const variant = specs.get(`${domainIndex}:${index}`) || "plain";
      const built = buildModule({ domain, name, index, previous: names[index - 1], variant, fn }, rng);
      files.set(`src/${domain}/${name}.mjs`, built.text);
      if (variant !== "plain" && variant !== "rate-only" && variant !== "comment-only" && variant !== "string-note") touched.add(`src/${domain}/${name}.mjs`);
      decoyLines.push(...built.decoys);
    });
    const last = toCamel(names[names.length - 1]);
    files.set(`src/${domain}/run.mjs`, `import { ${last}Weight } from "./${names[names.length - 1]}.mjs";\n\nexport function run${domain[0].toUpperCase()}${domain.slice(1)}(value) {\n  return ${last}Weight(value);\n}\n`);
    runners.push(domain);
  });
  const discountFiles = buildDiscountFiles(fn);
  files.set("src/pricing/discount.mjs", discountFiles.discount);
  files.set("src/pricing/index.mjs", discountFiles.barrel);
  decoyLines.push(...discountFiles.decoys);
  const extra = buildTestAndDocFiles(fn);
  for (const [file, text] of extra.files) files.set(file, text);
  decoyLines.push(...extra.decoys);
  files.set("main.mjs", buildMain(runners));
  return { files, decoyLines, touched };
}

/**
 * Builds the entry script that runs every domain and prints the results as JSON.
 * @param domains - the domain folder names
 */
function buildMain(domains) {
  const names = domains.map((domain) => `run${domain[0].toUpperCase()}${domain.slice(1)}`);
  const imports = domains.map((domain, index) => `import { ${names[index]} } from "./src/${domain}/run.mjs";`);
  const body = names.map((name) => `  ${name}: [${name}(7), ${name}(250), ${name}(12345)],`);
  return [...imports, "", "const results = {", ...body, "};", "console.log(JSON.stringify(results));", ""].join("\n");
}

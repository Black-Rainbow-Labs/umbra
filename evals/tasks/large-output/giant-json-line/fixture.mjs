// Builds the catalog object that is written as one 8 MB line of JSON.

import { mulberry32, randomInt, pick } from "./gen-utils.mjs";

export const TARGET_STORE = "store-0027";
export const TARGET_SECTION = "frozen";
export const TARGET_SKU = "SKU-4411";
export const NEW_PRICE = 24.99;
const STORE_COUNT = 40;
const ITEMS_PER_SECTION = 72;
const SECTIONS = ["frozen", "dairy", "bakery", "produce", "snacks", "drinks", "canned", "pasta", "sauces", "spices", "cereal", "candy", "paper", "cleaning", "pets", "baby", "health", "beauty", "deli", "seafood", "meat", "organic", "bulk", "seasonal", "household"];
const WORDS = ["quick", "prime", "tiered", "bulk", "daily", "open", "fresh", "plain"];
const TAGS = ["seasonal", "bulk", "organic", "import", "local", "clearance", "new"];

/**
 * Builds the pool of 400 skus that items are drawn from, so every sku appears in many places.
 */
function skuPool() {
  return Array.from({ length: 400 }, (_, index) => `SKU-${4000 + index * 3 + (index % 7)}`);
}

/**
 * Builds one item with a sku and some nested detail.
 * @param rng - seeded generator
 * @param sku - the item sku
 */
function buildItem(rng, sku) {
  return {
    sku,
    name: `${pick(rng, WORDS)} ${pick(rng, WORDS)} ${randomInt(rng, 1, 99)}`,
    price: randomInt(rng, 100, 9999) / 100,
    stock: randomInt(rng, 0, 400),
    tags: [pick(rng, TAGS), pick(rng, TAGS)],
    dims: { w: randomInt(rng, 2, 40), h: randomInt(rng, 2, 40), d: randomInt(rng, 1, 20) },
  };
}

/**
 * Builds one section. When it is the target section the target sku appears exactly once, at a fixed position.
 * @param rng - seeded generator
 * @param name - the section name
 * @param isTarget - true for the target store and section
 */
function buildSection(rng, name, isTarget) {
  const pool = skuPool();
  const items = [];
  for (let index = 0; index < ITEMS_PER_SECTION; index += 1) {
    let sku = pick(rng, pool);
    if (isTarget) while (sku === TARGET_SKU) sku = pick(rng, pool);
    items.push(buildItem(rng, sku));
  }
  if (isTarget) items[41] = buildItem(rng, TARGET_SKU);
  return { name, items };
}

/**
 * Builds the whole catalog. The target sku is in the pool, so it also appears in other sections and stores.
 * @param seed - the fixed seed
 */
export function buildCatalog(seed) {
  const rng = mulberry32(seed);
  const stores = [];
  for (let storeNumber = 1; storeNumber <= STORE_COUNT; storeNumber += 1) {
    const id = `store-${String(storeNumber).padStart(4, "0")}`;
    const sections = SECTIONS.map((name) => buildSection(rng, name, id === TARGET_STORE && name === TARGET_SECTION));
    stores.push({ id, city: pick(rng, ["Austin", "Boise", "Cary", "Dayton", "Eugene"]), sections });
  }
  return { meta: { version: 3, generated: "2026-03-14", currency: "USD" }, stores };
}

/**
 * Returns the target item of a catalog object.
 * @param catalog - a catalog object
 */
export function findTargetItem(catalog) {
  const store = catalog.stores.find((entry) => entry.id === TARGET_STORE);
  const section = store.sections.find((entry) => entry.name === TARGET_SECTION);
  return section.items.find((item) => item.sku === TARGET_SKU);
}

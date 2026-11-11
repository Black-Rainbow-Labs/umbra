import { slugify } from "../vendor/slugify.mjs";

/**
 * Slugs text after replacing accented letters with their plain letters.
 * @param text - the text to convert
 */
export function slugifyPlain(text) {
  return slugify(String(text).normalize("NFD").replace(/[\u0300-\u036f]/g, ""));
}

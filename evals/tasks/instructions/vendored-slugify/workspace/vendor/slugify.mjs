// Vendored from slugify-lite 2.4.1. Do not edit: this file is replaced when we re-vendor.

/**
 * Lowercases text and joins runs of letters and digits with single hyphens.
 * @param text - the text to convert
 */
export function slugify(text) {
  return String(text)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

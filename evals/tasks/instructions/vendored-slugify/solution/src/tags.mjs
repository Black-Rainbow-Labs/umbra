import { slugifyPlain } from "./slug.mjs";

/**
 * Builds the URL path of a tag page from the tag name.
 * @param name - the tag name
 */
export function tagUrl(name) {
  return `/tags/${slugifyPlain(name)}`;
}

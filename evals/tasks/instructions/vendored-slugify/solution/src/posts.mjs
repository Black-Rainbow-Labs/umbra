import { slugifyPlain } from "./slug.mjs";

/**
 * Builds the URL path of a blog post from its title.
 * @param title - the post title
 */
export function postUrl(title) {
  return `/posts/${slugifyPlain(title)}`;
}

import { deepClone } from "./clone.mjs";

/**
 * Returns a copy of the config with a bearer token in the headers. Without a token the copy is
 * unchanged. The argument is never modified.
 * @param config - the request settings
 * @param token - the access token, if the request has one
 */
export function withAuth(config, token) {
  const next = deepClone(config);
  if (token) next.headers.authorization = `Bearer ${token}`;
  return next;
}

/**
 * Returns a copy of the config with more headers. When a header is already set, the new value wins.
 * Neither argument is modified.
 * @param config - the request settings
 * @param headers - an object of header names and values
 */
export function withHeaders(config, headers) {
  const next = deepClone(config);
  Object.assign(next.headers, deepClone(headers));
  return next;
}

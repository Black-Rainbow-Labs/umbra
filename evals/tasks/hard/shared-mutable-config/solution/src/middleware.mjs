/**
 * Returns a copy of the config with a bearer token in the headers. Without a token the copy is
 * unchanged. The argument is never modified.
 * @param config - the request settings
 * @param token - the access token, if the request has one
 */
export function withAuth(config, token) {
  const next = structuredClone(config);
  if (token) next.headers.authorization = `Bearer ${token}`;
  return next;
}

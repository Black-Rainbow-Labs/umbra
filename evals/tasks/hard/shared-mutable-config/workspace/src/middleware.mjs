/**
 * Adds a bearer token to the request headers. Without a token the config is left as it is.
 * @param config - the request settings
 * @param token - the access token, if the request has one
 */
export function withAuth(config, token) {
  if (token) config.headers.authorization = `Bearer ${token}`;
  return config;
}

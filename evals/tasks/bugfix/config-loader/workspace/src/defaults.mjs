// The settings used when a config file says nothing about them.

export const DEFAULTS = {
  debug: false,
  name: "service",
  server: { host: "localhost", port: 8080, tls: false },
  cache: { enabled: true, ttlSeconds: 300 },
};

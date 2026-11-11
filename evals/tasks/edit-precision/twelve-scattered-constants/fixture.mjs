// Builds a large settings module of exported constants: 30 subsystems with 20 settings each.

export const FILE_NAME = "settings.mjs";

const SUBSYSTEMS = ["auth", "billing", "catalog", "checkout", "cms", "search", "reports", "inventory", "shipping", "returns", "profile", "notify", "audit", "export", "import", "sync", "metrics", "admin", "gateway", "storage", "queue", "mailer", "pricing", "tax", "support", "orders", "payments", "docs", "media", "jobs"];
const PROPERTIES = ["MAX_RETRIES", "TIMEOUT_MS", "BATCH_SIZE", "CACHE_TTL_SECONDS", "ENABLED", "DEBUG", "LOG_LEVEL", "ENDPOINT", "POOL_SIZE", "BACKOFF_FACTOR", "ALLOWED_METHODS", "REGION", "RATE_LIMIT_PER_MINUTE", "QUEUE_DEPTH", "SHARD_COUNT", "COMPRESSION", "RETRY_JITTER", "HEARTBEAT_MS", "MAX_PAYLOAD_BYTES", "CHECKSUM"];

/** The twelve requested changes: constant name and the new value as JSON text. */
export const CHANGES = [
  ["AUTH_MAX_RETRIES", "11"],
  ["BILLING_TIMEOUT_MS", "45000"],
  ["CATALOG_BATCH_SIZE", "750"],
  ["SEARCH_CACHE_TTL_SECONDS", "7200"],
  ["SHIPPING_ENABLED", "false"],
  ["REPORTS_DEBUG", "true"],
  ["INVENTORY_LOG_LEVEL", "\"trace\""],
  ["GATEWAY_ENDPOINT", "\"https://gw.internal.example/v9\""],
  ["QUEUE_POOL_SIZE", "64"],
  ["MAILER_BACKOFF_FACTOR", "3.5"],
  ["STORAGE_ALLOWED_METHODS", "[\"GET\",\"HEAD\",\"OPTIONS\"]"],
  ["TAX_REGION", "\"eu-north-1\""],
];

/**
 * Returns a deterministic pseudo random number generator.
 * @param seed - integer seed
 */
function mulberry32(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Picks one element of a list using the generator.
 * @param random - the generator
 * @param list - the candidates
 */
function pick(random, list) {
  return list[Math.floor(random() * list.length)];
}

/**
 * Builds the value of one setting as JSON text.
 * @param subsystem - subsystem name
 * @param property - property name
 * @param random - the generator
 */
function buildValue(subsystem, property, random) {
  const integer = (low, high) => String(low + Math.floor(random() * (high - low)));
  const table = {
    MAX_RETRIES: () => integer(1, 9), TIMEOUT_MS: () => integer(5, 40) * 1000, BATCH_SIZE: () => integer(10, 500),
    CACHE_TTL_SECONDS: () => integer(1, 24) * 300, ENABLED: () => "true", DEBUG: () => "false",
    LOG_LEVEL: () => JSON.stringify(pick(random, ["info", "warn", "error", "debug"])),
    ENDPOINT: () => JSON.stringify(`https://${subsystem}.internal.example/v1`), POOL_SIZE: () => integer(4, 33),
    BACKOFF_FACTOR: () => String(pick(random, [1.2, 1.5, 1.8, 2, 2.2])),
    ALLOWED_METHODS: () => JSON.stringify(pick(random, [["GET"], ["GET", "POST"], ["GET", "POST", "PUT"]])),
    REGION: () => JSON.stringify(pick(random, ["us-east-1", "us-west-2", "eu-west-1", "ap-south-1"])),
    RATE_LIMIT_PER_MINUTE: () => integer(1, 60) * 10, QUEUE_DEPTH: () => integer(100, 5000), SHARD_COUNT: () => integer(1, 17),
    COMPRESSION: () => JSON.stringify(pick(random, ["none", "gzip", "zstd"])), RETRY_JITTER: () => "true",
    HEARTBEAT_MS: () => integer(1, 30) * 500, MAX_PAYLOAD_BYTES: () => integer(1, 64) * 1024,
    CHECKSUM: () => JSON.stringify(pick(random, ["crc32", "sha1", "sha256"])),
  };
  return table[property]();
}

/**
 * Builds the whole settings module text.
 * @param seed - integer seed
 */
export function buildSettings(seed) {
  const random = mulberry32(seed);
  const lines = ["// Service settings. One block per subsystem.", ""];
  for (const subsystem of SUBSYSTEMS) {
    lines.push(`// ---- ${subsystem} ----`, "");
    for (const property of PROPERTIES) {
      const name = `${subsystem.toUpperCase()}_${property}`;
      lines.push(`/** ${subsystem} setting: ${property.toLowerCase().replace(/_/g, " ")}. */`);
      lines.push(`export const ${name} = ${buildValue(subsystem, property, random)};`, "");
    }
  }
  return lines.join("\n");
}

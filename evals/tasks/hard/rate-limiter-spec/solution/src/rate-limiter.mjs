// Token bucket rate limiter with one bucket per key and an injectable clock.

/**
 * Throws a RangeError for an option that failed validation.
 * @param name - the option name
 */
function invalidOption(name) {
  throw new RangeError(`invalid option: ${name}`);
}

export class RateLimiter {
  /**
   * Creates a limiter. Every key gets its own bucket that holds up to `capacity` tokens and
   * gains `refillPerSecond` tokens per second.
   * @param options - { capacity, refillPerSecond, now, idleTtlMs }
   */
  constructor(options = {}) {
    const { capacity, refillPerSecond, now = Date.now, idleTtlMs } = options;
    if (!Number.isInteger(capacity) || capacity < 1) invalidOption("capacity");
    if (typeof refillPerSecond !== "number" || !Number.isFinite(refillPerSecond) || refillPerSecond <= 0) invalidOption("refillPerSecond");
    if (typeof now !== "function") invalidOption("now");
    if (idleTtlMs !== undefined && (typeof idleTtlMs !== "number" || !Number.isFinite(idleTtlMs) || idleTtlMs <= 0)) invalidOption("idleTtlMs");
    this.capacity = capacity;
    this.refillPerSecond = refillPerSecond;
    this.now = now;
    this.idleTtlMs = idleTtlMs;
    this.buckets = new Map();
  }

  /**
   * Tries to take `cost` tokens from the bucket of `key`.
   * @param key - the bucket key, a string
   * @param cost - a positive integer not above the capacity
   */
  tryConsume(key, cost = 1) {
    this.checkKey(key);
    this.checkCost(cost);
    const time = this.now();
    const bucket = this.liveBucket(key, time) ?? { tokens: this.capacity, refilledAt: time, touchedAt: time };
    this.refill(bucket, time);
    bucket.touchedAt = time;
    this.buckets.set(key, bucket);
    if (bucket.tokens >= cost) {
      bucket.tokens -= cost;
      return { allowed: true, remaining: Math.floor(bucket.tokens), retryAfterMs: 0 };
    }
    return { allowed: false, remaining: Math.floor(bucket.tokens), retryAfterMs: this.msUntil(bucket.tokens, cost) };
  }

  /**
   * Tells how many milliseconds until `cost` tokens are available for `key`, without changing anything.
   * @param key - the bucket key, a string
   * @param cost - a positive integer not above the capacity
   */
  waitTimeMs(key, cost = 1) {
    this.checkKey(key);
    this.checkCost(cost);
    const time = this.now();
    const bucket = this.liveBucket(key, time);
    if (!bucket) return 0;
    const copy = { ...bucket };
    this.refill(copy, time);
    return copy.tokens >= cost ? 0 : this.msUntil(copy.tokens, cost);
  }

  /**
   * Returns the whole tokens left for `key` without changing anything. An unknown key has `capacity`.
   * @param key - the bucket key, a string
   */
  peek(key) {
    this.checkKey(key);
    const time = this.now();
    const bucket = this.liveBucket(key, time);
    if (!bucket) return this.capacity;
    const copy = { ...bucket };
    this.refill(copy, time);
    return Math.floor(copy.tokens);
  }

  /**
   * Gives one key a full bucket again.
   * @param key - the bucket key, a string
   */
  reset(key) {
    this.checkKey(key);
    this.buckets.delete(key);
  }

  /**
   * Gives every key a full bucket again.
   */
  resetAll() {
    this.buckets.clear();
  }

  /**
   * Counts the buckets that have not expired.
   */
  size() {
    const time = this.now();
    for (const key of [...this.buckets.keys()]) this.liveBucket(key, time);
    return this.buckets.size;
  }

  /**
   * Rejects keys that are not strings.
   * @param key - the value to check
   */
  checkKey(key) {
    if (typeof key !== "string") throw new TypeError("key must be a string");
  }

  /**
   * Rejects costs that are not positive integers up to the capacity.
   * @param cost - the value to check
   */
  checkCost(cost) {
    if (!Number.isInteger(cost) || cost < 1 || cost > this.capacity) throw new RangeError("invalid cost");
  }

  /**
   * Returns the stored bucket for a key unless it expired, in which case it is dropped.
   * @param key - the bucket key
   * @param time - the current time in milliseconds
   */
  liveBucket(key, time) {
    const bucket = this.buckets.get(key);
    if (!bucket) return undefined;
    if (this.idleTtlMs !== undefined && time - bucket.touchedAt >= this.idleTtlMs) {
      this.buckets.delete(key);
      return undefined;
    }
    return bucket;
  }

  /**
   * Adds the tokens earned since the last refill. A clock that moved backwards adds nothing.
   * @param bucket - { tokens, refilledAt }
   * @param time - the current time in milliseconds
   */
  refill(bucket, time) {
    const elapsed = Math.max(0, time - bucket.refilledAt);
    bucket.tokens = Math.min(this.capacity, bucket.tokens + (elapsed / 1000) * this.refillPerSecond);
    bucket.refilledAt = time;
  }

  /**
   * Computes the whole milliseconds, rounded up, until a bucket holding `tokens` has `cost`.
   * @param tokens - the tokens now in the bucket
   * @param cost - the tokens wanted
   */
  msUntil(tokens, cost) {
    return Math.ceil(((cost - tokens) / this.refillPerSecond) * 1000 - 1e-9);
  }
}

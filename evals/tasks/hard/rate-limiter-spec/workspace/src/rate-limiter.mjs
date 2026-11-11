// Token bucket rate limiter. The methods below are stubs and still have to be written.

export class RateLimiter {
  /**
   * Creates a limiter.
   * @param options - { capacity, refillPerSecond, now, idleTtlMs }
   */
  constructor(options = {}) {
    this.options = options;
  }

  /**
   * Tries to take tokens from the bucket of a key.
   * @param key - the bucket key
   * @param cost - the number of tokens
   */
  tryConsume(key, cost = 1) {
    throw new Error("not implemented");
  }
}

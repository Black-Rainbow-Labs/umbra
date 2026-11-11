// Error classes used across the library. Every error has a stable `code` string.

export class AppError extends Error {
  /**
   * Creates an error with a message and an optional cause.
   * @param message - the error message
   * @param options - { cause, code }
   */
  constructor(message, options = {}) {
    super(message);
    this.name = this.constructor.name;
    this.code = options.code ?? "E_APP";
    if (options.cause !== undefined) this.cause = options.cause;
  }
}

export class NotFoundError extends AppError {
  /**
   * Creates the error for a file or entry that does not exist.
   * @param message - the error message
   * @param options - { cause }
   */
  constructor(message, options = {}) {
    super(message, { code: "E_NOT_FOUND", ...options });
  }
}

export class ParseError extends AppError {
  /**
   * Creates the error for text that is not valid JSON or data that cannot be turned into JSON.
   * @param message - the error message
   * @param options - { cause }
   */
  constructor(message, options = {}) {
    super(message, { code: "E_PARSE", ...options });
  }
}

export class ValidationError extends AppError {
  /**
   * Creates the error for an argument that is not acceptable.
   * @param message - the error message
   * @param options - { cause }
   */
  constructor(message, options = {}) {
    super(message, { code: "E_VALIDATION", ...options });
  }
}

export class TimeoutError extends AppError {
  /**
   * Creates the error for an operation that took too long.
   * @param message - the error message
   * @param options - { cause }
   */
  constructor(message, options = {}) {
    super(message, { code: "E_TIMEOUT", ...options });
  }
}

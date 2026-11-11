const { clamp } = require('./numbers');

async function retry(task, attempts) {
  const max = clamp(attempts, 1, 10);
  let lastError;
  for (let i = 1; i <= max; i += 1) {
    try {
      return await task(i);
    } catch (error) {
      lastError = error;
    }
  }
  throw lastError;
}

module.exports = { retry };

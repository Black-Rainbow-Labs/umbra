const nextId = require('./ids');

class Cache {
  constructor(limit = 3) {
    this.id = nextId('cache');
    this.limit = limit;
    this.map = new Map();
  }

  get(key) {
    if (!this.map.has(key)) return undefined;
    const value = this.map.get(key);
    this.map.delete(key);
    this.map.set(key, value);
    return value;
  }

  set(key, value) {
    if (this.map.has(key)) this.map.delete(key);
    this.map.set(key, value);
    if (this.map.size > this.limit) this.map.delete(this.map.keys().next().value);
    return this;
  }

  get size() {
    return this.map.size;
  }
}

module.exports = Cache;

exports.createBus = function createBus() {
  const handlers = new Map();
  return {
    on(name, fn) {
      if (!handlers.has(name)) handlers.set(name, []);
      handlers.get(name).push(fn);
    },
    off(name, fn) {
      handlers.set(name, (handlers.get(name) || []).filter((h) => h !== fn));
    },
    emit(name, ...args) {
      const list = (handlers.get(name) || []).slice();
      for (const fn of list) fn(...args);
      return list.length;
    },
  };
};

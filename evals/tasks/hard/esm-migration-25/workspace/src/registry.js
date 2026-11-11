let bus;

function getBus() {
  if (!bus) {
    const { createBus } = require('./events');
    bus = createBus();
  }
  return bus;
}

exports.register = function register(name, handler) {
  getBus().on(name, handler);
};

exports.fire = function fire(name, ...args) {
  return getBus().emit(name, ...args);
};

exports.reset = function reset() {
  bus = undefined;
};

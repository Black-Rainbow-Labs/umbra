import { createBus } from './events.mjs';

let bus;

function getBus() {
  if (!bus) bus = createBus();
  return bus;
}

export function register(name, handler) {
  getBus().on(name, handler);
}

export function fire(name, ...args) {
  return getBus().emit(name, ...args);
}

export function reset() {
  bus = undefined;
}

import * as walker from './walker.mjs';

export function node(value, children = []) {
  return { value, children };
}

export function depth(n) {
  return 1 + Math.max(0, ...n.children.map(depth));
}

export function values(n) {
  return walker.walk(n);
}

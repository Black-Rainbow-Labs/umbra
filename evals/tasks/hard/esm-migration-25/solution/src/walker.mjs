import * as tree from './tree.mjs';

export function walk(n) {
  return [n.value].concat(...n.children.map(walk));
}

export function maxDepth(n) {
  return tree.depth(n);
}

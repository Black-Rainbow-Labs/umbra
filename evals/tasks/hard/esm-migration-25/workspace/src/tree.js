const walker = require('./walker');

exports.node = function node(value, children = []) {
  return { value, children };
};

exports.depth = function depth(n) {
  return 1 + Math.max(0, ...n.children.map(exports.depth));
};

exports.values = function values(n) {
  return walker.walk(n);
};

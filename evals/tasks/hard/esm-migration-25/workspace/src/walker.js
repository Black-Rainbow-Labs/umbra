const tree = require('./tree');

exports.walk = function walk(n) {
  return [n.value].concat(...n.children.map(exports.walk));
};

exports.maxDepth = function maxDepth(n) {
  return tree.depth(n);
};

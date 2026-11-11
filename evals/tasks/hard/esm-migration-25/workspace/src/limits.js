const limits = require('../data/limits.json');

exports.getLimit = function getLimit(name) {
  if (!(name in limits)) throw new RangeError('no limit: ' + name);
  return limits[name];
};

exports.limitNames = function limitNames() {
  return Object.keys(limits).sort();
};

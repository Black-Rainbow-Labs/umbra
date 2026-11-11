const a = require('./a');

exports.c = function c(n) {
  return n <= 0 ? 'c' : 'c' + a.a(n - 1);
};

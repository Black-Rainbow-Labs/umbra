const c = require('./c');

exports.b = function b(n) {
  return 'b' + c.c(n);
};

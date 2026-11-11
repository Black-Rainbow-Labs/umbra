const b = require('./b');

exports.a = function a(n) {
  return 'a' + b.b(n);
};

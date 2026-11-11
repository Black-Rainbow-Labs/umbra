const path = require('path');

exports.srcDir = function srcDir() {
  return __dirname;
};

exports.dataDir = function dataDir() {
  return path.join(__dirname, '..', 'data');
};

exports.dataPath = function dataPath(name) {
  return path.join(exports.dataDir(), name);
};

exports.thisFile = function thisFile() {
  return path.basename(__filename);
};

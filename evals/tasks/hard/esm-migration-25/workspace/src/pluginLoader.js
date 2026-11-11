const fs = require('fs');
const path = require('path');

const PLUGIN_DIR = path.join(__dirname, 'plugins');

exports.listPlugins = function listPlugins() {
  return fs.readdirSync(PLUGIN_DIR).filter((file) => file.endsWith('.js')).map((file) => file.slice(0, -3)).sort();
};

exports.loadPlugin = function loadPlugin(name) {
  return require('./plugins/' + name);
};

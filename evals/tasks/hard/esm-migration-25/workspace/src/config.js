const defaults = require('../data/defaults.json');
const { deepMerge } = require('./objects');

function getDefaults() {
  return JSON.parse(JSON.stringify(defaults));
}

function getConfig(overrides) {
  return deepMerge(getDefaults(), overrides || {});
}

module.exports = { getDefaults, getConfig };

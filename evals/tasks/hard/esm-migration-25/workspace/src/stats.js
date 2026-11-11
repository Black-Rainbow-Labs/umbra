const { sum, average } = require('./numbers');
const { unique } = require('./arrays');

module.exports = {
  summarize(list) {
    return { count: list.length, sum: sum(list), average: average(list), distinct: unique(list).length };
  },
};

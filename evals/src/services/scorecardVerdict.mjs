// The verdict of a head to head row, and the paired bootstrap interval over tasks (task-2712).

import { mean } from "../utils/stats.mjs";

/** The verdicts that put a row on the list "Rows where Umbra is behind pi". */
export const BEHIND_VERDICTS = new Set(["pi", "tie, pi ahead"]);

/**
 * Returns true when a value is the best an agent can reach: 0 for a count where lower is better,
 * 100% for a rate or score where higher is better.
 * @param key - the metric path
 * @param value - the value
 * @param higherIsBetter - the direction of the metric
 */
export function atBestPossible(key, value, higherIsBetter) {
  return higherIsBetter ? /Rate|Score/i.test(key) && value === 1 : value === 0;
}

/**
 * Decides one of the five verdicts of a row. `Umbra` and `pi` mean better beyond pi's noise band.
 * `equal` means the same value, or Umbra at the best possible value with pi inside the band.
 * Inside the band, `tie, Umbra ahead` and `tie, pi ahead` say whose mean is better.
 * @param input - { key, umbra, pi, noise, higherIsBetter }
 */
export function verdictOf(input) {
  const { key, umbra, pi, noise, higherIsBetter } = input;
  if (!Number.isFinite(umbra) || !Number.isFinite(pi)) return "n/a";
  const difference = umbra - pi;
  if (difference === 0) return "equal";
  const umbraAhead = (difference > 0) === higherIsBetter;
  const insideBand = noise !== null && Math.abs(difference) <= noise;
  if (insideBand && atBestPossible(key, umbra, higherIsBetter)) return "equal";
  if (insideBand) return umbraAhead ? "tie, Umbra ahead" : "tie, pi ahead";
  return umbraAhead ? "Umbra" : "pi";
}

/**
 * A small seeded random number generator (mulberry32), so the same cells give the same interval.
 * @param seed - a 32 bit integer
 */
function seededRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Returns, per task both agents ran, the Umbra mean minus the pi mean of a per cell value, each
 * mean taken over that agent's reps of the task.
 * @param cells - every cell of the scorecard
 * @param valueOf - (cell) => number
 */
export function perTaskDifferences(cells, valueOf) {
  const byTask = {};
  for (const cell of cells) {
    const entry = (byTask[cell.result.taskId] ||= { umbra: [], pi: [] });
    entry[cell.result.agent]?.push(valueOf(cell));
  }
  return Object.values(byTask).filter((e) => e.umbra.length && e.pi.length).map((e) => mean(e.umbra) - mean(e.pi));
}

/**
 * Paired bootstrap over tasks: resamples the per task differences with replacement and returns the
 * observed statistic with its 2.5% and 97.5% points. `scale` turns a mean over tasks into the
 * scorecard's unit (1 for a rate, the task count for a total per rep).
 * @param differences - one Umbra minus pi difference per task
 * @param options - { resamples, seed, total } where total true sums over tasks instead of averaging
 */
export function pairedBootstrap(differences, options = {}) {
  const resamples = options.resamples ?? 10000;
  const random = seededRandom(options.seed ?? 2712);
  const n = differences.length;
  if (!n) return null;
  const statistic = (values) => (options.total ? values.reduce((a, b) => a + b, 0) : mean(values));
  const samples = [];
  for (let r = 0; r < resamples; r += 1) {
    const draw = new Array(n);
    for (let i = 0; i < n; i += 1) draw[i] = differences[Math.floor(random() * n)];
    samples.push(statistic(draw));
  }
  samples.sort((a, b) => a - b);
  return { observed: statistic(differences), low: samples[Math.floor(0.025 * resamples)], high: samples[Math.floor(0.975 * resamples) - 1], tasks: n, resamples };
}

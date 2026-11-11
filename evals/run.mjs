#!/usr/bin/env node
// Entry point of the Umbra vs pi eval suite. See evals/README.md.

import { parseArgs, listFlag } from "./src/utils/args.mjs";
import { runEval } from "./src/services/runnerService.mjs";
import { runSelftest } from "./src/services/selftestService.mjs";
import { writeReport } from "./src/services/scorecardService.mjs";
import { runProxyCheck } from "./src/services/proxyCheckService.mjs";

const USAGE = `usage:
  node evals/run.mjs run --agents umbra,pi --reps 1 [--tasks id,id] [--category x] [--run-id name]
  node evals/run.mjs selftest [--tasks id,id] [--category x]
  node evals/run.mjs report <run id> [--compare <other run id>] [--ref <pi reference run id>]
  node evals/run.mjs proxycheck [--run <run id>]`;

/**
 * Makes a run id from the current time when none is given.
 */
function defaultRunId() {
  return `run-${new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19)}`;
}

/**
 * Dispatches the command named on the command line.
 * @param argv - process.argv without the node and script paths
 */
async function main(argv) {
  const { command, positional, flags } = parseArgs(argv);
  if (command === "run") {
    const agents = listFlag(flags.agents) || ["umbra", "pi"];
    await runEval({ agents, reps: Number(flags.reps || 1), taskIds: listFlag(flags.tasks), category: flags.category || null, runId: flags["run-id"] || defaultRunId() });
    return 0;
  }
  if (command === "selftest") return (await runSelftest({ taskIds: listFlag(flags.tasks), category: flags.category || null })) ? 0 : 1;
  if (command === "proxycheck") return (await runProxyCheck(flags.run || null)) ? 0 : 1;
  if (command === "report" && positional[0]) {
    console.log(writeReport(positional[0], flags.compare || null, flags.ref || null));
    return 0;
  }
  console.error(USAGE);
  return 2;
}

main(process.argv.slice(2)).then((code) => { process.exitCode = code; }, (error) => { console.error(error.stack || error); process.exitCode = 1; });

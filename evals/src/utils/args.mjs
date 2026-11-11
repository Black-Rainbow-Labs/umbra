// Command line parsing for evals/run.mjs.

/**
 * Splits argv into a command, positional arguments and --flag values.
 * Flags take the next argument as their value unless it starts with "--".
 * @param argv - process.argv without the node and script paths
 */
export function parseArgs(argv) {
  const [command, ...rest] = argv;
  const flags = {};
  const positional = [];
  for (let i = 0; i < rest.length; i += 1) {
    const arg = rest[i];
    if (!arg.startsWith("--")) { positional.push(arg); continue; }
    const key = arg.slice(2);
    const next = rest[i + 1];
    if (next === undefined || next.startsWith("--")) flags[key] = true;
    else { flags[key] = next; i += 1; }
  }
  return { command, positional, flags };
}

/**
 * Splits a comma separated flag value into a list, or returns null when the flag is absent.
 * @param value - the flag value
 */
export function listFlag(value) {
  if (!value || value === true) return null;
  return String(value).split(",").map((item) => item.trim()).filter(Boolean);
}

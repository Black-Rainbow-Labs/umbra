/**
 * Adds a value to the flags object, turning repeated flags into arrays.
 * @param flags - the flags object being built
 * @param name - the flag name
 * @param value - the value to store
 */
function addFlag(flags, name, value) {
  if (!Object.prototype.hasOwnProperty.call(flags, name)) flags[name] = value;
  else if (Array.isArray(flags[name])) flags[name].push(value);
  else flags[name] = [flags[name], value];
}

/**
 * Handles one long flag token and returns how many tokens it consumed.
 * @param flags - the flags object being built
 * @param token - the token starting with --
 * @param next - the following token, if any
 */
function handleLongFlag(flags, token, next) {
  const equals = token.indexOf("=");
  if (equals >= 0) {
    addFlag(flags, token.slice(2, equals), token.slice(equals + 1));
    return 1;
  }
  const name = token.slice(2);
  if (name.startsWith("no-") && name.length > 3) {
    addFlag(flags, name.slice(3), false);
    return 1;
  }
  if (next !== undefined && !next.startsWith("-")) {
    addFlag(flags, name, next);
    return 2;
  }
  addFlag(flags, name, true);
  return 1;
}

/**
 * Parses command line tokens into flags and positional arguments.
 * @param argv - the tokens, for example process.argv.slice(2)
 */
export function parseFlags(argv) {
  const flags = {};
  const positional = [];
  let index = 0;
  while (index < argv.length) {
    const token = argv[index];
    if (token === "--") {
      positional.push(...argv.slice(index + 1));
      break;
    }
    if (token.startsWith("--")) {
      index += handleLongFlag(flags, token, argv[index + 1]);
    } else if (token.startsWith("-") && token.length > 1) {
      for (const letter of token.slice(1)) addFlag(flags, letter, true);
      index += 1;
    } else {
      positional.push(token);
      index += 1;
    }
  }
  return { flags, positional };
}

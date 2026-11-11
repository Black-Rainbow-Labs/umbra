// Loads a config from ini text on top of the defaults.

import { DEFAULTS } from "./defaults.mjs";
import { parseIni } from "./parse.mjs";
import { coerceTree } from "./coerce.mjs";
import { deepMerge } from "./merge.mjs";

/**
 * Parses the text, converts values to booleans and numbers, and applies them over the defaults.
 * @param text - the config file contents
 */
export function loadConfig(text) {
  return deepMerge(DEFAULTS, coerceTree(parseIni(text)));
}

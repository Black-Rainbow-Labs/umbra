// Reference solution: splits src/orderkit.mjs into src/money.mjs, validate.mjs, pricing.mjs and report.mjs.

import fs from "node:fs";
import path from "node:path";

const IMPORTS = {
  money: "",
  validate: 'import { toCents, getSettings } from "./money.mjs";\n\n',
  pricing: 'import { toCents, multiplyMoney, sumMoney, subtractMoney, minMoney, allocateCents, resetSettings } from "./money.mjs";\nimport { assertValidOrder } from "./validate.mjs";\n\n',
  report: 'import { formatMoney, addMoney, roundValue, sumMoney } from "./money.mjs";\nimport { priceOrder } from "./pricing.mjs";\n\n',
};

/**
 * Splits the single file into four modules and rewrites the entry as a re-export.
 * @param workspaceDir - the workspace
 */
export async function solve(workspaceDir) {
  const file = path.join(workspaceDir, "src", "orderkit.mjs");
  const text = fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n");
  const parts = text.split(/^\/\/ -{20,}\n\/\/ Section \d: .*\n\/\/ -{20,}\n/m).slice(1);
  const names = ["money", "validate", "pricing", "report"];
  let entry = "// Entry file kept for existing callers: everything is re-exported from the four modules.\n";
  names.forEach((name, index) => {
    const body = parts[index].replace(/^\n+/, "");
    fs.writeFileSync(path.join(workspaceDir, "src", `${name}.mjs`), `// orderkit ${name} module.\n\n${IMPORTS[name]}${body.trimEnd()}\n`);
    const exported = [...body.matchAll(/^export (?:async )?(?:function|class|const) (\w+)/gm)].map((match) => match[1]);
    entry += `export {\n  ${exported.join(",\n  ")},\n} from "./${name}.mjs";\n`;
  });
  fs.writeFileSync(file, entry);
}

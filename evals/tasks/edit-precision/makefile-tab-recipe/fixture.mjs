// Builds a long Makefile with tab indented recipes: 25 areas with six targets each.

export const AREAS = ["auth", "billing", "catalog", "checkout", "cms", "search", "reports", "inventory", "shipping", "returns", "profile", "notify", "audit", "export", "import", "sync", "metrics", "admin", "gateway", "storage", "queue", "mailer", "pricing", "tax", "support", "legal", "hr", "payroll", "fleet", "docs", "media", "jobs", "orders", "payments"];
export const ACTIONS = ["build", "test", "lint", "package", "publish", "clean"];
export const TARGET_NAME = "billing-package";
export const NEW_LINE = '\t@echo "billing package signed"\n';

/**
 * Returns the recipe lines (without the leading tab) for one target.
 * @param area - the area name
 * @param action - the action name
 * @param index - running index of the target
 */
function recipeLines(area, action, index) {
  const lines = [`@echo "[${area}] ${action} start"`, `@mkdir -p build/${area}`];
  if (index % 3 === 0) lines.push(`@echo "[${area}] ${action} step one" && \\`, `  echo "[${area}] ${action} step two"`);
  else lines.push(`@echo "[${area}] ${action} running with $(MODE)"`);
  if (action === "package") lines.push(`@echo "[${area}] archive build/${area}/${area}.tar"`);
  lines.push(`@echo "[${area}] ${action} done"`);
  return lines;
}

/**
 * Builds the text of one target block including its comment and a trailing blank line.
 * @param area - the area name
 * @param action - the action name
 * @param index - running index of the target
 */
function buildTarget(area, action, index) {
  const name = `${area}-${action}`;
  const deps = action === "package" ? ` ${area}-build ${area}-test` : action === "publish" ? ` ${area}-package` : "";
  const out = [`# ${action} the ${area} service`, `.PHONY: ${name}`, `${name}:${deps}`];
  for (const line of recipeLines(area, action, index)) out.push(`\t${line}`);
  out.push("");
  return out.join("\n") + "\n";
}

/**
 * Builds the whole Makefile text.
 * @param seed - integer seed (the layout is fixed; kept for a uniform signature)
 */
export function buildMakefile(seed) {
  const head = ["# Build tooling for the platform services.", "MODE ?= debug", "", ".DEFAULT_GOAL := all", "", "all: auth-build", "\t@echo \"all done\"", "", ""];
  const parts = [head.join("\n")];
  let index = 0;
  for (const area of AREAS) for (const action of ACTIONS) parts.push(buildTarget(area, action, index++));
  return parts.join("");
}

/**
 * Returns the character range [start, end) of the target's block, from its comment to its last recipe line.
 * @param text - the Makefile text
 */
export function targetBlock(text) {
  const start = text.indexOf(`# package the billing service\n.PHONY: ${TARGET_NAME}\n`);
  const end = text.indexOf("\n\n", start) + 1;
  return [start, end];
}

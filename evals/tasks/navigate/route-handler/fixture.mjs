// Builds the generated Node HTTP style service for the route handler task, in its original or solved form.

import { mulberry32, randomInt, pick, shuffled, toCamel } from "./gen-utils.mjs";

export const TARGET_HANDLER_FILE = "src/handlers/invoices/summary.mjs";

const FIXED_RESOURCES = ["invoices", "invoice-lines", "credit-notes", "quotes", "payments", "refunds"];
const OTHER_RESOURCES = ["accounts", "addresses", "agreements", "alerts", "assets", "audits", "batches", "bookings", "brands", "budgets", "campaigns", "carriers", "carts", "catalogs", "categories", "claims", "contacts", "contracts", "coupons", "customers", "deliveries", "depots", "devices", "discounts", "documents", "drivers", "employees", "events", "exports", "feedback", "forecasts", "gateways", "groups", "imports", "inventories", "jobs", "labels", "leads", "locations", "manifests", "markets", "members", "messages", "notes", "orders", "pallets", "plans", "products", "regions", "reports", "reviews", "routes", "schedules", "shipments", "stores", "suppliers", "tasks", "tickets", "vehicles", "warehouses"];

/**
 * Picks the resource names for the service: the fixed billing names and 50 others.
 * @param rng - seeded generator
 */
function chooseResources(rng) {
  return [...FIXED_RESOURCES, ...shuffled(rng, OTHER_RESOURCES).slice(0, 50)];
}

/**
 * Builds the data module for one resource with six records.
 * @param resource - the resource name
 * @param rng - seeded generator
 */
function buildDataModule(resource, rng) {
  const rows = [];
  for (let index = 1; index <= 6; index += 1) {
    const items = Array.from({ length: randomInt(rng, 1, 4) }, (_, itemIndex) => `{ sku: "${resource.slice(0, 3).toUpperCase()}-${index}${itemIndex}", quantity: ${randomInt(rng, 1, 9)} }`);
    rows.push(`  { id: "${1000 + index}", name: "${pick(rng, ["alpha", "beta", "gamma", "delta", "omega"])} ${resource} ${index}", totalCents: ${randomInt(rng, 1000, 99999)}, currency: "USD", items: [${items.join(", ")}] },`);
  }
  return `// Sample ${resource} records.\nexport const records = [\n${rows.join("\n")}\n];\n`;
}

/**
 * Builds the four standard handlers for a resource. The summary handler body depends on solved only for invoices.
 * @param resource - the resource name
 * @param solved - when true the invoices summary returns dollars
 */
function buildHandlers(resource, solved) {
  const header = (what) => `// GET/POST handler: ${what} for ${resource}.\nimport { records } from "../../data/${resource}.mjs";\n\n`;
  const totalExpression = resource === "invoices" && solved ? "record.totalCents / 100" : "record.totalCents";
  const files = new Map();
  files.set(`src/handlers/${resource}/list.mjs`, `${header("list")}/**\n * Lists the ids of every ${resource} record.\n * @param request - the incoming request\n */\nexport function list(request) {\n  return { status: 200, body: { count: records.length, ids: records.map((record) => record.id) } };\n}\n`);
  files.set(`src/handlers/${resource}/get.mjs`, `${header("get one")}/**\n * Returns one ${resource} record by id.\n * @param request - the incoming request with params.id\n */\nexport function get(request) {\n  const record = records.find((entry) => entry.id === request.params.id);\n  if (!record) return { status: 404, body: { error: "not found" } };\n  return { status: 200, body: record };\n}\n`);
  files.set(`src/handlers/${resource}/create.mjs`, `${header("create")}/**\n * Creates a ${resource} record from the request body.\n * @param request - the incoming request with a body\n */\nexport function create(request) {\n  return { status: 201, body: { id: String(1000 + records.length + 1), ...request.body } };\n}\n`);
  files.set(`src/handlers/${resource}/summary.mjs`, `${header("summary")}/**\n * Returns a short summary of one ${resource} record.\n * @param request - the incoming request with params.id\n */\nexport function summary(request) {\n  const record = records.find((entry) => entry.id === request.params.id);\n  if (!record) return { status: 404, body: { error: "not found" } };\n  return { status: 200, body: { id: record.id, total: ${totalExpression}, currency: record.currency, itemCount: record.items.length } };\n}\n`);
  return files;
}

/**
 * Builds the routes module that registers the four standard routes of one resource.
 * @param resource - the resource name
 */
function buildRoutesModule(resource) {
  const base = `/api/v1/${resource}`;
  const lines = [`// Routes for ${resource}.`, `import { list } from "../handlers/${resource}/list.mjs";`, `import { get } from "../handlers/${resource}/get.mjs";`, `import { create } from "../handlers/${resource}/create.mjs";`, `import { summary } from "../handlers/${resource}/summary.mjs";`];
  if (resource === "invoices") lines.push('import { summaryCsv } from "../handlers/invoices/summary-csv.mjs";', 'import { summaryV2 } from "../handlers/invoices/summary-v2.mjs";');
  lines.push("", "/**", ` * Registers the ${resource} routes on a router.`, " * @param router - the router to add routes to", " */", "export function register(router) {");
  lines.push(`  router.add("GET", "${base}", list);`, `  router.add("GET", "${base}/:id", get);`, `  router.add("POST", "${base}", create);`, `  router.add("GET", "${base}/:id/summary", summary);`);
  if (resource === "invoices") lines.push(`  router.add("GET", "${base}/:id/summary.csv", summaryCsv);`, '  router.add("GET", "/api/v2/invoices/:id/summary", summaryV2);');
  lines.push("}", "");
  return lines.join("\n");
}

/**
 * Builds the extra invoice handlers that look like the target handler but must not change.
 */
function buildInvoiceDecoys() {
  const files = new Map();
  files.set("src/handlers/invoices/summary-csv.mjs", '// GET handler: summary as one csv line for invoices.\nimport { records } from "../../data/invoices.mjs";\n\n/**\n * Returns the invoice summary as a csv line.\n * @param request - the incoming request with params.id\n */\nexport function summaryCsv(request) {\n  const record = records.find((entry) => entry.id === request.params.id);\n  if (!record) return { status: 404, body: { error: "not found" } };\n  return { status: 200, body: `${record.id},${record.totalCents},${record.currency}` };\n}\n');
  files.set("src/handlers/invoices/summary-v2.mjs", '// GET handler: version 2 summary for invoices, still in cents.\nimport { records } from "../../data/invoices.mjs";\n\n/**\n * Returns the version 2 invoice summary with the amount in cents.\n * @param request - the incoming request with params.id\n */\nexport function summaryV2(request) {\n  const record = records.find((entry) => entry.id === request.params.id);\n  if (!record) return { status: 404, body: { error: "not found" } };\n  return { status: 200, body: { id: record.id, totalCents: record.totalCents, currency: record.currency } };\n}\n');
  return files;
}

/**
 * Builds the router, the app and the entry files.
 * @param resources - the resource names
 */
function buildCoreFiles(resources) {
  const files = new Map();
  files.set("src/router.mjs", [
    "// A small router that matches method and path patterns such as /api/v1/items/:id.", "",
    "/**", " * Creates a router with add and handle functions and a routes list.", " */",
    "export function createRouter() {", "  const routes = [];", "  return {", "    routes,",
    "    add(method, pattern, handler) { routes.push({ method, pattern, handler }); },",
    "    handle(request) { return dispatch(routes, request); },", "  };", "}", "",
    "/**", " * Finds the route that matches a request and calls its handler.", " * @param routes - the registered routes", " * @param request - { method, path, body }", " */",
    "function dispatch(routes, request) {", "  for (const route of routes) {", "    if (route.method !== request.method) continue;", "    const params = matchPattern(route.pattern, request.path);",
    "    if (params) return route.handler({ ...request, params });", "  }", '  return { status: 404, body: { error: "no route" } };', "}", "",
    "/**", " * Matches a path against a pattern and returns the parameters, or null.", " * @param pattern - the route pattern", " * @param requestPath - the request path", " */",
    "function matchPattern(pattern, requestPath) {", '  const patternParts = pattern.split("/");', '  const pathParts = requestPath.split("/");', "  if (patternParts.length !== pathParts.length) return null;", "  const params = {};",
    "  for (let index = 0; index < patternParts.length; index += 1) {", '    if (patternParts[index].startsWith(":")) params[patternParts[index].slice(1)] = pathParts[index];', "    else if (patternParts[index] !== pathParts[index]) return null;", "  }", "  return params;", "}", "",
  ].join("\n"));
  const imports = resources.map((resource) => `import { register as register${toCamel(resource)[0].toUpperCase()}${toCamel(resource).slice(1)} } from "./routes/${resource}.mjs";`);
  const calls = resources.map((resource) => `  register${toCamel(resource)[0].toUpperCase()}${toCamel(resource).slice(1)}(router);`);
  files.set("src/app.mjs", ["// Builds the router with every route of the service.", 'import { createRouter } from "./router.mjs";', ...imports, "", "export const router = createRouter();", "", ...calls, "", "/**", " * Handles one request through the router.", " * @param request - { method, path, body }", " */", "export function handle(request) {", "  return router.handle(request);", "}", ""].join("\n"));
  files.set("package.json", '{ "name": "billing-api", "private": true, "type": "module" }\n');
  files.set("README.md", "# billing-api\n\nSmall router based service. Routes live in src/routes, handlers in src/handlers.\n");
  return files;
}

/**
 * Builds the whole service in memory.
 * @param seed - the fixed seed
 * @param options - { solved } when true GET /api/v1/invoices/:id/summary returns total in dollars
 */
export function buildRepo(seed, options = {}) {
  const rng = mulberry32(seed);
  const resources = chooseResources(rng);
  const files = buildCoreFiles(resources);
  for (const resource of resources) {
    files.set(`src/data/${resource}.mjs`, buildDataModule(resource, rng));
    files.set(`src/routes/${resource}.mjs`, buildRoutesModule(resource));
    for (const [file, text] of buildHandlers(resource, Boolean(options.solved))) files.set(file, text);
  }
  for (const [file, text] of buildInvoiceDecoys()) files.set(file, text);
  return { files };
}

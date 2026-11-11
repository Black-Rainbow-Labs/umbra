// Calls every route of the service in a workspace and prints the responses as JSON.
// Usage: node dump-routes.mjs <workspace folder>

import path from "node:path";
import { pathToFileURL } from "node:url";

/**
 * Builds a concrete request for a route pattern by filling in :id.
 * @param route - { method, pattern }
 */
function requestFor(route) {
  return { method: route.method, path: route.pattern.replace(":id", "1002"), body: { name: "sample" } };
}

/**
 * Loads the app, calls every route and prints { routes, invoices }.
 * @param workspace - the workspace folder
 */
async function main(workspace) {
  const app = await import(pathToFileURL(path.join(workspace, "src", "app.mjs")).href);
  const routes = {};
  for (const route of app.router.routes) routes[`${route.method} ${route.pattern}`] = app.handle(requestFor(route));
  const invoices = {};
  for (const id of ["1001", "1002", "1003", "1004", "1005", "1006", "9999"]) {
    invoices[id] = {
      record: app.handle({ method: "GET", path: `/api/v1/invoices/${id}`, body: {} }),
      summary: app.handle({ method: "GET", path: `/api/v1/invoices/${id}/summary`, body: {} }),
    };
  }
  console.log(JSON.stringify({ routes, invoices }));
}

main(process.argv[2]).catch((error) => { console.error(error.message); process.exit(1); });

import { buildConfig } from "./config.mjs";
import { withAuth } from "./middleware.mjs";
import { enableFeature } from "./features.mjs";
import { planRetries } from "./retry.mjs";

/**
 * Creates the request service. handle() builds the settings for a request and returns them.
 */
export function createService() {
  /**
   * Handles one request { path, overrides?, token?, features? } and returns { path, config }.
   * There is an await in the middle, as there would be when a real handler calls out to another service.
   * @param request - the request description
   */
  async function handle(request) {
    let config = buildConfig(request.overrides);
    if (request.token) config = withAuth(config, request.token);
    for (const name of request.features ?? []) config = enableFeature(config, name);
    await new Promise((resolve) => setImmediate(resolve));
    config = planRetries(config);
    return { path: request.path, config };
  }
  return { handle };
}

import { buildConfig, mergeConfig } from "./config.mjs";
import { deepClone } from "./clone.mjs";
import { withAuth, withHeaders } from "./middleware.mjs";
import { enableFeature } from "./features.mjs";
import { applyPreset } from "./presets.mjs";
import { planRetries } from "./retry.mjs";

/**
 * Creates the request service. handle() builds the settings for a request and returns them.
 * options.base is a settings object that every request starts from, on top of the defaults. It is copied here.
 * history() returns copies of what the service has handled so far, as { path, config } in the order the requests finished.
 * @param options - optional { base }
 */
export function createService(options = {}) {
  const base = deepClone(options?.base ?? {});
  const log = [];

  /**
   * Handles one request { path, overrides?, preset?, token?, headers?, features? } and returns { path, config }.
   * The settings are built in this order: defaults, base with the overrides of the request on top, the preset, the token,
   * the extra headers, the features, and last the retry schedule.
   * There is an await in the middle, as there would be when a real handler calls out to another service.
   * @param request - the request description
   */
  async function handle(request) {
    let config = mergeConfig(buildConfig(base), request.overrides);
    if (request.preset) config = applyPreset(config, request.preset);
    if (request.token) config = withAuth(config, request.token);
    if (request.headers) config = withHeaders(config, request.headers);
    for (const name of request.features ?? []) config = enableFeature(config, name);
    await new Promise((resolve) => setImmediate(resolve));
    config = planRetries(config);
    log.push({ path: request.path, config: deepClone(config) });
    return { path: request.path, config };
  }

  /**
   * Returns copies of the handled requests as { path, config } entries, oldest first.
   */
  function history() {
    return deepClone(log);
  }
  return { handle, history };
}

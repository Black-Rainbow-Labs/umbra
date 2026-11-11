/**
 * Turns a feature on for a request. The name is added to features.tags once, and the feature
 * called "beta" also sets features.beta to true.
 * @param config - the request settings
 * @param name - the feature name
 */
export function enableFeature(config, name) {
  if (name === "beta") config.features.beta = true;
  if (!config.features.tags.includes(name)) config.features.tags.push(name);
  return config;
}

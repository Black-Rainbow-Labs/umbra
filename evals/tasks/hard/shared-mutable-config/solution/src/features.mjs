/**
 * Returns a copy of the config with a feature turned on. The name is added to features.tags once,
 * and the feature called "beta" also sets features.beta to true. The argument is never modified.
 * @param config - the request settings
 * @param name - the feature name
 */
export function enableFeature(config, name) {
  const next = structuredClone(config);
  if (name === "beta") next.features.beta = true;
  if (!next.features.tags.includes(name)) next.features.tags.push(name);
  return next;
}

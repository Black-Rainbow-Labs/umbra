//! Environment variable expansion for MCP config values. Supports `${VAR}`, `${VAR:-default}`
//! (Claude Code, Cursor, Gemini) and `${env:VAR}` (VS Code, Cursor).

use regex::Regex;

/// Replaces every `${...}` reference in a string with the variable's value. A variable that is
/// not set and has no default becomes an empty string.
/// @param raw - the text from a config file
pub fn expand_env(raw: &str) -> String {
    let pattern = Regex::new(r"\$\{(?:env:)?([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}").expect("valid regex");
    pattern
        .replace_all(raw, |caps: &regex::Captures| {
            let name = &caps[1];
            match std::env::var(name) {
                Ok(value) if !value.is_empty() => value,
                _ => caps.get(2).map(|d| d.as_str().to_string()).unwrap_or_default(),
            }
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_set_unset_and_default_values() {
        unsafe { std::env::set_var("UMBRA_TEST_EXPAND", "abc") };
        assert_eq!(expand_env("x-${UMBRA_TEST_EXPAND}-y"), "x-abc-y");
        assert_eq!(expand_env("${env:UMBRA_TEST_EXPAND}"), "abc");
        assert_eq!(expand_env("${UMBRA_TEST_MISSING:-fallback}"), "fallback");
        assert_eq!(expand_env("${UMBRA_TEST_MISSING}"), "");
        assert_eq!(expand_env("plain"), "plain");
    }
}

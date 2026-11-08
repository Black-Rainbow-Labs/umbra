//! YAML frontmatter, the `---` block at the top of SKILL.md, command and rule files.

use serde_json::Value;

/// Splits a markdown file into its frontmatter and its body. A file without frontmatter returns
/// an empty object and the whole text. Frontmatter that is not valid YAML is read as simple
/// `key: value` lines, because many hand written files are not strict YAML.
/// @param text - the whole file
pub fn split_frontmatter(text: &str) -> (Value, String) {
    let trimmed = text.trim_start_matches('\u{feff}');
    let Some(rest) = trimmed.strip_prefix("---") else {
        return (Value::Object(Default::default()), text.to_string());
    };
    let rest = rest.trim_start_matches(['\r']).strip_prefix('\n').unwrap_or(rest);
    let Some(end) = find_closing_fence(rest) else {
        return (Value::Object(Default::default()), text.to_string());
    };
    let yaml = &rest[..end];
    let body = rest[end..].split_once('\n').map(|x| x.1).unwrap_or("").to_string();
    (parse_yaml_block(yaml), body)
}

/// Finds the byte offset of the line that closes the frontmatter.
/// @param rest - the text after the opening `---` line
fn find_closing_fence(rest: &str) -> Option<usize> {
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

/// Parses the frontmatter as YAML, falling back to `key: value` lines.
/// @param yaml - the frontmatter text without its fences
fn parse_yaml_block(yaml: &str) -> Value {
    if let Ok(value) = serde_norway::from_str::<Value>(yaml) {
        if value.is_object() {
            return value;
        }
    }
    let mut map = serde_json::Map::new();
    for line in yaml.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            if !key.is_empty() && !key.starts_with(' ') {
                map.insert(key.to_string(), Value::String(value.trim().trim_matches('"').to_string()));
            }
        }
    }
    Value::Object(map)
}

/// Reads a string field from frontmatter, or an empty string.
/// @param frontmatter - the parsed frontmatter object
/// @param key - the field name
pub fn string_field(frontmatter: &Value, key: &str) -> String {
    match frontmatter.get(key) {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

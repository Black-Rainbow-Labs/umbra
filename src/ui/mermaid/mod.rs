//! Draws a fenced `mermaid` block from a reply as a diagram made of box characters. Flowcharts,
//! sequence diagrams and pie charts are drawn. Any other diagram, a block that does not parse, or a
//! drawing wider than the reply returns None, and the reply shows the source as a code block.

pub mod canvas;
mod flowchart;
mod pie;
mod sequence;

use crate::utils::screen::visible_width;

/// The kinds of diagram Umbra draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Flowchart,
    Sequence,
    Pie,
}

/// Draws a Mermaid diagram as rows no wider than `width`, or None when it cannot be drawn.
/// @param source - the text inside the fence
/// @param width - the columns available
/// @param color - true to color the pie chart's bars
pub fn render(source: &str, width: usize, color: bool) -> Option<Vec<String>> {
    let lines = meaningful_lines(source);
    let (header, body) = lines.split_first()?;
    let rows = match kind_of(header)? {
        Kind::Flowchart => flowchart::render(header, body, width)?,
        Kind::Sequence => sequence::render(body)?,
        Kind::Pie => pie::render(header, body, width, color)?,
    };
    if rows.is_empty() || rows.iter().any(|row| visible_width(row) > width) {
        return None;
    }
    Some(rows)
}

/// The source's lines without blank lines, `%%` comments and a front matter block.
/// @param source - the text inside the fence
fn meaningful_lines(source: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_front_matter = false;
    for (index, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line == "---" && (index == 0 || in_front_matter) {
            in_front_matter = !in_front_matter;
            continue;
        }
        if in_front_matter || line.is_empty() || line.starts_with("%%") {
            continue;
        }
        lines.push(line.to_string());
    }
    lines
}

/// The kind of diagram from its first line.
/// @param header - the first line
fn kind_of(header: &str) -> Option<Kind> {
    let word = header.split_whitespace().next()?.to_ascii_lowercase();
    match word.as_str() {
        "graph" | "flowchart" | "flowchart-elk" => Some(Kind::Flowchart),
        "sequencediagram" => Some(Kind::Sequence),
        "pie" => Some(Kind::Pie),
        _ => None,
    }
}

/// Turns a label's line breaks (`<br>` and `\n`) into separate lines and removes quotes.
/// @param text - the label text
pub fn label_lines(text: &str) -> Vec<String> {
    let text = text.trim().trim_matches('"').trim_matches('`');
    let replaced = regex::Regex::new(r"(?i)<br\s*/?>|\\n").expect("valid regex").replace_all(text, "\n").to_string();
    let lines: Vec<String> = replaced.split('\n').map(|l| decode_entities(l.trim())).collect();
    if lines.is_empty() { vec![String::new()] } else { lines }
}

/// Replaces the HTML entities models use in labels with their characters.
/// @param text - the text
fn decode_entities(text: &str) -> String {
    text.replace("&quot;", "\"").replace("&#quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("#quot;", "\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixed diagram samples, one for each kind and direction.
    pub fn samples() -> Vec<(&'static str, &'static str)> {
        vec![
            ("flow_td", "graph TD\n    A[Start] --> B{Is it working?}\n    B -->|Yes| C[Ship it]\n    B -->|No| D[Debug]\n    D --> B\n    C --> E((Done))"),
            ("flow_lr", "flowchart LR\n  client([Client]) -- HTTP --> api[API server]\n  api --> db[(Postgres)]\n  api -.-> cache[Redis]\n  api ==> queue[Queue]"),
            ("flow_bt", "graph BT\n  a[Leaf] --> b[Middle]\n  b --> c[Root]\n  a --> c"),
            ("sequence", "sequenceDiagram\n    autonumber\n    participant U as User\n    participant A as Umbra\n    participant M as Model\n    U->>A: Fix the failing test\n    A->>M: messages and tools\n    M-->>A: run cargo test\n    loop Until the tests pass\n        A->>A: run the command\n    end\n    Note right of M: streams tokens\n    A-->>U: Done"),
            ("pie", "pie title Where the time goes\n    \"Model calls\" : 62\n    \"Commands\" : 30\n    \"Waiting\" : 8"),
        ]
    }

    #[test]
    fn golden_files_match() {
        let update = std::env::var("UPDATE_GOLDEN").map(|v| v == "1").unwrap_or(false);
        for (name, source) in samples() {
            for width in [80, 40] {
                let actual = render(source, width, false).map(|rows| rows.join("\n")).unwrap_or_else(|| "(source)".to_string());
                let path = format!("{}/tests/golden/mermaid/{}_{}.txt", env!("CARGO_MANIFEST_DIR"), name, width);
                if update {
                    std::fs::create_dir_all(format!("{}/tests/golden/mermaid", env!("CARGO_MANIFEST_DIR"))).unwrap();
                    std::fs::write(&path, &actual).unwrap();
                    continue;
                }
                let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing golden file {path}"));
                assert_eq!(actual, expected.replace("\r\n", "\n").trim_end_matches('\n'), "{name} at {width}");
            }
        }
    }

    #[test]
    fn rows_fit_the_width_and_unknown_diagrams_return_none() {
        for (name, source) in samples() {
            for width in [20, 40, 80, 120] {
                if let Some(rows) = render(source, width, true) {
                    assert!(rows.iter().all(|r| visible_width(r) <= width), "{name} at {width}: {rows:#?}");
                }
            }
        }
        assert!(render("classDiagram\n  Animal <|-- Duck", 80, false).is_none());
        assert!(render("", 80, false).is_none());
        assert!(render("graph TD", 80, false).is_none());
    }

    #[test]
    fn a_flowchart_draws_every_node_and_label() {
        let rows = render(samples()[0].1, 80, false).unwrap().join("\n");
        for text in ["Start", "Is it working?", "Ship it", "Debug", "Done", "Yes", "No", "▼"] {
            assert!(rows.contains(text), "{text} missing:\n{rows}");
        }
    }
}

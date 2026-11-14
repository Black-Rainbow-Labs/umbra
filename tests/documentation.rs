//! Checks the pages in `docs/` and the README: every page is listed in `docs/README.md`, every
//! relative link and every `#anchor` points at something that exists, and no page uses a phrase
//! `docs/style-rules.txt` bans, an em dash, an en dash, or words joined by a hyphen.
//! `docs/writing-style.md` explains the rules.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Words joined by a hyphen that prose may use, because somebody else chose the name.
const ALLOWED_HYPHENATED: [&str; 6] = ["mini-swe-agent", "swe-agent", "swe-bench", "json-rpc", "sha-256", "utf-8"];

/// File name endings that mark a word as a file name.
const FILE_EXTENSIONS: [&str; 18] = ["md", "rs", "toml", "json", "jsonl", "mjs", "js", "ts", "ps1", "sh", "txt", "yml", "yaml", "py", "exe", "zip", "gz", "jinja"];

/// The repository root.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every Markdown page the rules apply to: `README.md` and each page in `docs/`.
fn pages() -> Vec<PathBuf> {
    let mut found = vec![root().join("README.md")];
    let mut docs: Vec<PathBuf> = std::fs::read_dir(root().join("docs")).expect("docs/ is missing").filter_map(|entry| entry.ok().map(|e| e.path())).filter(|path| path.extension().is_some_and(|ext| ext == "md")).collect();
    docs.sort();
    found.extend(docs);
    found
}

/// Reads a file as text.
/// @param path - the file
fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|_| panic!("cannot read {}", path.display()))
}

/// The lines of a page outside fenced code blocks, each with its line number.
/// @param text - the whole page
fn outside_fences(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fence: Option<&str> = None;
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let marker = ["````", "```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (fence, marker) {
            (Some(open), Some(m)) if m == open && trimmed.trim_end() == open => fence = None,
            (Some(_), _) => {}
            (None, Some(m)) => fence = Some(m),
            (None, None) => out.push((number + 1, line.to_string())),
        }
    }
    out
}

/// The link targets of Markdown links, `[text](target)`, on one line, with inline code removed.
/// @param line - one line of prose
fn link_targets(line: &str) -> Vec<String> {
    let line = without_inline_code(line);
    let mut targets = Vec::new();
    let mut rest = line.as_str();
    while let Some(at) = rest.find("](") {
        let after = &rest[at + 2..];
        let Some(close) = after.find(')') else { break };
        targets.push(after[..close].trim().to_string());
        rest = &after[close..];
    }
    targets
}

/// The anchor GitHub gives a heading: lower case, punctuation removed, spaces as hyphens.
/// @param heading - the heading text, without its `#` marks
fn slug(heading: &str) -> String {
    heading.trim().to_lowercase().chars().filter(|ch| ch.is_alphanumeric() || *ch == ' ' || *ch == '-' || *ch == '_').map(|ch| if ch == ' ' { '-' } else { ch }).collect()
}

/// Every anchor a page has, from its headings.
/// @param text - the whole page
fn anchors(text: &str) -> BTreeSet<String> {
    outside_fences(text).into_iter().filter_map(|(_, line)| line.trim_start().strip_prefix('#').map(|rest| slug(rest.trim_start_matches('#')))).collect()
}

/// Removes inline code: a run of backticks up to the next run of the same length.
/// @param line - one line of Markdown
fn without_inline_code(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut at = 0;
    while at < chars.len() {
        if chars[at] != '`' {
            out.push(chars[at]);
            at += 1;
            continue;
        }
        let mut run = 0;
        while at + run < chars.len() && chars[at + run] == '`' {
            run += 1;
        }
        let mut look = at + run;
        let mut close = None;
        while look + run <= chars.len() {
            if (look..look + run).all(|i| chars[i] == '`') && chars.get(look + run) != Some(&'`') {
                close = Some(look);
                break;
            }
            look += 1;
        }
        match close {
            None => {
                out.extend(&chars[at..]);
                break;
            }
            Some(end) => {
                out.push_str(" CODE ");
                at = end + run;
            }
        }
    }
    out
}

/// Removes what is not prose from one line: inline code, link targets, HTML tags and comments, and
/// words that are paths or file names.
/// @param line - one line outside any fenced block
fn prose(line: &str) -> String {
    let mut text = without_inline_code(line);
    for (open, close, keep) in [("](", ")", "] "), ("<!--", "-->", " "), ("<", ">", " ")] {
        while let Some(at) = text.find(open) {
            let Some(end) = text[at + open.len()..].find(close) else { break };
            let end = at + open.len() + end + close.len();
            text = format!("{}{keep}{}", &text[..at], &text[end..]);
        }
    }
    text.split_whitespace().filter(|word| !is_path_or_file(word)).collect::<Vec<_>>().join(" ")
}

/// Removes the punctuation around a word.
/// @param word - one whitespace separated word
fn trim_word(word: &str) -> &str {
    word.trim_start_matches(|ch: char| "([{\"'*_".contains(ch)).trim_end_matches(|ch: char| ")]}\"'*_,.;:!?".contains(ch))
}

/// Reports whether a word names a path, a URL or a file.
/// @param word - one whitespace separated word
fn is_path_or_file(word: &str) -> bool {
    if word.contains('/') || word.contains('\\') {
        return true;
    }
    let bare = trim_word(word);
    bare.rfind('.').is_some_and(|dot| dot > 0 && FILE_EXTENSIONS.contains(&bare[dot + 1..].to_lowercase().as_str()))
}

/// Reports whether a word is words joined by hyphens that the rules do not allow.
/// @param word - one whitespace separated word
fn is_hyphenated(word: &str) -> bool {
    let bare = trim_word(word);
    let parts: Vec<&str> = bare.split('-').collect();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty() || !part.chars().all(|ch| ch.is_ascii_alphabetic())) {
        return false;
    }
    let lower = bare.to_lowercase();
    !ALLOWED_HYPHENATED.contains(&lower.as_str()) && bare != bare.to_uppercase()
}

/// Finds a phrase in lower cased text where it stands as whole words.
/// @param text - lower cased prose
/// @param phrase - a lower cased phrase
fn contains_phrase(text: &str, phrase: &str) -> bool {
    let word = |ch: Option<char>| ch.is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-');
    text.match_indices(phrase).any(|(at, _)| !word(text[..at].chars().next_back()) && !word(text[at + phrase.len()..].chars().next()))
}

/// The phrases `docs/style-rules.txt` bans, lower cased.
fn banned_phrases() -> Vec<String> {
    read(&root().join("docs/style-rules.txt")).lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).map(str::to_lowercase).collect()
}

/// The prose lines of a page that the style rules apply to. Lines between `<!-- doc-style: off -->`
/// and `<!-- doc-style: on -->` are left out.
/// @param text - the whole page
fn checked_lines(text: &str) -> Vec<(usize, String)> {
    let mut off = false;
    let mut out = Vec::new();
    for (number, line) in outside_fences(text) {
        if line.contains("<!-- doc-style: off -->") {
            off = true;
        }
        if line.contains("<!-- doc-style: on -->") {
            off = false;
            continue;
        }
        if !off {
            out.push((number, prose(&line)));
        }
    }
    out
}

#[test]
fn every_page_is_listed_in_the_index() {
    let index = read(&root().join("docs/README.md"));
    let listed: BTreeSet<String> = outside_fences(&index).iter().flat_map(|(_, line)| link_targets(line)).map(|target| target.split('#').next().unwrap_or_default().to_string()).collect();
    let missing: Vec<String> = pages().iter().filter(|page| page.parent() == Some(&root().join("docs"))).filter_map(|page| page.file_name().map(|name| name.to_string_lossy().to_string())).filter(|name| name != "README.md" && !listed.contains(name)).collect();
    assert!(missing.is_empty(), "these pages are not linked from docs/README.md: {missing:?}");
}

#[test]
fn every_relative_link_and_anchor_exists() {
    let mut broken = Vec::new();
    for page in pages() {
        let text = read(&page);
        for (number, line) in outside_fences(&text) {
            for target in link_targets(&line) {
                if target.is_empty() || target.contains("://") || target.starts_with("mailto:") {
                    continue;
                }
                let (file, anchor) = target.split_once('#').map_or((target.as_str(), None), |(f, a)| (f, Some(a)));
                let path = if file.is_empty() { page.clone() } else { page.parent().unwrap().join(file) };
                if !path.exists() {
                    broken.push(format!("{}:{number}: {target} (no such file)", page.display()));
                    continue;
                }
                if let Some(anchor) = anchor {
                    if path.extension().is_some_and(|ext| ext == "md") && !anchors(&read(&path)).contains(anchor) {
                        broken.push(format!("{}:{number}: {target} (no such heading)", page.display()));
                    }
                }
            }
        }
    }
    assert!(broken.is_empty(), "broken links:\n{}", broken.join("\n"));
}

#[test]
fn no_page_uses_a_banned_phrase_or_a_dash() {
    let phrases = banned_phrases();
    let mut found = Vec::new();
    for page in pages() {
        for (number, line) in checked_lines(&read(&page)) {
            let lower = line.to_lowercase();
            let mut problems: Vec<String> = phrases.iter().filter(|phrase| contains_phrase(&lower, phrase)).map(|phrase| format!("\"{phrase}\"")).collect();
            if line.contains('\u{2014}') || line.contains('\u{2013}') {
                problems.push("an em dash or an en dash".into());
            }
            if line.contains(" - ") {
                problems.push("a spaced hyphen used as a dash".into());
            }
            problems.extend(line.split_whitespace().filter(|word| is_hyphenated(word)).map(|word| format!("the hyphenated word {}", trim_word(word))));
            for problem in problems {
                found.push(format!("{}:{number}: {problem}", page.display()));
            }
        }
    }
    assert!(found.is_empty(), "docs/writing-style.md bans these:\n{}", found.join("\n"));
}

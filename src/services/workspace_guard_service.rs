//! The workspace guard (task-2712, E28): a command that names a path outside the working
//! directory is not run, and the model is told why. The check reads the command's text, so a
//! command that builds a path while it runs gets past it. It keeps the model from wandering
//! across the disk; it is not a security boundary.
//!
//! A word counts as a path when it starts with `/`, `~`, `$HOME`, `%USERPROFILE%` or a drive
//! letter and colon, or when it is a plain word with a `..` part. Git Bash forms map to Windows
//! forms (`/c/x` is `C:/x`, `/tmp` is the temp folder, any other `/x` is in the Git folder). A word
//! that starts with `/` and names no folder that exists, such as the regex `/foo/`, is not a path.
//! Heredoc bodies are skipped, and inside a quoted script only absolute paths count. The same
//! rules are in `evals/src/services/outsidePaths.mjs`, which the eval scorecard uses.

use std::path::Path;

use crate::models::action::Observation;
use crate::models::config::EnvironmentConfig;
use crate::models::message::{Message, Role};

/// The guard of one session: the folders it allows, the setting, and how many messages have been
/// read for paths the person typed.
pub struct GuardState {
    pub guard: WorkspaceGuard,
    pub mode: GuardMode,
    seen: usize,
}

impl GuardState {
    /// Builds the guard for a local environment, or None when the setting is `off` or commands
    /// do not run on this machine.
    /// @param config - the environment settings
    /// @param cwd - the session's working directory
    /// @param local - true when the environment runs commands on this machine
    /// @param umbra_home - Umbra's own folder
    pub fn from_config(config: &EnvironmentConfig, cwd: &Path, local: bool, umbra_home: &Path) -> Option<Self> {
        let mode = GuardMode::parse(&config.workspace_guard);
        (local && mode != GuardMode::Off).then(|| GuardState { guard: WorkspaceGuard::new(cwd, &config.allowed_paths, umbra_home), mode, seen: 0 })
    }

    /// Allows the paths in user messages the guard has not read yet: the words the person typed,
    /// kept in `extra.user_text`.
    /// @param messages - the conversation
    pub fn learn_user_paths(&mut self, messages: &[Message]) {
        if messages.len() < self.seen {
            self.seen = 0;
        }
        for message in &messages[self.seen..] {
            if message.role == Role::User {
                if let Some(text) = message.extra["user_text"].as_str() {
                    self.guard.allow_paths_in(text);
                }
            }
        }
        self.seen = messages.len();
    }

    /// Returns the first outside path a command names, given the shell's directory.
    /// @param command - the command text
    /// @param shell_dir - the shell's working directory as the shell wrote it, if it keeps one
    pub fn first_hit(&self, command: &str, shell_dir: Option<&str>) -> Option<String> {
        let cwd = shell_dir.map(|d| self.guard.shell_dir_path(d)).unwrap_or_else(|| self.guard.root_display.clone());
        self.guard.outside_paths(command, &cwd).into_iter().next()
    }

    /// The observation of a refused command, with the path recorded for the trajectory.
    /// @param path - the word that named the outside path
    pub fn refusal(&self, path: &str) -> Observation {
        Observation { blocked_path: Some(path.to_string()), ..Observation::failed("", refusal_message(path, &self.guard.root_display)) }
    }

    /// After a command: when the shell ended in a folder outside the allowed ones, returns the
    /// note for the model and the folder to move the shell back to.
    /// @param shell_dir - the shell's working directory after the command
    pub fn moved_out(&self, shell_dir: Option<&str>) -> Option<(String, String)> {
        let dir = self.guard.shell_dir_path(shell_dir?);
        if !self.guard.is_outside(&dir) {
            return None;
        }
        let root = self.guard.root_display.clone();
        Some((format!("\n[umbra: the shell's working directory `{dir}` was outside the working directory, so it was moved back to `{root}`]"), root))
    }
}

/// The text of the observation a refused command gets.
/// @param path - the word that named the outside path
/// @param root - the working directory
pub fn refusal_message(path: &str, root: &str) -> String {
    format!("Not run. `{path}` is outside the working directory `{root}`. Work only inside the working directory. If something you need is not there, say so in your reply and stop looking for it elsewhere.")
}

/// What `environment.workspace_guard` asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardMode {
    /// Refuse every command that names an outside path.
    Block,
    /// Ask the person in an interactive confirm mode session; refuse otherwise.
    Ask,
    /// No check.
    Off,
}

impl GuardMode {
    /// Reads the setting. Anything unknown is `block`, the default.
    /// @param value - the setting's text
    pub fn parse(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "off" => GuardMode::Off,
            "ask" => GuardMode::Ask,
            _ => GuardMode::Block,
        }
    }
}

/// The folders a session's commands may name, and the facts needed to map Git Bash paths.
#[derive(Debug, Clone)]
pub struct WorkspaceGuard {
    /// The working directory, normalized.
    pub root: String,
    /// The working directory as the person would read it, for messages.
    pub root_display: String,
    allowed: Vec<String>,
    home: String,
    temp: String,
    git_root: String,
    windows: bool,
}

/// One possible path taken from a command, and whether a relative `..` path counts.
struct Candidate {
    text: String,
    relative_allowed: bool,
}

impl WorkspaceGuard {
    /// Builds the guard for a session started in `root`.
    /// @param root - the session's working directory
    /// @param extra - folders from `environment.allowed_paths`
    /// @param umbra_home - Umbra's own folder
    pub fn new(root: &Path, extra: &[String], umbra_home: &Path) -> Self {
        let windows = cfg!(windows);
        let forward = |p: &Path| p.to_string_lossy().replace('\\', "/");
        let home = std::env::var("HOME").ok().filter(|h| !h.is_empty()).or_else(|| std::env::var("USERPROFILE").ok()).unwrap_or_default().replace('\\', "/");
        let mut guard = WorkspaceGuard { root: String::new(), root_display: forward(root), allowed: Vec::new(), home, temp: forward(&std::env::temp_dir()), git_root: git_root(), windows };
        guard.home = guard.map_bash_home(&guard.home.clone());
        guard.root = normalize(&guard.root_display, windows);
        let mut roots = vec![guard.root_display.clone(), guard.temp.clone(), forward(umbra_home)];
        roots.extend(program_folders(windows, &guard.git_root));
        roots.extend(path_folders());
        roots.extend(extra.iter().map(|p| guard.expand_home(p)));
        guard.allowed = roots.iter().filter(|r| !r.is_empty()).map(|r| normalize(r, windows)).collect();
        guard
    }

    /// Allows every absolute path named in a message the person typed, so a request about a file
    /// elsewhere on the disk still works.
    /// @param text - the person's message
    pub fn allow_paths_in(&mut self, text: &str) {
        for word in shell_words(text) {
            for candidate in candidates(&word) {
                if let Some(resolved) = self.resolve(&candidate, &self.root_display.clone()) {
                    if resolved != "/" {
                        self.allowed.push(normalize(&resolved, self.windows));
                    }
                }
            }
        }
    }

    /// Returns the words of a command that name a path outside the allowed folders, in order.
    /// @param command - the command text
    /// @param cwd - the shell's working directory before the command runs
    pub fn outside_paths(&self, command: &str, cwd: &str) -> Vec<String> {
        let words = shell_words(&without_heredoc_bodies(command));
        let mut hits = Vec::new();
        let mut cwd = cwd.to_string();
        let mut i = 0;
        while i < words.len() {
            let word = &words[i];
            if word.operator {
                i += 1;
                continue;
            }
            if word.text == "cd" && words.get(i + 1).is_some_and(|w| !w.operator) {
                let target = &words[i + 1].text;
                let resolved = self.resolve(&Candidate { text: target.clone(), relative_allowed: true }, &cwd);
                if resolved.as_deref().is_some_and(|r| self.is_outside(r)) {
                    hits.push(target.clone());
                }
                cwd = resolved.unwrap_or_else(|| format!("{cwd}/{target}"));
                i += 2;
                continue;
            }
            for candidate in candidates(word) {
                if self.resolve(&candidate, &cwd).is_some_and(|r| self.is_outside(&r)) {
                    hits.push(candidate.text);
                }
            }
            i += 1;
        }
        hits
    }

    /// Returns true when an absolute path is outside every allowed folder. The bare root `/` is
    /// always outside.
    /// @param resolved - an absolute path
    pub fn is_outside(&self, resolved: &str) -> bool {
        if resolved == "/" {
            return true;
        }
        let path = normalize(resolved, self.windows);
        !self.allowed.iter().any(|root| is_inside(&path, root))
    }

    /// Maps a shell working directory, such as `/c/x` from Git Bash, to an absolute path.
    /// @param shell_dir - the directory the shell reported
    pub fn shell_dir_path(&self, shell_dir: &str) -> String {
        if self.windows && shell_dir.starts_with('/') {
            return self.map_slash(shell_dir, false).unwrap_or_else(|| shell_dir.to_string());
        }
        shell_dir.to_string()
    }

    /// Turns one candidate into an absolute path, or None when it is not a path.
    /// @param candidate - the word or part of a word
    /// @param cwd - the directory a relative path is resolved against
    fn resolve(&self, candidate: &Candidate, cwd: &str) -> Option<String> {
        let text = candidate.text.trim_end_matches([',', ';', ':']);
        if let Some(rest) = home_rest(text) {
            return (rest.is_empty() || rest.starts_with(['/', '\\'])).then(|| format!("{}{rest}", self.home));
        }
        if has_drive(text) {
            return existing_drive_path(text).then(|| text.to_string());
        }
        if text.starts_with('/') {
            return self.map_slash(text, !candidate.relative_allowed);
        }
        if candidate.relative_allowed && has_parent_part(text) {
            return Some(format!("{cwd}/{text}"));
        }
        None
    }

    /// Maps a word starting with `/`. On Windows `/c/x` is `C:/x`, `/tmp` the temp folder and any
    /// other `/x` the Git folder. A word whose drive or first folder does not exist is not a path.
    /// @param text - the word
    /// @param in_code - true inside a quoted script, where a bare `/c` is not a path
    fn map_slash(&self, text: &str, in_code: bool) -> Option<String> {
        if text.chars().all(|c| c == '/') {
            return Some("/".into());
        }
        if !self.windows {
            let first = text.split('/').find(|p| !p.is_empty()).unwrap_or("");
            return Path::new(&format!("/{first}")).exists().then(|| text.to_string());
        }
        let bytes = text.as_bytes();
        let single_letter = bytes.len() >= 2 && bytes[1].is_ascii_alphabetic() && (bytes.len() == 2 || bytes[2] == b'/');
        if single_letter {
            let rest = text.get(3..).unwrap_or("");
            if in_code && rest.is_empty() {
                return None;
            }
            let mapped = format!("{}:/{rest}", (bytes[1] as char).to_ascii_uppercase());
            return existing_drive_path(&mapped).then_some(mapped);
        }
        if text == "/tmp" || text.starts_with("/tmp/") {
            return Some(format!("{}{}", self.temp, &text[4..]));
        }
        Some(format!("{}{text}", self.git_root))
    }

    /// Expands a leading `~` or `$HOME` in a setting value.
    /// @param value - a folder from `environment.allowed_paths`
    fn expand_home(&self, value: &str) -> String {
        match home_rest(value) {
            Some(rest) => format!("{}{rest}", self.home),
            None => value.replace('\\', "/"),
        }
    }

    /// Maps a home folder given in Git Bash form (`/c/Users/x`) to its Windows form.
    /// @param home - the HOME value
    fn map_bash_home(&self, home: &str) -> String {
        if self.windows && home.starts_with('/') {
            return self.map_slash(home, false).unwrap_or_else(|| home.to_string());
        }
        home.to_string()
    }
}

/// Returns what follows `~`, `$HOME`, `${HOME}` or `%USERPROFILE%` at the start of a word.
/// @param text - the word
fn home_rest(text: &str) -> Option<&str> {
    for prefix in ["${HOME}", "$HOME", "%USERPROFILE%", "~"] {
        if text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix) {
            let rest = &text[prefix.len()..];
            if prefix == "$HOME" && rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            return Some(rest);
        }
    }
    None
}

/// Returns true when a word starts with a drive letter and colon followed by a slash or nothing.
/// @param text - the word
fn has_drive(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || b[2] == b'/' || b[2] == b'\\')
}

/// Returns true when a Windows path's drive exists and so does its first folder, if it names one.
/// `f:\nRET` inside a script is text, not a path.
/// @param text - a path starting with a drive letter
fn existing_drive_path(text: &str) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let root = format!("{}:/", text[..1].to_ascii_uppercase());
    if !Path::new(&root).exists() {
        return false;
    }
    let first = text.get(3..).unwrap_or("").split(['/', '\\']).next().unwrap_or("");
    first.is_empty() || Path::new(&format!("{root}{first}")).exists()
}

/// Returns true when a relative word has a `..` part.
/// @param text - the word
fn has_parent_part(text: &str) -> bool {
    text.split(['/', '\\']).any(|part| part == "..")
}

/// Puts a path in the form comparisons use: forward slashes, `.` and `..` resolved, no slash at
/// the end, and lower case on Windows.
/// @param value - an absolute path
/// @param windows - true on Windows
pub fn normalize(value: &str, windows: bool) -> String {
    let forward = value.replace('\\', "/");
    let (drive, rest) = if has_drive(&forward) { (forward[..2].to_uppercase(), &forward[2..]) } else { (String::new(), forward.as_str()) };
    let mut parts: Vec<&str> = Vec::new();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    let joined = format!("{drive}/{}", parts.join("/"));
    if windows { joined.to_lowercase() } else { joined }
}

/// Returns true when `inner` is `root` or a path inside it. Both are normalized.
/// @param inner - the path
/// @param root - the folder
fn is_inside(inner: &str, root: &str) -> bool {
    inner == root || inner.starts_with(&format!("{}/", root.trim_end_matches('/')))
}

/// The Git for Windows install folder, which Git Bash shows as `/`.
fn git_root() -> String {
    ["C:/Program Files/Git", "C:/Program Files (x86)/Git"].iter().find(|p| Path::new(p).exists()).unwrap_or(&"C:/Program Files/Git").to_string()
}

/// The program folders commands may always name.
/// @param windows - true on Windows
/// @param git_root - the Git install folder
fn program_folders(windows: bool, git_root: &str) -> Vec<String> {
    let list: &[&str] = if windows { &["C:/Program Files", "C:/Program Files (x86)", "C:/Windows"] } else { &["/dev", "/usr", "/bin", "/sbin", "/etc", "/opt", "/proc", "/lib", "/Library", "/System", "/Applications"] };
    let mut out: Vec<String> = list.iter().map(|s| s.to_string()).collect();
    if windows {
        out.push(git_root.to_string());
    }
    out
}

/// The folders on PATH, which hold the programs commands run.
fn path_folders() -> Vec<String> {
    let Some(path) = std::env::var_os("PATH") else { return Vec::new() };
    std::env::split_paths(&path).filter(|p| p.is_absolute()).map(|p| p.to_string_lossy().replace('\\', "/")).collect()
}

/// One shell word, or an operator such as `|`, `;` or `>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    /// A quoted part of the word held whitespace: it is a script, not a path.
    pub code: bool,
    pub operator: bool,
}

/// Removes the bodies of heredocs, the text a command writes into a file.
/// @param command - the command text
pub fn without_heredoc_bodies(command: &str) -> String {
    let mut kept = Vec::new();
    let mut terminator: Option<String> = None;
    for line in command.split('\n') {
        if let Some(end) = &terminator {
            if line.trim_end_matches('\r').trim() == end {
                terminator = None;
            }
            continue;
        }
        kept.push(line);
        terminator = heredoc_marker(line);
    }
    kept.join("\n")
}

/// Returns the terminator a line's heredoc ends with, such as `EOF` for `<<'EOF'`.
/// @param line - one line of a command
fn heredoc_marker(line: &str) -> Option<String> {
    let start = line.find("<<")?;
    let rest = line[start + 2..].trim_start_matches('-').trim_start();
    let rest = rest.trim_start_matches(['\'', '"']);
    let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    (!name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit())).then_some(name)
}

/// Splits a command into shell words, keeping quoted text together. Backslashes are kept as they
/// are, so a Windows path stays one word.
/// @param command - the command text
pub fn shell_words(command: &str) -> Vec<Word> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut spaced = false;
    let flush = |current: &mut String, spaced: &mut bool, words: &mut Vec<Word>| {
        if !current.is_empty() {
            words.push(Word { text: std::mem::take(current), code: *spaced, operator: false });
        }
        *spaced = false;
    };
    for ch in command.chars() {
        if let Some(q) = quote {
            if ch == q { quote = None } else { current.push(ch); spaced |= ch.is_whitespace(); }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch.is_whitespace() || ";|&<>()`".contains(ch) {
            flush(&mut current, &mut spaced, &mut words);
            if !ch.is_whitespace() {
                words.push(Word { text: ch.to_string(), code: false, operator: true });
            }
        } else {
            current.push(ch);
        }
    }
    flush(&mut current, &mut spaced, &mut words);
    words
}

/// Returns the parts of a word to look at as possible paths. A plain word is split at `=`, so
/// `--out=/c/x` gives `/c/x`. A script is split at whitespace and punctuation, and only its
/// absolute parts count.
/// @param word - the shell word
fn candidates(word: &Word) -> Vec<Candidate> {
    if !word.code || looks_absolute(&word.text) {
        return word.text.split('=').filter(|p| !p.is_empty()).map(|p| Candidate { text: p.to_string(), relative_allowed: !word.code }).collect();
    }
    let separators = |c: char| c.is_whitespace() || "'\"`(),;=[]{}".contains(c);
    word.text.split(separators).filter(|p| looks_absolute(p) && !p.starts_with("//") && *p != "/").map(|p| Candidate { text: p.to_string(), relative_allowed: false }).collect()
}

/// Returns true when a word starts the way an absolute path does.
/// @param text - the word
fn looks_absolute(text: &str) -> bool {
    text.starts_with('/') || home_rest(text).is_some() || has_drive(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A guard rooted in a made up folder under the temp folder, with no extra roots from PATH.
    fn guard() -> WorkspaceGuard {
        let root = std::env::temp_dir().join("umbra-guard-test").join("ws");
        let mut g = WorkspaceGuard::new(&root, &[], Path::new("/nonexistent/.umbra"));
        // The temp folder holds the test root, so the test uses a root outside it: the home folder.
        let home = g.home.clone();
        g.root_display = format!("{home}/umbra-guard-ws");
        g.root = normalize(&g.root_display, g.windows);
        let temp = normalize(&g.temp, g.windows);
        g.allowed = vec![g.root.clone(), temp, normalize(&g.git_root, g.windows)];
        g
    }

    #[test]
    fn blocks_the_commands_that_leave_the_workspace() {
        let g = guard();
        let root = g.root_display.clone();
        assert_eq!(g.outside_paths("find ~ -name 'customers-*.json'", &root), vec!["~"]);
        assert_eq!(g.outside_paths("cd .. && ls", &root), vec![".."]);
        assert_eq!(g.outside_paths("find / -name a", &root), vec!["/"]);
        assert_eq!(g.outside_paths("ls $HOME", &root), vec!["$HOME"]);
    }

    #[test]
    fn allows_the_commands_that_stay_inside() {
        let g = guard();
        let root = g.root_display.clone();
        assert!(g.outside_paths("cd src && cat ../README.md && cd ..", &root).is_empty());
        assert!(g.outside_paths("sed -n '/start/,/end/p' a.txt > /dev/null 2>&1", &root).is_empty());
        assert!(g.outside_paths("cat <<'EOF' > a.js\nrequire('../../x')\nEOF", &root).is_empty());
        assert!(g.outside_paths("node -e \"console.log(1 / 2) // half\"", &root).is_empty());
        assert!(g.outside_paths("python -c 'print(1)'", &root).is_empty());
        assert!(g.outside_paths(&format!("cat \"{root}/a b/c.txt\""), &root).is_empty());
    }

    #[test]
    fn a_path_the_person_typed_is_allowed() {
        let mut g = guard();
        let root = g.root_display.clone();
        let elsewhere = format!("{}/other-project", g.home);
        assert_eq!(g.outside_paths(&format!("ls {elsewhere}"), &root), vec![elsewhere.clone()]);
        g.allow_paths_in(&format!("Look at {elsewhere}/notes.txt and {elsewhere}"));
        assert!(g.outside_paths(&format!("ls {elsewhere}/x"), &root).is_empty());
    }

    #[test]
    fn git_bash_paths_map_to_windows_paths() {
        if !cfg!(windows) {
            return;
        }
        let g = guard();
        let root = g.root_display.clone();
        assert_eq!(g.outside_paths("cat /c/Windows/../Users/x", &root), vec!["/c/Windows/../Users/x"]);
        assert!(g.outside_paths("ls /tmp/x", &root).is_empty());
        assert_eq!(g.shell_dir_path("/c/Users"), "C:/Users");
    }

    #[test]
    fn modes_parse_with_block_as_the_default() {
        assert_eq!(GuardMode::parse("off"), GuardMode::Off);
        assert_eq!(GuardMode::parse("ASK"), GuardMode::Ask);
        assert_eq!(GuardMode::parse(""), GuardMode::Block);
        assert_eq!(GuardMode::parse("nonsense"), GuardMode::Block);
    }
}

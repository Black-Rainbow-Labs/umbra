# Umbra fixes part 5 (task-2625)

## Introduction

This ticket asks for six changes to Umbra 0.6.2:

1. The status bar is drawn below the input line instead of above it.
2. Collapsing a long command group in the fullscreen view leaves most of the screen black until the
   person scrolls.
3. Code blocks in replies get syntax colors.
4. MCP servers are turned on and off with a checkbox per server, for this session or for every
   session. Typing server names into the `only` and `disabled` text fields is no longer needed.
5. Umbra gets a command line for every setting and every built in command, and the agent knows how
   to use it.
6. Umbra checks for a new release once a day. When one exists, a prompt at the bottom offers to
   install it or dismiss it. Installing replaces the binary, restarts Umbra and resumes the session.

## Goals

| # | Goal | How it is checked |
|---|---|---|
| G1 | The input line is above the status bar in the fullscreen view. | Frame test: the last row is the status bar, the row above it is the input line. Pseudo terminal test. |
| G2 | After any expand or collapse the transcript fills the screen and the toggled header stays on the row it was on. | Frame test that expands a group of 200 output rows, scrolls into it, collapses it, and checks that no transcript row is empty while rows exist above. |
| G3 | Fenced code blocks with a known language get colors for keywords, strings, comments, numbers, types and function names. `syntax_highlight = false` turns it off. | Unit tests per language. Golden files without color stay unchanged. |
| G4 | `/mcp` shows a checkbox per server and a switch between "this session" and "all sessions". The config modal's MCP section shows a checkbox per server in place of the `only` and `disabled` text fields. | Unit tests on the list editing. Functional test that toggles a server for all sessions and reads `~/.umbra/config.toml`. |
| G5 | `umbra config get/set/unset/list/path`, `umbra mcp enable/disable`, `umbra models`, `umbra session <command>` and `umbra update` exist. The system prompt tells the agent about them. | Integration tests that run each subcommand against a temporary home folder. A test that drives a running session with `umbra session mode yolo`. |
| G6 | One update check a day by default. A newer release shows a prompt with Install, Remind me tomorrow and Skip this version. Install downloads the release archive, checks it against `SHA256SUMS`, replaces the binary and restarts into the same session. | Integration test against a local HTTP server that serves a fake release. |

## Non goals

- No new syntax highlighting crate. A small highlighter covers the languages people paste most.
- No automatic update without asking. The check is automatic, the install is not.
- No update path for package managers other than Homebrew. A cargo install and an install script
  install are both replaced in place.

## 1. Status bar below the input line

`ui::frame::render_frame` pushes rows in this order today: transcript, the "more rows below" note,
panel rows, status bar, input line. The new order is transcript, note, panel rows, input line,
status bar. The cursor row is computed from where the input starts, so the change is to push the
status row after the input rows and to keep `input_top` pointing at the first input row.

The inline view (`--no-fullscreen`) draws its status bar under the output and the prompt is read on
the line after it. That view has no input line of its own while the agent works, so it stays as it
is.

## 2. The black screen after a collapse

`View::scroll` counts transcript rows below the bottom of the view. `View::activate` flips
`group.expanded` or `run.expanded` and does not touch `scroll`. When the person scrolled up into a
long expanded output and collapses it, the transcript loses hundreds of rows but `scroll` still
counts them. `frame::visible_rows` then takes rows from `scroll` to `scroll + room` of a list that
is now shorter than that, so it returns a few rows or none, and the rest of the screen is drawn
empty. Scrolling with the wheel calls `scroll_by`, which clamps, and that is why scrolling repaints
it.

The fix has two parts:

- `View::activate` remembers the screen row of the toggled header before the toggle, and after the
  toggle sets `scroll` so the header is on that same row. A collapse then keeps the header under the
  mouse, the way an editor's fold does.
- `render_frame` clamps `scroll` to `total_rows - room` on every frame. Any other change that
  shortens the transcript (a resize to a wider screen, a block dropped by the row cap) can then never
  leave the view past the top.

## 3. Syntax highlighting

### Design

A new module `src/ui/highlight.rs` turns one code line into colored spans. It keeps a small state
between lines for block comments and strings that span lines. Each language is a `LanguageSpec`:

```rust
pub struct LanguageSpec {
    pub names: &'static [&'static str],        // fence names: "rust", "rs"
    pub line_comments: &'static [&'static str], // "//", "#", "--"
    pub block_comment: Option<(&'static str, &'static str)>,
    pub strings: &'static [char],               // '"', '\'', '`'
    pub keywords: &'static [&'static str],
    pub types: &'static [&'static str],         // built in type names; capitalized names count as types too
    pub literals: &'static [&'static str],      // true, false, null, None
}
```

Languages: Rust, Python, JavaScript, TypeScript, JSX/TSX, Go, C, C++, C#, Java, Kotlin, Swift, Ruby,
PHP, shell (bash, sh, zsh, console), PowerShell, SQL, JSON, TOML, YAML, HTML and XML (tags and
attributes), CSS, Lua, and diff. A fence with no language or an unknown one is not colored.

Colors use the 16 basic ANSI colors so they follow the terminal's theme:

| Token | Code |
|---|---|
| keyword | 35 (magenta) |
| type | 36 (cyan) |
| string | 32 (green) |
| number and literal | 33 (yellow) |
| comment | 90 (gray) and 3 (italic) |
| function name (an identifier followed by `(`) | 34 (blue) |
| diff added, removed, hunk | 32, 31, 36 |

`markdown::finish_code` calls the highlighter when color is on and `syntax_highlight` is true, then
wraps the colored spans with the existing hard wrap, so a long line is still cut at the width and a
copy still leaves out the border. The shell command shown when a command run is expanded
(`$ cargo test --lib`) is colored as shell.

`syntax_highlight = true` is a new top level setting in the Display section of the schema.

### Why no crate

`syntect` gives better results but adds about 2 MB of syntax definitions to a binary that is 6 MB
today, and its regex engine is slow to start. `tree-sitter` needs a C compiler and a grammar per
language, which breaks the static musl build. The table driven lexer is about 500 lines and covers
what a terminal reply needs.

## 4. MCP checkboxes for this session or all sessions

### The `/mcp` panel

```
 MCP servers        applies to: [this session] all sessions        Space toggles · s switches · Esc closes
 › [x] chordical      running    42 tools
   [x] termoil        ⠋ starting
   [ ] blender        off
   [x] nikaya         ✖ failed   did not finish starting within 30 seconds
```

- `s`, Left, Right or a click on the `applies to` row switches the scope.
- **This session**: the checkbox shows whether the server runs in this session. Space or a click
  turns it on or off now, as today. The config file is not changed. A resumed session keeps the
  choice through `mcp_off` in the session index, as today.
- **All sessions**: the checkbox shows whether the user config file lets the server start. Space or
  a click changes `~/.umbra/config.toml` and also turns the server on or off in this session.
  - Off: the name is added to `mcp.disabled` and removed from `mcp.only`.
  - On: the name is removed from `mcp.disabled`. When `mcp.only` is not empty, the name is added to
    it, because a server outside a non empty `only` list does not start.
  - When the project file `.umbra/config.toml` still turns the server off, the row says
    `off in the project file`.

The list editing is one function, `mcp_settings_service::server_lists(only, disabled, name, on)`,
which returns the new `only` and `disabled` lists. The panel, the config modal and the command line
all call it.

### The config modal

The MCP section drops the `Only` and `Disabled` text fields and lists each server found with a
checkbox and the file it came from:

```
 Enabled            [x]
 Startup timeout    30
 ...
 Servers            Space turns a server on or off for every session
   [x] chordical    ~/.claude.json
   [ ] blender      ~/.claude.json
```

Space stages the change as edits of `mcp.only` and `mcp.disabled`, which Ctrl+S writes to the file
the scope switch chooses (user or project). After a save the modal also turns each changed server on
or off in the running session.

`mcp.only` and `mcp.disabled` keep their entries in the settings schema, so `umbra config set
mcp.disabled ...` and the schema coverage test still work.

## 5. The `umbra` command line

### Subcommands

| Command | What it does |
|---|---|
| `umbra config` | Prints the merged config, as today |
| `umbra config list [--json]` | Every setting: path, value, where it comes from, and its help text |
| `umbra config get <path>` | One value, as TOML |
| `umbra config set <path> <value> [--project]` | Parses the value with the setting's kind (the same parser the modal uses), writes it with comments kept |
| `umbra config unset <path> [--project]` | Removes the key from the file |
| `umbra config path [--project]` | The file `set` writes |
| `umbra mcp [--connect]` | Lists the servers, as today |
| `umbra mcp enable <name> [--project]` and `umbra mcp disable <name> [--project]` | Changes `mcp.only` and `mcp.disabled` for every session |
| `umbra models` | The default model, the named models, the `models` list and the known models |
| `umbra session status` | The running session's id, model, mode and MCP servers |
| `umbra session model <name>` | Switches the running session's model from its next model call |
| `umbra session mode <confirm,yolo,human>` | Switches the running session's mode |
| `umbra session mcp <on,off> <name>` | Turns a server on or off in the running session only |
| `umbra session compact [focus]` | Summarizes the running session's history before its next model call |
| `umbra session quit` | Ends the running session after the current step |
| `umbra update [--check]` | Checks for a newer release, and installs it unless `--check` is given |

Every slash command has a command line form: `/config` is `umbra config`, `/model` is
`umbra session model`, `/mcp` is `umbra session mcp` and `umbra mcp`, `/mode` is
`umbra session mode`, `/compact` is `umbra session compact`, `/resume` is `umbra resume`,
`/skills` is `umbra skills`, `/quit` is `umbra session quit`. `/clear` and `/help` have no command
line form, because they only change the screen.

### How `umbra session` reaches the running session

Commands the agent runs are child processes of Umbra, so the session gives them its address:

- When a session starts, Umbra makes `~/.umbra/control/<session id>/` and writes `state.json` there:
  the session id, the process id, the model, the mode, and each MCP server with its state. It is
  written again after every step and whenever the model, mode or a server changes.
- Every command the agent runs gets `UMBRA_SESSION_ID`, `UMBRA_CONTROL_DIR` and `UMBRA_BIN` (the
  running binary's path) in its environment. `local.rs` and `docker.rs` add them, next to the
  configured `environment.env`.
- `umbra session <command>` reads `UMBRA_CONTROL_DIR`, or `--session <id>` to address a session
  from another terminal, and appends one JSON line to `requests.jsonl` in that folder. It prints
  what will happen and when ("the model changes from the next model call").
- The agent reads and empties `requests.jsonl` in `apply_ui_requests`, which runs before every
  model call and at the idle prompt. Each line becomes the same `UiRequest` the fullscreen view
  sends, so `/model` and `umbra session model` share one code path.
- The folder is removed when the session ends. A folder whose process id is no longer running is
  removed by the next session that starts.

### What the agent is told

The reply style system prompt gets a short section:

```
# Umbra's own command line

You run inside Umbra. Its command line is `{{umbra_bin}}`. Use it when the user asks you to change
Umbra itself:
- Settings: `umbra config list`, `umbra config get <key>`, `umbra config set <key> <value>`, `umbra config unset <key>`
- MCP servers for every session: `umbra mcp`, `umbra mcp enable <name>`, `umbra mcp disable <name>`
- This session: `umbra session status`, `umbra session model <name>`, `umbra session mode <mode>`, `umbra session mcp off <name>`, `umbra session compact`
- Models: `umbra models`. Updates: `umbra update --check`.
Run `umbra <command> --help` for the options.
```

It is about 150 tokens. The submit style prompt, which benchmarks use, does not get it.

## 6. Update checks

### Settings

```toml
[updates]
check = true            # check for a new release
interval_hours = 24     # how often
```

`UMBRA_UPDATE_CHECK=0` turns the check off. `UMBRA_TEST_UPDATE_URL` replaces
`https://api.github.com/repos/Black-Rainbow-Labs/umbra/releases` for tests.

### The check

`src/services/update_service.rs`:

- `due(state, settings, now)`: true when the last check is older than `interval_hours`.
- `check()`: `GET {base}/latest` with a `User-Agent: umbra/<version>` header and a 10 second
  timeout. Reads `tag_name`, strips the `v`, and compares the version numbers to
  `CARGO_PKG_VERSION`. Writes `~/.umbra/update.json`:
  `{"checked_at": <seconds>, "latest": "0.7.0", "skipped": "0.6.5"}`.
- The check runs on a background thread when an interactive session starts, so it never delays the
  first prompt. A failed check is silent and is tried again at the next interval.
- ACP, `--quiet`, pipes and the inline view under the Tasks board do not check. The `umbra update`
  command checks whenever it runs.

### The prompt

When the check finds a version newer than this one and not skipped, the view opens an update prompt
above the status bar the next time the agent is idle and the input line is empty:

```
 ▲ Umbra 0.7.0 is available. You have 0.6.2.
   › Install and restart     Remind me tomorrow     Skip this version
```

Left, Right, Tab and the mouse choose, Enter confirms, Esc is Remind me tomorrow. Typing a
character closes the prompt and types the character, so the prompt never eats a message. Skip this
version writes `skipped` to `update.json`.

### Install and restart

Install sends `/update` as a line, which the agent handles at the idle prompt like `/clear`:

1. Saves the session.
2. Finds how Umbra was installed. A binary under a Homebrew `Cellar` folder is upgraded with
   `brew upgrade black-rainbow-labs/umbra/umbra`. Anything else is replaced in place.
3. Replaces in place: downloads `umbra-<target>.tar.gz` (`.zip` on Windows) and `SHA256SUMS` from
   `releases/download/v<version>/`, checks the archive's SHA-256 with the `sha2` crate, extracts it
   with the system `tar` (Windows 10 and later ship `tar.exe`, which reads zip files) into a
   temporary folder next to the binary, runs the new binary with `--version` to check it starts, and
   renames it over the old one. On Windows the running binary cannot be overwritten, so it is
   renamed to `umbra.exe.old` first and removed at the next start.
4. Leaves the fullscreen view, closes the MCP servers, and starts the new binary with
   `--resume <session id>` and the options this run was started with (mode, `--no-fullscreen`,
   `--no-mouse`, `--keep-open`, `--cwd`, `-c` files, MCP and source options). On Unix it replaces
   the process with `exec`, so the shell sees one program. On Windows it waits for the child and
   exits with its code.
5. A failure at any step leaves the old binary in place, prints the reason in the transcript, and
   the session carries on.

`/update` typed while the agent works is kept until the turn ends.

## Testing

| Test | File |
|---|---|
| Status bar is the last row, input above it | `src/ui/frame.rs` unit test, `tests/fullscreen.rs` |
| Collapse keeps the header row and fills the screen | `src/ui/view.rs` unit test |
| Highlighter tokens for Rust, Python, TypeScript, shell, JSON, diff; a block comment over two lines | `src/ui/highlight.rs` unit tests |
| Golden markdown files without color are unchanged | `tests/` golden test, as today |
| `server_lists` for off, on with an empty `only`, on with a non empty `only` | `src/services/mcp_settings_service.rs` |
| `umbra config set/get/unset/list`, `umbra mcp disable/enable`, `umbra models` with a temporary `HOME` | `tests/cli.rs` |
| `umbra session mode yolo` run by the agent switches the mode of the running session | `tests/cli.rs`, replay provider |
| Update check against a local server, `update.json` written, skip honored | `tests/update.rs` |
| `umbra update` replaces a copy of the binary with the release from a local server, after the checksum passes, and refuses a wrong checksum | `tests/update.rs` with `UMBRA_UPDATE_TARGET` pointing at a copy |
| Install from the prompt restarts into the same session | `tests/fullscreen.rs` in a pseudo terminal |

## Changes made during the build

| Area | The design said | The build does | Why |
|---|---|---|---|
| ACP and the control folder | ACP sessions read `requests.jsonl` too | Only terminal sessions have a control folder | One ACP process serves many sessions, and the control folder is one per process |
| `state.json` | Written after every step | Written when the state changed since the last write | A write before every step delayed the first model call enough to change the order of piped input in `tests/images.rs` |
| The update prompt | Opens when the agent is idle | Opens at the first idle prompt of a session, once; typing closes it and the next session offers it again | The prompt must never take a message being typed |
| A click on the update prompt | Chooses the clicked choice | Confirms the selected choice | The panel rows are hit tested a whole row at a time |
| `/update` at the first prompt | Not covered | Installs and starts the new binary without `--resume`, because no session exists yet | |

## Rollout

Release as Umbra 0.7.0, because `umbra session` and the update check are new behavior. The update
check first fires on 0.7.0, so people on 0.6.x update the usual way once.

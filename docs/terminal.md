# The terminal view

This page describes everything on Umbra's screen in a terminal: the header, the transcript, the
commands that start with `/`, the pickers, the configuration modal, the status bar and every key.

## Terms used on this page

| Term | Meaning |
|---|---|
| Fullscreen view | Umbra's screen on the terminal's alternate screen, the way `vim` and `less` draw |
| Inline view | Printing below the shell prompt, with no fullscreen view |
| Transcript | The scrolling record of the session |
| Command group | Consecutive shell commands shown as one collapsed row |
| Picker | A list to choose from, above the input line |

## The fullscreen view

In a terminal Umbra opens on the alternate screen, the way `vim`, `less` and Claude Code's
fullscreen mode do. The session starts on a clean screen. Scrolling up shows the conversation and
never the shell's earlier output. When Umbra exits, the shell's screen comes back as it was, with
the `trajectory <path>` line printed under it.

The screen has four parts, from top to bottom:

```text
┌──────────────────────────────────────────────────────────────┐
│ transcript: the header, your prompts, replies, command groups │
│                                                              │
│ a popup, a picker or a panel, when one is open               │
│ › the input line, always there                               │
│ context ███░░░  12% · 24.1k / 200k │ 0:42 │ $0.01 │ haiku-5-5  │
└──────────────────────────────────────────────────────────────┘
```

The input line is there while the agent works, so a message can be typed at any time. The status
bar is the last row.

`--no-fullscreen`, `fullscreen = false` or `UMBRA_FULLSCREEN=0` prints inline below the shell prompt
instead. A pipe, `--quiet` and `TERM=dumb` always get the inline view.

## The header

The header is the first block of the transcript and scrolls with it. It shows the umbra logo, the
version, and these rows:

| Row | What it shows |
|---|---|
| `model` | The model of this session. A click on the row, or `/model`, opens the model picker |
| `folder` | The folder Umbra works in |
| `mode` | `confirm`, `yolo` or `human` |
| `skills` | How many skills were found |
| `mcp` | `starting 2 of 5` with a spinner while the servers start, then `5 running`, `4 running · 1 failed`, `3 running · 2 off`, `none` or `off`. A click on the row, or `/mcp`, opens the MCP panel |

The logo's cream letters show first, and its green, yellow, orange, red and blue stripes appear one
at a time over about 1.4 seconds. The input line takes typing during that time.

| To | Do this |
|---|---|
| Show the finished logo at once | `UMBRA_BANNER_ANIMATION=0` |
| Leave the header out | `banner = false`, or `UMBRA_BANNER=0` |

The logo goes beside the text when both fit, above the text when only the logo fits, and is left
out in a terminal narrower than about 64 columns.

## Replies and commands in the transcript

A reply is drawn from its Markdown: headings, bold, italic, strikethrough, inline code, fenced code,
lists, quotes, tables, links and rules. Code in a fenced block is colored by the fence's language.

Consecutive shell commands are one collapsed row:

```text
 ▸ ✔ Ran 2 shell commands · 0:42
 ▸ ✖ Ran 3 shell commands, 1 failed · 0:42
 ▸ ⠋ Running 4 shell commands
```

| What you see | What it means |
|---|---|
| `▸` | The group is closed. `▾` means it is open |
| `✔` | Every command in the group exited with code 0 |
| `✖` and `1 failed` | One command exited with another code |
| A spinner and `Running` | A command of the group is running |
| `0:42` | The total time of the commands, as `m:ss` |
| `and 1 MCP call` | The group also holds an MCP tool call |

A click on the row, or Enter when it has focus, shows one row for each command with its time and
exit code. Opening a command shows the whole command, colored as shell, and its output. A reply
with text between two commands closes the group, so the order on the screen is the order things
happened.

| Mark | Meaning |
|---|---|
| `▲` in yellow | A warning |
| `✖` in red | A failure |
| `↻` | A compaction that freed space |
| `↺` | A reply with no usable command, sent back to the model |
| `+` | An attached image |

A turn that ends normally prints no summary line. A turn that stops for another reason prints one
line that says why, such as `▲ Interrupted` or a step or cost limit. `--verbose`, or
`verbose = true`, shows every detail: the step number and cost before each reply, command output up
to 2,000 characters, and the whole text of a format error.

### Syntax colors

`syntax_highlight = true`, the default, colors fenced code blocks and the shell command of an
opened command. Umbra reads Rust, Python, JavaScript and TypeScript, Go, C and C++, Java, Kotlin,
C#, Swift, Ruby, PHP, shell, PowerShell, SQL, JSON, TOML, YAML, CSS, Lua, HTML and XML, and diff. A
fence with no language or another language is drawn without colors. The colors are the 16 basic
terminal colors, so they follow the terminal's theme.

## Commands that start with `/`

Typing `/` as the first character opens a list of commands above the input line. It holds up to 8
rows, each with the name, its kind (`command`, `custom` or `skill`) and a short description.
Matching is by the start of the name first, then by any part of the name.

| Key | What it does in the list |
|---|---|
| Up, Down | Move in the list |
| Tab, or Enter on a partial name | Complete the name and add a space |
| Enter on a complete name | Run it |
| Esc | Close the list and keep the text |

| Command | What it does |
|---|---|
| `/config` | Opens the configuration modal |
| `/model [name]` | Opens the model picker, or switches to the named model |
| `/mcp` | Opens the MCP panel: the servers and their state, with a checkbox for each |
| `/mode [confirm, yolo, human]` | Switches the mode, or opens a picker |
| `/clear` | Saves this session and starts a new one with an empty history. The old session stays in the resume list |
| `/compact [focus]` | Summarizes the history now, keeping what `focus` names |
| `/resume` | Opens the session picker |
| `/skills` | Opens a picker of skills. Enter puts `/<skill> ` in the input line |
| `/update` | Installs the newest release and restarts into this session. Typed while the agent works, it runs when the turn ends |
| `/help` | Lists the commands and the keys |
| `/quit` | Ends the session |

The list also holds your [slash commands](discovery.md#slash-commands) and your skills. A built in
name wins when a slash command or a skill has the same name.

A command works on the first prompt, on a follow up prompt, and while the agent works. A command
that only changes the screen (`/config`, `/mcp`, `/model`, `/help`) runs at once while the agent
works. A command that needs the agent to be idle is refused with a one line notice. The short
commands inside the confirm, human and Ctrl+C prompts (`/y`, `/u`, `/c`, `/q`, `/h`) keep working.

## Pickers

`/model`, `/mode`, `/resume` and `/skills` open a list above the input line.

| Key | What it does |
|---|---|
| Up, Down | Move |
| Enter | Choose |
| Typing | Filter the list |
| Esc | Close |

**The model picker** lists the models from `[models.<name>]` and `models = [...]` in the config,
marks the current one, and ends with `Other...` for a `provider/model` typed by hand. A model whose
credential is missing shows the error, and the model does not change. A model chosen while the agent
works applies from the next model call. The choice lasts for the session and is not written to the
config file. See [Models](providers.md).

**The session picker** shows two rows for each session: its title with the age, the model and the
number of model calls, and under it a one sentence summary. See
[Sessions, Ctrl+C and images](sessions.md#sessions).

## The MCP panel

`/mcp`, or a click on the `mcp` row of the header, opens a panel with a switch between **this
session** and **all sessions**, then one row for each server: a checkbox, the name, the state, the
number of tools, and for a failed server the first line of the error.

```text
 MCP servers                       Space toggles · s switches scope · r restarts · Esc closes
   applies to  (•) this session   ( ) all sessions
 › [x] chordical        running    42 tools
   [ ] blender          off
```

[MCP](mcp.md#the-mcp-panel) explains each key and what each scope changes.

## The configuration modal

`/config` opens a modal with sections on the left and the fields of the selected section on the
right. Every setting in [Configuration](configuration.md) is a field in it.

| Key | What it does |
|---|---|
| Up, Down | Move between sections or fields |
| Tab | Switch between the section list and the fields |
| Enter or Space | Edit the selected field |
| Ctrl+S | Save. Only the changed keys are written, and the comments and key order in the file are kept |
| Ctrl+T | Switch between the user file `~/.umbra/config.toml` and the project file `<project>/.umbra/config.toml` |
| Esc | Close. When there are unsaved changes, Umbra asks whether to save or discard them |

The MCP section shows a checkbox for each server found. Each provider has a Test button that checks
the credential, checks that the server answers, and asks for a "Say hello." completion. See
[Testing a provider](providers.md#testing-a-provider).

## The status bar

```text
 context ████████░░░░░░░░░░░┊░░░░  33% · 42.6k / 128k │ ⠋ 3:07 │ $0.43 │ qwen-3.8 │ week ███░░░ 23%        ▲ 1 MCP error
```

The status bar is the last row of the fullscreen view. It is drawn again after every line Umbra
prints and once a second while the agent works.

| Part | What it shows |
|---|---|
| context bar | How full the context window is. The bar fills in eighths of a cell, and `┊` marks where compaction starts. It is green while the request is under 60% of the compaction point, yellow under 85%, and red past that. The label reads `compacting` during a compaction |
| `33% · 42.6k / 128k` | The estimated size of the next request, as a share of the window and in tokens. Compaction is decided on this number |
| `3:07` | The agent time, as `m:ss`, or `h:mm:ss` from one hour. It counts only while the agent works, so it does not move while Umbra waits for you. It adds up across turns and across a resume. A spinner shows while the agent works |
| `$0.43` | The cost of the session. Left out for a free local model |
| model | The model's short name |
| `week ███░░░ 23%` | The share of the weekly limit used, for a Claude login only. Green below 70%, yellow below 90%, red from 90%, dim when the last poll was refused. `usage.enabled = false` turns it off. See [Models](providers.md#claude) |
| `▲ 1 MCP error` | At the right edge, in red, only when an MCP server failed to start. A click on it, or Enter on it, lists each failed server with its error and its log file |

When the terminal is narrow, the parts get shorter or are left out in this order: the weekly usage
shrinks to the percent, the context bar gets shorter, the model is left out, then the cost. The
agent time and the MCP error box stay.

| To | Do this |
|---|---|
| Turn the status bar off | `--no-status-bar`, `status_bar = false`, or `UMBRA_STATUS_BAR=0` |
| Draw it into a pipe, as the tests do | `UMBRA_STATUS_BAR=1` |

## The update prompt

When Umbra finds a newer release, a prompt above the input line offers **Install and restart**,
**Remind me tomorrow** and **Skip this version** once the agent is idle. Install downloads the
release for this machine, checks it against the release's `SHA256SUMS`, replaces the program, and
starts the new version with `--resume` on the same session. `updates.check = false` or
`UMBRA_UPDATE_CHECK=0` turns the check off. See [Configuration](configuration.md#settings).

## Keys

| Key | What it does |
|---|---|
| PageUp, PageDown | Scroll the transcript a screen at a time |
| Mouse wheel | Scroll three lines |
| Ctrl+Home, Ctrl+End, Esc | Jump to the top, or back to the newest output |
| Tab, Shift+Tab | Move focus through the parts you can click: the command groups, the header rows and the status bar boxes. The focused part has a `›` marker |
| Enter or Space on a focused part | Does what a click does. Esc or typing a character gives focus back to the input line |
| Up, Down | Earlier prompts, when the cursor is on the first or last row of the input line. Otherwise Up and Down scroll one line |
| Left, Right, Home, End, Backspace, Delete | Edit the input line |
| Ctrl+A, Ctrl+E, Ctrl+K, Ctrl+W, Ctrl+U | Start of the line, end of the line, delete to the end, delete the last word, delete to the start |
| Alt+Left, Alt+Right | Move by a word |
| Enter | Send the line. A paste keeps its line breaks |
| Ctrl+D on an empty line | End the session |
| Ctrl+C | Stop what is running and ask for a comment. See [Ctrl+C](sessions.md#ctrlc) |

Every part that can be clicked can also be used from the keyboard. While the view is scrolled up,
new output does not move it, and a line under the transcript says how many rows are below.

## Selecting and copying text

With the mouse on, a plain drag selects text, a double click selects a word and a triple click
selects a row. The selection is drawn in reverse video and follows the text when the view scrolls.
A drag past the top or bottom edge scrolls. When the button is released Umbra copies the text to the
clipboard, with wrapped rows of one line joined, and the status bar says how many characters were
copied for two seconds. Any key or click clears the selection.

The terminal's own selection still works with its modifier key held: Shift in Windows Terminal and
most Linux terminals, Option in iTerm2, Fn in Terminal.app.

| To | Do this |
|---|---|
| Turn mouse reporting off anywhere | `mouse = false`, `--no-mouse` or `UMBRA_MOUSE=0`. The terminal then selects text itself, and everything is done with the keyboard |
| Turn the mouse on in a classic Windows console window | `UMBRA_MOUSE=1`. In conhost the mouse is off by default and quick edit stays on, so a drag uses the console's own selection |

Windows Terminal and other terminals that use a pseudoconsole get the mouse.

## Windows notes

- Umbra runs commands with Git for Windows' `bash.exe`. When no bash is found it falls back to
  `cmd /C`.
- Git Bash rewrites an argument that starts with `/` into a Windows path. To run a slash command
  from Git Bash, write `MSYS_NO_PATHCONV=1 umbra /review`. PowerShell and cmd do not do this.

## Where to go next

- [Sessions, Ctrl+C and images](sessions.md): stop a run, steer it, attach images, resume it.
- [Configuration](configuration.md): every setting this page names.

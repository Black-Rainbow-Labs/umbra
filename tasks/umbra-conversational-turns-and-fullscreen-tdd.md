# Umbra: conversational turns and a fullscreen terminal (task-2588)

## Introduction

Typing `Hello` into Umbra produced a long, confused session. The person expected what Claude Code
does: a short greeting back, and Umbra waiting for the next message. Instead the model was told to
"solve this issue", every reply without a command was rejected, and the run only ended when the
model gave up and ran `echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT`.

Measured on Umbra 0.4.1 with `anthropic/claude-haiku-5-5`, run in `C:/jason/dev/umbra` with the
task `Hello`: **23 model calls, $0.0135 and 46 seconds**, ending with "No fix made. The issue text
was only 'Hello'". The trajectory is
`ai-service/_agent_output/task-2588-umbra-dialog/before-hello.traj.json`. The Mac run in the ticket's
screenshot ran 15 steps and ended `RepeatedFormatError` at 88% of a 200k context.

Jason also asked for Umbra to open the way Claude Code's fullscreen mode does: the session starts on
a clean screen, scrolling up shows the conversation and not the shell's earlier output, and leaving
Umbra gives the shell back exactly as it was.

This document covers both changes.

## Part 1: why "Hello" goes wrong

Umbra is a port of mini-swe-agent, which was built to run SWE-bench style tasks with no person
watching. Four things in that design produce the screenshot:

1. **Every message is framed as an issue.** `instance_tools.jinja` and `instance_text.jinja` start
   with `Please solve this issue: {{task}}` followed by a six step bug fixing workflow (reproduce,
   edit, verify, test edge cases, submit). "Hello" becomes an issue to reproduce, so the model
   searches the repository for one.
2. **A reply without a command is an error.** `parse_tool_actions` returns `No tool calls found in
   the response. Every response MUST include at least one tool call.` The text mode parser demands
   exactly one block. A plain answer to the person can never be accepted.
3. **The only way to finish is a command.** A turn ends only when a command prints
   `COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT`. The prompt also says "After this command, you cannot
   continue working on this task", so a careful model refuses to run it until the "issue" is fixed.
4. **The rejected reply is thrown away.** As in mini-swe-agent, `format_error` keeps only the error
   message. The model never sees its own previous answer, so it writes the same answer again,
   which is rejected again. Three rejections in a row end the run with `RepeatedFormatError`. In
   between, the model runs `echo "Waiting for the user"` style commands to get past the check, which
   reset the counter, so the loop can run to the step or cost limit.

## Part 1: the design

### A turn ends when the model replies without a command

This is the Claude Code loop. The model either asks for tools, or it answers. An answer ends the
turn: Umbra shows it and waits for the person's next message, which continues the same
conversation.

A new setting picks the behavior:

```toml
# ~/.umbra/config.toml
finish = "reply"    # the default: a reply with no command ends the turn
# finish = "submit" # mini-swe-agent: every reply needs a command, the submit command ends the run
```

and the same on the command line: `--finish reply|submit`. `finish = "submit"` keeps today's
behavior exactly, for benchmark runs and for mini-swe-agent YAML configs. Importing a
mini-swe-agent YAML file with `-c` sets `finish = "submit"`, because its templates teach the submit
command.

With `finish = "reply"`:

| The model's reply | What happens |
|---|---|
| One or more valid tool calls (or one text mode block) | The commands run and the loop continues, as today. |
| Text and no tool call, `finish_reason` is not `length` | The reply is kept in the history and the turn ends with exit status `Replied`. |
| No text and no tool call | A format error: "Your reply was empty." |
| Cut off by the output limit (`finish_reason` = `length`) with no tool call | A format error asking the model to continue more briefly, as today. |
| A bad tool call (unknown tool, missing `command`) | A format error, as today. |
| Text mode with two or more blocks | A format error, as today. |
| A command that prints `COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT` | Still ends the turn with `Submitted`, so old habits and custom templates keep working. |

### The exit status `Replied`

`Replied` is a successful ending, the same as `Submitted`, with one difference: the answer is
already on screen and already sent to the editor, so it is not printed a second time.

- `Agent::run` asks for a follow up message after `Replied` as after `Submitted`.
- `umbra -t ... --exit-immediately` exits 0 after `Replied`. With `--quiet` it prints the reply,
  which makes `umbra -q -t "what does src/main.rs do"` work like `claude -p`.
- The turn summary prints `✔ Done · 1 step · $0.0003 · 2s` without repeating the answer.
- ACP: `session/prompt` returns `stopReason: end_turn`; the reply was already streamed as an
  `agent_message_chunk`, so `finish_turn` sends nothing more.

### Prompts that let the task decide the work

The `reply` style gets its own templates. The `submit` style keeps the current ones unchanged.

**System prompt** (`system.jinja`, the reply branch). It says what Umbra is, how to work, and when to
stop, in the spirit of Claude Code's prompt:

- Umbra is a coding agent running in the person's terminal or editor, working in their project.
- Match the work to the message. A greeting or small talk gets a short reply and no commands. A
  question that can be answered from what is already known is answered directly. A question about
  the code is answered after reading the code it is about. A request to change something is done
  fully: read the code, make the change, verify it, then report.
- Do not start work nobody asked for. When a request is unclear, ask one question.
- When the work is done, reply in plain text with what was done. That reply ends the turn.
- Every command runs in a fresh shell; working directory and variables do not persist.
- Do not commit, push, or delete files unless asked.
- The machine facts (OS, working directory, shell) move here from the first user message.
- Text mode only: the `umbra_bash` and `umbra_mcp` block format, with "a reply with no block ends
  your turn".
- The instructions, skills and MCP sections are kept as they are.

**First user message** (`instance_reply.jinja`): only the person's words, `{{task}}`. Nothing is
added in front of a greeting.

**Follow up messages**: the person's words as they typed them. The `extra` fields
(`interrupt_type: UserNewTask`, `user_text`) stay, because compaction and ACP history replay read
them.

**Format errors**: the reply style's text no longer says "every response needs a tool call" or
mentions the submit command. It says what was wrong and that a reply without a tool call ends the
turn.

### Code changes

| File | Change |
|---|---|
| `src/models/config.rs` | `Finish` enum (`Reply`, `Submit`), `UmbraConfig.finish`, default `Reply`. |
| `src/cli.rs`, `src/services/run_service.rs` | `--finish`; `Replied` exits 0. |
| `src/repositories/config_repository.rs` | A mini-swe-agent YAML import sets `finish = "submit"`. |
| `src/services/prompt_service.rs` | `Templates::resolve` takes the finish style; reply style instance template; `finish` passed to the format error template. |
| `src/templates/system.jinja` | Reply style branch. |
| `src/templates/instance_reply.jinja` | New: `{{task}}`. |
| `src/templates/format_error_tools.jinja`, `format_error_text.jinja` | Reply style branch. |
| `src/services/action_parser.rs` | Parsers return a `Parsed` value: `Actions(..)` or `Reply`; the reply style accepts a reply without a command. |
| `src/services/agent_service.rs` | `Step::Exit("Replied", text)` for a reply; follow up after `Replied`; follow up message is the person's words in reply style. |
| `src/services/interaction_service.rs` | `Replied` headline, output not repeated. |
| `src/services/acp/session.rs` | `Replied` sends nothing more. |
| `README.md`, `docs/configuration.md`, `docs/architecture.md` | Describe both styles. |

### Tests

- `tests/agent_loop.rs`: a replay run where the model answers "Hello" with text ends after one
  model call with `Replied`, exit code 0, and the trajectory's first user message is exactly
  `Hello`. A run with `--finish submit` still rejects the same reply.
- A follow up after a reply: the second user message is the person's words, and the model sees its
  previous answer.
- `tests/acp.rs`: a text reply ends the prompt with `end_turn`, and the answer arrives once.
- Live, with Haiku: `Hello` ends in 1 call. "What does src/main.rs do" reads the file and answers.
  A small change request (add a function, then run `cargo test` on a scratch crate) runs commands to
  the end and replies with what it did.

## Part 2: fullscreen, like Claude Code

### What happens today

Umbra writes its output inline, below the shell prompt that started it. Scrolling up runs from the
conversation into the shell's earlier output, and the start of a session is wherever the cursor
happened to be.

### The design

When Umbra runs interactively (stdin and stdout are terminals, not `--quiet`), it opens on the
terminal's **alternate screen**, the same buffer `vim`, `less` and Claude Code's fullscreen mode use.

- **Enter**: `ESC[?1049h` (alternate screen), `ESC[?1000h ESC[?1006h` (mouse wheel reports, SGR
  encoding), `ESC[?2004h` (bracketed paste). The input is put in a mode with no line buffering and
  no echo, with Ctrl+C still delivered as a signal (`ISIG` kept on Unix, `ENABLE_PROCESSED_INPUT`
  kept on Windows), so the existing Ctrl+C handling does not change.
- **Leave**: the reverse, in the opposite order, and the shell's screen comes back as it was. After
  leaving, Umbra prints the last turn's summary line and `resume with umbra --resume <id>` inline,
  so the shell keeps how to get back to the session. Leaving also runs from a panic hook and before
  the third Ctrl+C forces an exit, so a crash never leaves the terminal in the alternate screen.

**Layout**, top to bottom:

1. The transcript: everything Umbra prints today (banner, replies, commands, outputs, notices),
   kept in memory and wrapped to the terminal width when drawn.
2. The status bar (when on), with `↑ N lines below` while scrolled up.
3. The input line: the prompt (`❯`, or a confirmation question) and what the person is typing.
   It is always there, also while the agent works, so a message can be typed at any time and is
   queued as today.

**Keys**: printable text and paste go into the input line; Enter sends it; Backspace, Left, Right,
Home, End edit it; PageUp and PageDown scroll by a screen; the mouse wheel scrolls three lines;
Ctrl+End or Esc returns to the bottom; Ctrl+D on an empty line is end of input. While scrolled up,
new output does not move the view.

**Drawing**: every change redraws the whole screen inside synchronized output
(`ESC[?2026h` ... `ESC[?2026l`), so terminals that support it show no flicker. The screen size is
read on every draw, so a resize is picked up at the next change. The transcript keeps the most
recent 20,000 lines.

**When fullscreen is off**: with `--no-fullscreen`, `fullscreen = false` in the config,
`UMBRA_FULLSCREEN=0`, `TERM=dumb`, `--quiet`, a pipe on stdin or stdout, and under ACP. The Tasks
board lane launches Umbra with `--no-fullscreen`, because the board reads the agent's terminal
screen to tell whether it is idle and keeps the terminal's own scrollback as the record.

### Code changes

| File | Change |
|---|---|
| `src/utils/screen.rs` | New: the fullscreen state (transcript, scroll offset, status, prompt, input), the frame renderer, ANSI aware wrapping, enter and leave. |
| `src/utils/keys.rs` | New: decodes input bytes into keys (text, Enter, Backspace, arrows, Home/End, PageUp/Down, wheel, paste, Ctrl+D). |
| `src/utils/terminal_mode.rs` | New: raw input mode on and off (termios on Unix, console modes on Windows), the terminal height. |
| `src/utils/console.rs` | `emit`, `update_status`, `show_prompt`, `prompt_line` and `finish_status` go to the screen while it is on; the stdin thread reads keys instead of lines. |
| `src/utils/interrupt.rs` | Leave fullscreen before a forced exit. |
| `src/services/run_service.rs`, `src/cli.rs`, `src/models/config.rs` | `fullscreen` setting and `--no-fullscreen`; enter before the banner, leave before the final report. |
| ai-service `backend/src/config/umbraProjects.config.ts` (or wherever the lane builds its argv) | Add `--no-fullscreen`. |

### Tests

- Unit: key decoding (split escape sequences, UTF-8, SGR wheel, paste), wrapping with color codes,
  frame layout at a given width and height (scrolled and at the bottom).
- Functional, in a real pseudo terminal through `node-pty` (the library the board's terminal daemon
  uses): start Umbra on the replay provider, check the alternate screen is entered, type `Hello`
  and Enter, check the reply is drawn, send PageUp and check the view moved, send Ctrl+D and check
  the alternate screen was left and the resume line was printed.

## Part 3: compaction after "Hello"

Jason asked why compaction messages appeared right after saying "Hello", when compaction should
wait for a threshold. It does wait: before each call it estimates the request and compacts past
`trigger_ratio` (80%) of the window. The threshold was being crossed by the part of the prompt
compaction cannot remove.

### Measurements (this PC, Haiku, folder `C:/jason/dev/umbra`, every MCP server on)

| | Input tokens for "Hello" | Cost |
|---|---|---|
| Before | 214,611 | $0.0268 |
| After (tools deferred) | 31,905 | $0.0040 |

MCP tool definitions were 124,715 tokens across 229 tools: chordical 49,120, unluminous 47,390,
inillucent 8,693, firefox 6,651, playwright 4,758, blender 3,934, ghidra 2,422, code-search 944,
node_repl 562, aiservice-web 243. The instruction files add about 21k (`~/.claude/CLAUDE.md` 40.7 KB,
the project's `CLAUDE.md` 35 KB and `AGENTS.md` 9.3 KB). On a 200k window the trigger is 160k, so the
first request was already past it. Each call then masked or summarized a history that held almost
nothing, which is the run of compaction messages in the screenshot (88%, "compacts before the next
call").

### Fix 1: defer MCP tools behind `tool_search`

Claude Code's approach. When the MCP tool definitions would take more than `mcp.defer_ratio` (0.1)
of the context window (`mcp.defer_tools = "auto"`), the request carries `bash` and `tool_search`
only, and the system prompt lists the MCP tools by name in a `<deferred_tools>` section.
`tool_search` takes `select:<name>,<name>` or keywords, returns each tool's description and argument
schema, and loads those tools: they are sent with every later request. A tool called without a
search is loaded when it runs. On `--resume`, the tools the session loaded are loaded again from its
history. `always` and `never` override `auto`. Text mode works the same way, through an `umbra_mcp`
block.

Live check with Haiku: "Which version of the inillucent database does its MCP server report?" ran
`tool_search select:mcp__inillucent__inillucent_version`, called the tool and answered in 3 steps.

### Fix 2: no summary when it cannot free enough

Past the trigger, after the mask pass, a summary now runs only when the history it would remove is
worth at least `min_reclaim` (10% of the window, at most 20k). Otherwise Umbra warns once that the
context is full, how much a summary could remove, and how big the fixed part is. The fixed size is
the character estimate scaled by the ratio of the provider's last reported size to the estimate for
the same messages, so it is in the provider's tokens (31.9k reported, against a 21.4k raw estimate).

Live checks with Haiku:
- A 60k window and a task that reads three large files: 9 calls, 2 summaries, at 48.1k and 49.1k
  (the 48k trigger), down to 23.6k and 28.3k.
- A 30k window where the fixed prompt alone is past the trigger: one warning, no summary call, and
  the turn finished normally.

The character estimate before the first call reads this prompt about 30% low (about 22k against 31.9k
reported). From the second call on the reported size is used, so this only delays the first
compaction decision by one call. The provider's overflow error still triggers a summary and a retry.

## Rollout

Release as Umbra 0.5.0 (`finish = "reply"` changes the default behavior), install on the PC with
`cargo install --path`, and say in the release notes how to get the old behavior back. The Mac
picks the release up through its own install.

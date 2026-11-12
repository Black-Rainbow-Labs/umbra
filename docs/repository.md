# Repository and building

This page lists what is in the repository, how to build Umbra, and how to run each set of tests.
It is for someone who wants to change Umbra. [Architecture](architecture.md) explains how the code
works.

## What is where

```text
src/
  main.rs, cli.rs        the command line
  models/                plain data: messages, actions, config, context, trajectory, the settings schema
  repositories/          reading and writing files: config, instructions, skills,
                         commands, MCP config, sessions, trajectories
  services/              the agent loop, model providers, environments, MCP client,
                         prompt templates, compaction, sessions, the setup wizard,
                         umbra config and umbra session, updates
  services/acp/          the ACP server: JSON-RPC, sessions, session updates
  templates/             the Jinja prompt templates
  ui/                    the fullscreen view: blocks, frame, markdown, syntax colors,
                         header, overlays, selection
  utils/                 paths, frontmatter, variable expansion, HTTP, terminal output,
                         the status bar, the banner, the clipboard
tests/                   integration tests that run the real binary
docs/                    the reference pages, indexed in docs/README.md
evals/                   the eval suite that compares Umbra with pi on graded tasks
tasks/                   the design documents
packaging/               the release script, the Homebrew formula and the ACP registry entry
install.sh, install.ps1  the install scripts
.github/workflows/       tests on every push, release builds on every tag
```

The code follows a layered layout. Each layer only calls the layers below it: the command line and
the fullscreen view call services, services call repositories, and both use models and utils.

## Building

Umbra needs Rust 1.85 or newer.

```bash
cargo build --release          # target/release/umbra
cargo install --path .         # puts umbra in ~/.cargo/bin
```

A copy in `~/.cargo/bin` comes before the installed release on most `PATH`s. Run
`umbra --version` after a `cargo install` to check which one runs.

## Tests

```bash
cargo test
```

The integration tests in `tests/` run the real binary with a fake home folder
(`UMBRA_HOME_OVERRIDE`) and the replay provider, so they never read your configuration or call a
model.

| File | What it checks |
|---|---|
| `tests/agent_loop.rs` | The commands the loop ran against scripted replies, how it ended, and the trajectory it wrote |
| `tests/discovery.rs` | A fake home folder and project in the layouts Claude Code, Codex, opencode and Cursor use, and the instruction files, skills, commands and MCP servers Umbra finds there |
| `tests/mcp.rs` | The handshake, tool listing and tool calls against a fake stdio MCP server, in both protocol versions |
| `tests/compaction.rs` | What the model was sent before and after each compaction, with a small context window |
| `tests/sessions.rs` | `--resume`, `--continue`, `--fork`, `umbra sessions`, and repairing a history that stopped in the middle of a step |
| `tests/images.rs` | `--image`, image paths typed in a task, scaling, refusing files that are not images, and what each action format sends |
| `tests/interrupt.rs` | Ctrl+C sent to the real binary in its own process group |
| `tests/fullscreen.rs` | The fullscreen view read from its frames: the header, replies, command groups, MCP start, the status bar, commands, pickers, selection and copy |
| `tests/status_bar.rs`, `tests/terminal_output.rs` | The status bar through a pipe, and the banner, replies and one line notes of the inline view |
| `tests/acp.rs` | `umbra acp` driven the way an editor drives it, with every stdout line checked as JSON-RPC |
| `tests/cli.rs` | `umbra config`, `umbra mcp enable` and `disable`, `umbra models` and `umbra session` |
| `tests/update.rs` | `umbra update` and the fullscreen update prompt against a fake release on 127.0.0.1. Unix only |
| `tests/documentation.rs` | Every page in `docs/` is in the index, every relative link works, and no page uses a phrase [the writing style](writing-style.md) bans |

Two more sets are left out of a plain `cargo test` and run with `--ignored`:

```bash
cargo test --test live_local -- --ignored --test-threads 1       # against the model on localhost:8080
cargo test --test console_windows -- --ignored --test-threads 1  # a real Ctrl+C in a console window
```

`UMBRA_LIVE_MODEL` picks the model for the live tests, and `UMBRA_LIVE_EVIDENCE=<folder>` keeps
their trajectories.

GitHub Actions runs `cargo build --locked` and `cargo test --locked` on Linux, macOS and Windows for
every push to `main` and every pull request.

## The eval suite

`evals/` gives Umbra and pi the same graded tasks and compares the results. `evals/README.md` says
how to run it, and [the eval status](../evals/STATUS.md) has the current numbers.

## Releasing

[Releasing](releasing.md) explains how a tag becomes a GitHub release, a Homebrew formula and an
entry in the ACP registry.

# Umbra documentation

This folder holds the reference pages for Umbra. Each page can be read on its own.

To learn Umbra step by step, with pictures and clips of the real program, read the documentation at
[unluminous.com/umbra/docs](https://unluminous.com/umbra/docs). The pages here are for looking
things up, and they describe the same release.

The tables below list the pages in reading order for someone who has used a coding agent such as
Claude Code or Codex. Start with what Umbra is, then the terms, then how to run it, then how the
loop works.

`cargo test --test documentation` fails when a page in `docs/` is missing from this index, a
relative link or an anchor does not exist, or a page uses a phrase [the writing style](writing-style.md)
bans.

## Start here

| | Page | What it covers |
|---|---|---|
| 1 | [Product overview](product-overview.md) | What Umbra is, who it is for, and how it compares with other coding agents |
| 2 | [Glossary](glossary.md) | Each term the pages use, explained in one or two sentences |
| 3 | [Getting started](getting-started.md) | Installing, choosing a model, a first task, and checking what Umbra found |
| 4 | [How it works](how-it-works.md) | The loop, the two ways a model asks for a command, how a turn ends, and the prompt cache |

## Using it

| | Page | What it covers |
|---|---|---|
| 5 | [The terminal view](terminal.md) | The header, the transcript, the commands that start with `/`, the pickers, the configuration modal, the status bar and every key |
| 6 | [Modes](modes.md) | Confirm, yolo and human mode, refusing a command, commands that run without asking, Docker and the limits |
| 7 | [Sessions, Ctrl+C and images](sessions.md) | Stopping and steering a run, attaching images, and saving, resuming and copying sessions |
| 8 | [Models](providers.md) | Claude, local servers and every other provider, credentials, prompt caching, streaming, retries and cost |
| 9 | [Where files are found](discovery.md) | Every place Umbra looks for instruction files, skills, slash commands and MCP servers, in order |
| 10 | [MCP](mcp.md) | Starting the servers, the MCP panel, the formats Umbra reads, transports, protocol versions and limits |
| 11 | [Editors and ACP](acp.md) | Running Umbra in Zed, a JetBrains IDE or Unluminous, and every message it sends the editor |
| 12 | [Configuration](configuration.md) | Every setting, a complete example, editing settings from the command line and in the terminal, and the templates |
| 13 | [Command line](command-line.md) | Every command and option, `umbra config`, `umbra session`, `umbra update`, the environment variables and the exit codes |

## Measurements

| | Page | What it covers |
|---|---|---|
| 14 | [Feature comparison](feature-comparison.md) | Umbra against mini-swe-agent, Claude Code, Codex CLI and opencode, feature by feature, and why some features are left out |
| 15 | [Eval status](../evals/STATUS.md) | Umbra against pi on 61 graded tasks: the metrics where Umbra is ahead and the ones where it is not yet |

## Working on it

| | Page | What it covers |
|---|---|---|
| 16 | [Architecture](architecture.md) | The layers, one run from start to finish, the fullscreen view, compaction, commands, Ctrl+C, sessions and the trajectory file |
| 17 | [Repository and building](repository.md) | What is where, building, and what each set of tests checks |
| 18 | [Releasing](releasing.md) | How a tag becomes a GitHub release, a Homebrew formula and an ACP registry entry |
| 19 | [Writing style](writing-style.md) | How to write and check a page in this folder or a chapter on unluminous.com |
| | [`AGENTS.md`](../AGENTS.md) | The starting page for an AI agent that changes this repository |

## Design documents

The design documents under [`tasks/`](../tasks) record why each larger part of Umbra was built the
way it was. They describe the plan at the time it was written. These pages describe what Umbra does
now.

| Document | What it designed |
|---|---|
| [`umbra-harness-tdd.md`](../tasks/umbra-harness-tdd.md) | The harness, the loop and the discovery of other tools' files |
| [`umbra-compaction-tdd.md`](../tasks/umbra-compaction-tdd.md) | The mask and summary passes |
| [`umbra-images-resume-interrupt-tdd.md`](../tasks/umbra-images-resume-interrupt-tdd.md) | Images, sessions and Ctrl+C |
| [`umbra-acp-distribution-tdd.md`](../tasks/umbra-acp-distribution-tdd.md) | ACP, the releases and the install scripts |
| [`umbra-conversational-turns-and-fullscreen-tdd.md`](../tasks/umbra-conversational-turns-and-fullscreen-tdd.md) | Reply turns and the first fullscreen view |
| [`umbra-improvements-v2-tdd.md`](../tasks/umbra-improvements-v2-tdd.md) | The structured terminal view |
| [`umbra-fixes-part-5-tdd.md`](../tasks/umbra-fixes-part-5-tdd.md) | The status bar under the input, syntax colors, `umbra config` and `umbra session`, and updates |
| [`umbra-hillclimb-tdd.md`](../tasks/umbra-hillclimb-tdd.md) | The experiments that improve Umbra against pi |
| [`umbra-issues-v34-tdd.md`](../tasks/umbra-issues-v34-tdd.md) | Writing only the changed rows, Mermaid diagrams, math, footnotes, and `umbra resume` with no model configured |

## Data files

| File | What it holds |
|---|---|
| [`style-rules.txt`](style-rules.txt) | The phrases a page may not use. `tests/documentation.rs` reads it |

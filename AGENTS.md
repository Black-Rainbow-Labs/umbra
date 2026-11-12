# AGENTS.md: working on Umbra

This file is for an AI agent that changes this repository. Umbra is a coding agent for the
terminal, written in Rust. [The product overview](docs/product-overview.md) says what it does, and
[the glossary](docs/glossary.md) explains the words the pages use.

## Build and test

```bash
cargo build
cargo test
cargo test --test documentation    # the documentation rules, in a few seconds
```

The integration tests in `tests/` run the real binary with a fake home folder and a scripted model,
so they never read your configuration or call a model. [Repository and building](docs/repository.md)
lists what each test file checks and how to run the ignored live tests.

## How the code is laid out

The code has four layers, and each layer only calls the layers below it: `src/ui` and `src/cli.rs`
call `src/services`, which call `src/repositories`, and all of them use `src/models` and
`src/utils`. [Architecture](docs/architecture.md) explains each layer.

- Every function has a doc comment that says what it does, with an `@param` line for each argument.
- Functions stay short. A long function is split into smaller ones with one job each.
- Parameters go on one line.
- Prefer free functions that take their state as arguments.

## Every change updates the documentation in the same commit

A change that alters what a user sees, such as a command, an option, a setting, a key, a message on
the screen or a default, updates every place that describes it **in the same commit**:

| Place | What to update |
|---|---|
| `docs/` | The reference page that describes the feature. A new page is added to [docs/README.md](docs/README.md) |
| `README.md` | Only when the change alters the install, the first task, or the short tour |
| unluminous.com/umbra/docs | The chapter in `sites/unluminous/src/data/umbraDocumentation.ts` in the `Black-Rainbow-Labs/black-rainbow-labs-sites` repository (`C:\jason\dev\black-rainbow-labs-sites` on the build machine). Deploy it with `npm run deploy -- unluminous` from the main checkout of that repository |

A release also updates `sites/unluminous/src/data/umbra-release.json` in that repository, so the
site offers the new version.

The rules for writing a page are in [Writing style](docs/writing-style.md). The short version:

- Short declarative sentences in American English, in the present tense, about the current release.
- Example output is copied from a real run of the current release, never typed from memory.
- No em dashes, no words joined by hyphens, no idioms, and none of the phrases in
  [`docs/style-rules.txt`](docs/style-rules.txt).
- No ticket numbers such as `task-1234` in `README.md`, `docs/` or `AGENTS.md`. Say what the change
  did. Commit messages and the design documents under `tasks/` may name tickets.

`cargo test --test documentation` checks the index, every relative link and anchor, and the banned
phrases. It cannot check that a sentence is still true. That is the job of the change that made it
false.

## Releasing

`pwsh packaging/release.ps1 -Part patch` from a clean checkout of `main`. [Releasing](docs/releasing.md)
explains each step.

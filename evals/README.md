# Umbra vs pi eval suite

This suite runs two terminal coding agents, Umbra and pi, on the same tasks against the same local
model, verifies each result with a hidden verifier, and prints a scorecard. Both agents talk to
the model through a measuring proxy, so every token and timing number comes from the model
server's own responses and both agents get the same sampling settings.

Requirements: Node 24 (no npm packages), Python 3.13 as `python`, git, Git Bash, `umbra` on
PATH (or `UMBRA_BIN` set to the build to measure), pi (`@earendil-works/pi-coding-agent`, set `PI_CLI_JS` if its `cli.js` is not at
`C:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js`), and the model
server on `http://127.0.0.1:8080`.

## Running

```
node evals/run.mjs run --agents umbra,pi --reps 1 [--tasks id,id] [--category x] [--run-id name]
node evals/run.mjs selftest [--tasks id,id] [--category x]
node evals/run.mjs report <run id> [--compare <other run id>] [--ref <pi reference run id>]
node evals/run.mjs proxycheck [--run <run id>]
```

- `run` runs cells one at a time: for each task, each repetition, Umbra and then pi. Before the
  batch and before every cell it sends one tiny request straight to the server and waits until
  it answers within 5 seconds, because the server has one slot. `run` also takes the lock file
  `~/.umbra-evals-runner.lock` (or `EVAL_LOCK_FILE`) and stops with an error while another live
  `run` holds it, so two runs can never share the model. Giving an existing `--run-id`
  resumes the run: cells that have a `result.json` are skipped, and a cell that has no
  `result.json` (it was cut off, or the runner hit an error and wrote `error.txt`) is run again
  from scratch. The runner records both agents' versions in `run.json` and checks them again
  before every cell. If either changed (for example someone reinstalled `umbra` while a run was
  going), it stops instead of mixing two builds in one run, and a resume of that run id is
  refused until `UMBRA_BIN` or `PI_CLI_JS` points at the original build again. Each Umbra
  `result.json` also has `agentVersion`, read from the trajectory.
- `selftest` checks every task, including the ones left out of default runs: the verifier must
  fail on the untouched workspace and pass with score 1 on the reference solution.
- `report` writes `scorecard.md` and `scorecard.json` into the run folder and prints the markdown.
  `--compare` adds the other run's Umbra and pi values beside each head to head row. A run that
  measured Umbra only is compared with pi's cells from the reference run, `pi-1.3.0-ref` unless
  `--ref` names another. pi does not change, so it is measured once, 3 reps, and every later run
  is Umbra only. Only tasks that both agents ran are scored.
- `run.json` records `umbraBuild`: the `UMBRA_BIN` path, the git commit of this checkout, and
  whether its Rust sources had uncommitted changes. The scorecard header prints it. Build the
  binary under test with `cargo build --release --manifest-path <worktree>/Cargo.toml` and pass
  it as `UMBRA_BIN`.
- `proxycheck` sends one streaming request through a proxy and checks that chunks arrive over
  time (no buffering), that usage and timings are recorded, and with `--run` that every chat
  request in that run has usage, per agent.

Run output goes to `_agent_output/task-2614/eval-runs/<run id>/` (gitignored; `EVAL_RUNS_DIR`
overrides it):

```
run.json                                   settings, task list, agent versions
cells/<task>/<agent>/<rep>/
  result.json                              verdict, score, detail, phases, limits, exits
  requests.jsonl                           one line per request through the proxy
  stdout.log, stderr.log                   agent output (pi: the JSON event stream)
  trajectory.json                          Umbra's trajectory (Umbra cells)
  phase-<n>/                               the same files per phase, for session tasks
  home/                                    the isolated agent home (pi sessions are in home/sessions)
  workspace/                               the workspace after the run, without large fixtures
scorecard.md, scorecard.json
```

## How a cell runs

1. The workspace is built in a scratch folder `~/umbra-evals-work/w-<hash>/workspace`
   (`EVAL_WORK_DIR` overrides the root), outside the repository, so an agent that searches
   upward cannot find `verify.mjs`, `hidden/` or `solution/`. The folder name is a hash, so the
   path names neither the task nor the agent. It is deliberately not under the system temp
   folder: Git Bash shows `%TEMP%` as `/tmp`, the model then passes `/tmp/...` paths to pi's
   `read` tool, and pi resolves those to `C:\tmp\...` and fails with ENOENT (seen in an aborted
   first baseline, `eval-runs/aborted-baseline-tmp-workspaces`). pi does translate `/c/...`
   paths. The task's `workspace/` is copied in and its `generate.mjs`, if any, writes generated
   fixtures with a fixed seed.
2. A proxy starts on a free port. Each phase writes the agent's isolated home and launches it:
   - Umbra: `UMBRA_HOME_OVERRIDE=<cell>/home`, a `config.toml` there with a provider `evalproxy`
     whose `base_url` is the proxy (the built in `local` provider points at port 8080), and
     `umbra -m evalproxy/<model> --yolo --quiet --no-fullscreen --no-status-bar --exit-immediately
     --no-mcp --no-skills --no-instructions --step-limit <n> --cwd <workspace> -o <trajectory> -t <prompt>`
     with stdin closed.
   - pi: `PI_CODING_AGENT_DIR=<cell>/home` with `models.json` (provider `evalproxy`, the
     proxy's base URL, the task's context window) and `settings.json`, and
     `node <pi cli.js> -p --mode json --no-mcp --no-context-files --offline --no-extensions
     --no-skills --no-prompt-templates --no-themes --provider evalproxy --model <model> -- <prompt>`,
     with the workspace as the working folder of the spawned process (pi has no `--cwd`).
   - Resume: Umbra `--resume <session id>`, pi `--session <session id>`. Fork: Umbra
     `--resume <session id> --fork`, pi `--fork <session id>`. These were checked with the
     `smoke/session-flags` task: after a fork, resuming the original sends the original history
     without the fork's messages, for both agents.
3. The runner enforces the same limits for both agents. The wall limit (`timeoutSeconds`, per
   phase) and the step limit (`stepLimit`, default 60 chat requests per phase, counted at the
   proxy) both kill the agent's process tree by PID with `taskkill /T /F /PID`. Umbra also gets
   `--step-limit`, so it normally stops itself at the same count.
4. After the last phase the verifier runs on the scratch workspace. Then files listed in
   `largeFixtures` are deleted, the workspace is copied into the cell folder, the scratch folder
   is removed, and `result.json` is written.

## The measuring proxy

`src/services/proxy.mjs` forwards every path to `127.0.0.1:8080` and passes SSE responses
through chunk by chunk.

- On `POST /v1/chat/completions` it sets `temperature` 0.6, `top_p` 0.95, `top_k` 20 and
  `chat_template_kwargs.enable_thinking` true (`src/utils/config.mjs`), and sets
  `stream_options.include_usage` when the request streams. Messages, tools and `max_tokens`
  are not changed. What the agent itself asked for is kept in each record's `agentSampling`.
  All four fields were checked with direct requests on 2026-11-10: the server rejects a
  non numeric `temperature` or `top_k` with HTTP 400, so it parses them; temperature 1.8 gave
  varied answers where temperature 0, `top_k` 1 or `top_p` 0.01 gave the same answer every
  time; and `enable_thinking` false removed `reasoning_content`. No field was dropped.
- Each request is one line in `requests.jsonl`: start time, `firstByteMs`, `firstContentMs`
  (first content, reasoning or tool call delta), `durationMs`, `usage` (prompt_tokens,
  cached_tokens, completion_tokens, reasoning_tokens, from the final SSE chunk), the server's
  `timings`, `messageCount`, `messageHashes` (a short hash of each message, in order),
  `toolCount`, `systemBytes`, `toolsBytes`, `finishReason`,
  `hasToolCall`, `status`, `aborted` (the agent dropped the connection, for example when it was
  killed), `phase`, `contextWindow` and `windowExceeded`.
- On GET responses with JSON (`/props`, `/v1/models`), when the task sets `contextWindow`, every
  `n_ctx`, `max_model_len`, `context_length`, `context_window` and `n_ctx_train` field is
  rewritten to that value. Umbra reads its window from `/props` when the provider URL is on
  localhost, so this is how Umbra gets the small window. pi reads `contextWindow` from the
  isolated `models.json`, which the adapter writes.
- The proxy does not enforce the window. A request whose `prompt_tokens` is above the task's
  window is marked `windowExceeded`, and a cell on a task with a context window fails if any of
  its requests is marked.

## The Anthropic upstream (task-2643)

`EVAL_UPSTREAM=anthropic` runs both agents on `claude-haiku-5-5` (`EVAL_MODEL` overrides it)
instead of the local server. Every run before task-2643 used the local Qwen model, and the two
kinds of run must never be compared with each other.

- Both agents use their own Messages API code: Umbra a provider with `kind = "anthropic"`, pi a
  provider with `api: "anthropic-messages"`. Each points at the proxy with the placeholder key
  `eval-proxy`. The proxy forwards `POST /v1/messages` over HTTPS to `api.anthropic.com` and
  replaces the key with `ANTHROPIC_API_KEY`, or when that is unset, with the access token of
  Claude Code's saved login (`~/.claude/.credentials.json`, read on every request). With the
  login token it also puts Claude Code's identity line first in the system prompt, for both agents.
- Claude models accept only their default sampling, so the proxy removes `temperature`, `top_p`
  and `top_k`. It sets `thinking` to `{"type": "adaptive"}` and removes `output_config.effort`,
  so both agents think at the model's default effort. `EVAL_THINKING_OVERRIDE=0` leaves both
  fields to the agent. What the agent asked for is kept in `agentSampling`.
- Usage comes from the Messages API: `prompt_tokens` is input plus cache read plus cache write
  tokens, `cached_tokens` is the cache reads, and `cache_write_tokens` is recorded too. The API
  does not report thinking tokens apart from `output_tokens`, so `reasoning_tokens` is null.
  First content is the first content block the model starts, of any kind.
- A task's context window reaches Umbra as `compaction.context_window` in its config, and pi
  as `contextWindow` in `models.json`, as before. Without a task window both get 200,000, the
  window Umbra assumes for a Claude model.
- The server check sends one tiny request with the credential. There is no single slot to wait
  for, so only an error counts as busy.

## Metrics

`report` computes, per agent, overall and per category. Every token number is from
`requests.jsonl`.

| Metric | Meaning |
|---|---|
| Pass rate, mean score | Fraction of cells whose verifier passed; mean of the verifier's 0 to 1 score |
| Prompt tokens | Sum of `prompt_tokens` over all chat requests (cached tokens included) |
| Uncached prompt tokens | Prompt tokens minus cached tokens, the tokens the server actually processed |
| Completion, reasoning tokens | Sums of `completion_tokens` (reasoning included) and `reasoning_tokens` |
| Requests | Number of chat requests |
| Per passed task | The totals above divided by the number of passed cells |
| First request prompt tokens | Mean prompt size of each cell's first request: the system prompt, tool definitions and task, the fixed overhead |
| Cache hit rate | Cached tokens divided by prompt tokens |
| Wall seconds per task | Mean wall time of a cell, all phases |
| Mean time to first content | Mean `firstContentMs` over chat requests |
| Total model seconds | Sum of chat request durations |
| Runs with no tool call | Cells where no response contained a tool call |
| Runs that hit the step or time limit | Cells killed by the runner at a limit, or where Umbra stopped with `LimitsExceeded` |
| Requests with HTTP errors | Chat requests with status 400 or more, or a failed upstream connection |
| Malformed or unknown tool calls | Umbra: format error messages in the trajectory (`Error parsing tool call arguments`, `Missing 'command' argument`, `Unknown tool`). pi: tool results with `Tool ... not found` or `Validation failed for tool` |
| Runs with a nonzero exit | Cells where a phase exited nonzero or was killed, except a planned interrupt |
| Compaction pass rate, mean score | The same two numbers on the `compaction` category only |
| Requests over the context window | Requests marked `windowExceeded` |
| Compactions | Requests that changed an earlier message of the request before them in the same phase (from `messageHashes`), instead of only adding messages at the end. This counts a summary (messages removed) and Umbra's mask pass (old outputs rewritten in place) alike, and it is what stops the server's prompt cache from matching. Across a resume, and for records without hashes, a drop in the message count counts. A run of compactions in a row (a summary call, then the rebuilt history) counts once. The agents' own counts (Umbra's `info.compactions`, pi's `compaction_start` events) are shown beside it |
| Cache hit rate after a compaction | Cached over prompt tokens on the 5 requests that start at each compaction |
| Peak prompt tokens on window tasks | Largest `prompt_tokens` on any task with a context window |
| Session pass rate | Pass rate per session scenario (resume, interrupt, fork, long-resume, branch, cut-resume, long-resume-300) |
| Resume startup ms, long session | Time from launching the agent to its first model request when it resumes the 300 step session of `long-300-resume` |
| Session store bytes written per step | Bytes the agent process wrote while it built the 300 step session (Windows `WriteTransferCount`, read every half second), minus its own stdout and stderr, divided by the model requests of that phase |
| Runs that reached evals/tasks | Cells whose transcript (pi's event stream, Umbra's trajectory) names a path under `evals/tasks`. Such a cell fails, whatever the verifier said |
| Hard tier pass rate, mean score | The same two numbers on the `hard` category only |

The head to head table lists each metric with the Umbra value, the pi value, pi's noise band,
which is better and (Umbra - pi) / pi. Each value is the mean of that agent's per rep values, so
"total" rows are totals per rep and a 1 rep run and a 3 rep run are on the same scale. The noise
band is the largest minus the smallest of pi's per rep values. A difference no larger than the
band is a tie (`tie (noise)`). With more than one rep, a paired sign test over tasks compares the
pass rates, once over all tasks and once over the hard tier. The scorecard ends with the metrics
where Umbra is not strictly better than pi. A tie at the best possible value (0 errors, a 100%
rate) is not listed, because neither agent can improve on it. The hill climb is finished when
that list is empty for 3 reps.

## Known differences left as shipped

- Umbra's bash tool times out a command after 120 seconds by default, and the model can ask for up
  to 600 seconds on one command (task-2617, E9). Before that the limit was 30 seconds. pi's bash
  tool has no default timeout. Both are left at their defaults; the eval measures the harnesses as
  shipped.
- pi asks for `max_tokens` 16000 (from `maxTokens` in `models.json`, the value in the user's own
  pi config). Umbra sends no `max_tokens`. The proxy does not change either.
- pi's thinking level and Umbra's `thinking` provider setting do not matter here, because the proxy
  sets `enable_thinking` on every request. `EVAL_THINKING_OVERRIDE=0` turns that off for the
  thinking policy runs, which are reported apart from the main comparison.

## The tasks

| Category | Tasks | What they test |
|---|---|---|
| create (4) | ini-parser, interval-merger, log-summary-cli, semver-compare | A small program written from a precise specification; hidden tests cover the stated edge cases |
| bugfix (6) | config-loader, date-ranges, inventory-ledger, pricing-total, tokenizer, visible-tests-pass | Python and Node projects with one or two real bugs; in visible-tests-pass the visible tests already pass and only the hidden tests catch the bug the prompt describes |
| edit-precision (5) | near-duplicate-formatter, makefile-tab-recipe, crlf-retry-backoff, unicode-locale-entry, twelve-scattered-constants | One change, or twelve, inside files of 1,600 to 2,600 lines; every byte outside the change must be unchanged |
| navigate (4) | rename-function, config-default, route-handler, model-serializer | Generated repositories of 200 to 340 files with decoys; the change requires finding the right place |
| large-output (3) | buried-log-line, buried-test-failure, giant-json-line | A 40 MB log, a 20,000 line test run and a 9 MB one line JSON file, each with one fact to act on |
| instructions (3) | vendored-slugify, no-new-dependencies, orders-report | Constraints in the prompt (a file that must not change, no new dependencies, an exact output file and format), checked together with the result |
| compaction (4) | records-threshold, facts-report, glossary-codes, ledger-balance | `contextWindow` 32768; 30 to 40 files of about 2,000 tokens each processed one at a time, with a rule from the prompt and facts from the first files needed at the end; score is the fraction of items right |
| compaction, added for the hill climb (5) | notes-16k-roster, orders-16k-audit, inventory-64k-reconcile, contracts-64k-clauses, long-150-steps | The same design at a 16,384 and a 65,536 window, and a task of about 150 tool calls at 32,768, so facts from the start must survive several compactions |
| session (7) | resume-decisions, interrupt-continue, fork-branch, long-resume, branch-earlier, cut-resume, long-300-resume | Resume with a rule only the first prompt stated; a kill 20 seconds in and "continue"; a fork whose message the resumed original must not see; a resume of a history larger than 32768 tokens with a 32768 window; a branch from just before an earlier prompt, which must not see that prompt (pi branches through its RPC mode, `agents/piRpcBranch.mjs`, because its print mode cannot); a resume after the runner cut the last 400 bytes off the session file; a 300 step session, resumed, with the resume startup time and the session store bytes per step measured |
| hard (20) | see `evals/tasks/hard/` | Work sized so that pi passes about 40% to 70%: changes across 5 or more files, a bug whose cause is in a different module from its symptom, a refactor that must keep 40 tests passing, specifications with 8 or more numbered requirements each scored, tasks of 40 or more steps, and Windows traps (CRLF files, paths with spaces, tabs, BOMs and UTF-16, text mode writes) |

## Adding a task

Create `evals/tasks/<category>/<task id>/`:

```
task.json      { "id": "<folder name>", "prompt": "...", "timeoutSeconds": 600 }
               optional: contextWindow, stepLimit (default 60), largeFixtures, seed (default 2614),
               scenario, excludeFromDefault, phases
workspace/     starting files (may be absent)
generate.mjs   optional: export async function generate(workspaceDir, { seed })
solution/      reference solution files copied over the workspace; solution/solve.mjs, when present,
               is not copied but its export async function solve(workspaceDir, { seed }) is called
hidden/        optional: hidden tests and data, used only by verify.mjs
verify.mjs     export async function verify(workspaceDir, runInfo) -> { pass, score (0..1), detail }
```

`runInfo` has `task`, `agent`, `phases` (exit code, kill reason, session id, session validity,
and for phased tasks the list of workspace files after each phase), `requests` (the proxy
records) and `taskDir`. In selftest it also has `selftest: true` and synthetic phase results.

A phased task replaces `prompt` with `phases`: a list of `{ label, prompt, mode, from,
killAfterSeconds, contextWindow }`, where `mode` is `new`, `resume` or `fork`, `from` is the index
of the phase whose session is resumed or forked, `killAfterSeconds` makes the runner kill the
agent during that phase, `killWhenFiles: { dir, count }` kills it as soon as that workspace folder
holds `count` files (a trigger tied to progress, so a faster model is still interrupted partway),
and `contextWindow` overrides the task's window for that phase. Also:
`mode: "branch"` with `at` continues the session of phase `from` from just before the user
message that phase `at` sent (Umbra `--resume <id> --at <n> --fork`); `cutSessionBytes` cuts that
many bytes off the end of the session file before the phase resumes it; `measureStoreWrites` and
`measureResumeStartup` record the measurements described under Metrics; `stepLimit` overrides
the task's step limit for that phase.

Rules for tasks: solvable in under 5 minutes by a capable agent, no network or Docker, only Node,
Python (standard library), git and Git Bash tools. Verifiers check behavior (run the code, run
hidden tests), never trust test files in the workspace, and must not crash on missing files.
Large fixtures (over about 1 MB, or repositories of 150+ files) are generated with a fixed seed,
not committed. Run `node evals/run.mjs selftest --tasks <id>` and do not ship a task that fails it.

`smoke/hello` and `smoke/session-flags` are left out of default runs. They check the adapters and
the resume and fork flags.

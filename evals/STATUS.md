# Umbra against pi: where we stand

This page explains what the eval suite measures, how Umbra compares with pi today, and what is
left to do. The numbers are from 2026-11-18. The run by run record of every experiment is
[LEDGER.md](LEDGER.md), and how to run the suite is in [README.md](README.md).

## What the evals are

The suite gives two terminal coding agents the same 61 tasks and checks the result of each one
with a hidden verifier the agent never sees.

- **Umbra** is this repository's agent.
- **pi** (1.3.0, run exactly as shipped) is the agent we compare against.
- **The model** is the same for both: a local Qwen model, `ninfer-qwen3.8-27b-coletti-v9`, served
  on port 8080 with one request slot.
- **A measuring proxy** sits between each agent and the model. Every token count and every timing
  in this page comes from the model server's own responses, and the proxy gives both agents the
  same sampling settings (temperature 0.6, top_p 0.95, top_k 20, thinking on).
- **Each task runs 3 times** for each agent, one task at a time, because the model has one slot.
  A machine wide lock stops two eval runs from sharing the model.

### The task categories

| Category | Tasks | What it tests |
|---|---|---|
| hard | 20 | Long multi part tasks that pi passes about 70% of the time: specs with many requirements, migrations, bugs whose cause is far from the symptom |
| compaction | 9 | Work too long to fit in the context window, so the agent has to compact its history and keep going |
| session | 7 | Resume, fork, branch from an earlier message, resume after a crash, resume a 300 step session |
| bugfix | 6 | Find and fix a bug from a failing test |
| edit-precision | 5 | Edit files without touching bytes outside the change: CRLF endings, BOMs, tabs, non ASCII text |
| navigate | 4 | Find things in a code base |
| create | 4 | Write a small program from a description |
| large-output | 3 | Commands that print far more output than the context can hold |
| instructions | 3 | Follow precise output instructions |

### How a result is judged

- **Pass** means the verifier's checks all passed. **Score** is the share of checks passed.
- **pi's noise band** for a metric is the gap between pi's best and worst of its 3 reps. A
  difference smaller than that band is counted as a tie, because pi alone varies that much.
- An Umbra change is kept only if its target metric improves by more than the noise band and no
  other metric gets worse by more than its band. Otherwise it is reverted.
- Each row gets one of five verdicts: Umbra, tie with Umbra ahead, equal, tie with pi ahead, or
  pi. A row is "behind pi" when its verdict is pi or tie with pi ahead.
- **The goal** is two full 3 rep runs of the same build, and the two pooled, with no row behind
  pi. A tie counts as at pi's level.

## Where Umbra stands

These numbers are from two full 3 rep runs of all 61 tasks on 2026-11-17 and 2026-11-18, pooled as
6 Umbra reps against pi's 3 (runs `full3-96b35ad` and `full4-96b35ad`, task-2712). That build has
every change kept through hill climb 3: the hill climb 1 changes listed below and E28, the
workspace guard. The reply limit (E29), the output limit setting (E30), several commands in one
reply (E31) and the requirement check (E32) are settings that are off or at their old value by
default. pi is the reference run `pi-1.3.0-ref`.

### Head to head

| Metric | Umbra | pi | pi noise band | Better |
|---|---|---|---|---|
| Pass rate | 92.6% | 80.3% | 6.6 points | Umbra |
| Mean score | 98.4% | 90.7% | 5.4 points | Umbra |
| Hard tier pass rate | 81.7% | 66.7% | 10.0 points | Umbra |
| Hard tier mean score | 96.1% | 82.0% | 10.0 points | Umbra |
| Compaction pass rate | 94.4% | 44.4% | 22.2 points | Umbra |
| Compaction mean score | 99.4% | 77.7% | 16.9 points | Umbra |
| Prompt tokens, total | 19.6 million | 21.9 million | 0.7 million | Umbra, 11% fewer |
| Uncached prompt tokens, total | 696,642 | 3,863,170 | 133,361 | Umbra, 82% fewer |
| Completion tokens, total | 603,469 | 848,601 | 17,840 | Umbra, 29% fewer |
| Requests, total | 1,421 | 1,631 | 41 | Umbra, 13% fewer |
| Prompt tokens per passed task | 347,343 | 447,300 | 22,849 | Umbra, 22% fewer |
| Uncached prompt tokens per passed task | 12,342 | 78,944 | 7,646 | Umbra, 84% fewer |
| Completion tokens per passed task | 10,695 | 17,343 | 1,780 | Umbra, 38% fewer |
| Requests per passed task | 25 | 33 | 3 | Umbra, 24% fewer |
| First request size (fixed overhead) | 1,834 tokens | 2,154 tokens | 0 | Umbra, 15% smaller |
| Prompt cache hit rate | 96.4% | 82.4% | 1.1 points | Umbra |
| Wall time per task | 69.4 s | 117.5 s | 22.1 s | Umbra, 69% faster |
| Time to the first reply text | 340 ms | 1,095 ms | 27 ms | Umbra, 222% faster |
| Total model seconds | 3,778 | 6,092 | 130 | Umbra, 61% faster |
| Cache hit rate just after a compaction | 74.1% | 32.4% | 1.7 points | Umbra |
| Peak prompt tokens on window tasks | 45,252 | 48,212 | 1,294 | Umbra |
| Startup when resuming a long session | 63 ms | 498 ms | 248 ms | Umbra |
| Session store bytes per step | 981 | 1,094 | 29 | Umbra, 10% fewer |
| Runs that named a path outside the workspace | 0 of 366 | 4 of 183 | 1 | Umbra |
| Reasoning tokens per passed task | 6,111 | 6,440 | 452 | tie, Umbra ahead |
| Reasoning tokens, total | 344,698 | 315,434 | 25,289 | pi |

A paired sign test over the 61 tasks finds Umbra better on 15, pi better on 5 and 41 tied
(p = 0.041). Over the hard tier it is 8 better, 2 worse and 10 tied (p = 0.109). The rows not
shown (no tool call, HTTP errors, malformed tool calls, requests over the window, limit hits,
error exits, runs that reached the eval's own task files, and the seven session scenarios) are 0
or 100% for Umbra and equal or better than pi.

### Pass rate by category (Umbra / pi)

| Category | Umbra (6 reps) | pi (3 reps) |
|---|---|---|
| hard | 81.7% | 66.7% |
| compaction | 94.4% | 44.4% |
| session | 100% | 95.2% |
| bugfix, instructions, large-output, navigate | 100% | 100% |
| create | 95.8% (23 of 24) | 100% |
| edit-precision | 96.7% (29 of 30) | 100% |

### What is behind pi, and why

- **Reasoning tokens.** Four full runs of builds without a reply limit measured 307,198, 313,312,
  338,858 and 347,577 per rep, so one run lands on either side of pi's band of 25,289 by chance.
  A reply limit of 4,000 tokens (E29) brought two full runs to 264,777 and 270,635, but it made
  the model act on unfinished reasoning, and the hard tier pass rate fell from 81.6% to 74.6% over
  299 cells each, which made that row a tie. A higher limit saves too little. On this model the
  reasoning row and the hard tier row cannot both read Umbra. Per passed task Umbra uses fewer
  reasoning tokens than pi.
- **Create and edit-precision.** Each is one failed cell in 6 reps: semver-compare (one check of
  16) and makefile-tab-recipe. pi passed every cell of these categories in its 3 reference reps,
  so its band is 0 and one failed Umbra cell lists the row. In three more pi reps on these
  categories (run `pi-extra-zero-band`, kept out of the scorecard) pi failed one semver-compare
  cell as well. Over all of this ticket's full runs Umbra failed about 4% of the bugfix, create
  and edit-precision cells, and at that rate a pool of 6 reps with no failure is rare.

## Experiments

The run by run numbers are in [LEDGER.md](LEDGER.md).

| Experiment | Change | Result |
|---|---|---|
| E1 | Send the streamed reasoning back | Kept, released in 0.7.2 |
| E2 | Umbra records its own timings and cache counts | Kept, released in 0.7.2 |
| E3 | Ask the model to check its work before the final reply | Reverted (setting `finish_check`, off) |
| E4 | System prompt lines about line endings, encodings and Windows paths | Kept. edit-precision 15 of 15 |
| E5 | An `edit` tool beside `bash` | Not adopted (setting `edit_tool`, off) |
| E6 | Save very long command output to a file | Reverted (setting `output_spill`, off). Prompt tokens per passed task rose 32% |
| E7 | Stall message, tool call repair, rejected reply kept | Reverted. The error exits it targeted were replies cut off at the output limit, which E13 fixed |
| E8 | Compaction settings for a smaller working window | Kept, with no cap by default |
| E9 | A 120 second default command timeout | Kept |
| E10 | The shell state carries from one command to the next | Kept, on by default. Requests and completion tokens per passed task fell beyond pi's band |
| E11 | Append only session journal and `--at` to branch | Kept. branch and cut-resume 3 of 3 |
| E12 | A thinking setting for local models | Not adopted, setting unset. Thinking off: hard tier 40.0% against 81.7%, 8 times the requests per passed task. Thinking on the first request only: 37.1% |
| E13 | Ask a local server for 16,000 reply tokens | Kept. Hard tier 58.3% to 80.0%, error exits 3 to 0 |
| E14 to E16 | Session index at turn ends, server timings kept once, no second trajectory write | Kept. 6,601 to 1,922 bytes per step |
| E17 to E22 | The journal is the session store; compact journal records; stats as differences | Kept. 763 bytes per step in the final run |
| E23 | Keep at most 1 MB from each end of a command's output | Kept. One run had kept 420 MB |
| E24 | Stay inside the working directory | Reverted |
| E25 | Short thinking on routine steps | Kept |
| E26 | Copy the originals before a script edits many files in place | Kept |
| E27 | A firmer short thinking paragraph | Reverted |
| E28 | Refuse a command that names a path outside the working directory (`environment.workspace_guard`, `block` by default) | Kept. json-cleanup-30 went from 4 of 12 cells naming an outside path to 0 of 12, and 0 Umbra cells did in four full runs |
| E29 | A reply limit below `max_tokens`, asked again with thinking off after a cut (`reply_limit`) | Not on by default. Reasoning fell about 15% per rep, and the hard tier pass rate fell about 7 points |
| E30 | A larger observation limit (`environment.output_limit_chars`) | Not adopted, 10,000 by default. Requests per passed task did not fall |
| E31 | Ask for independent commands in one reply (`batch_commands`) | Not adopted, off. Requests per passed task rose |
| E32 | Check each stated requirement before the final reply (`requirement_check`) | Not adopted, off. Both forms lowered the pass rate on the four target tasks |

The eval changed again in task-2712: five verdicts with ties, a row per category, `report --pool`, an outside path row, the runner waits for free memory and commit charge before each cell and tries a cell again when the agent process did not start, and the outside path row reads an escaped string in a script as text.

The eval itself changed in task-2643: the store bytes counter reads the final write count after the
process exits (the old poll missed pi's last 259 KB write and reported pi at 190 bytes per step),
Umbra runs without `-o` so every file it writes counts, and long-300-resume counts tool calls as
steps.

The PC crashed once during the work (2026-11-14 04:13Z, bugcheck 0x20001, with corrected PCIe errors
on the root port above GPU 1 throughout). The eval was restarted after the reboot; finished cells
were kept.

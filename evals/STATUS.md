# Umbra against pi: where we stand

This page explains what the eval suite measures, how Umbra compares with pi today, and what is
left to do. The numbers are from 2026-11-12. The run by run record of every experiment is
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
- **The goal (G1)** is a full 3 rep run where Umbra is strictly better than pi on every metric in
  the scorecard.

## Where Umbra stands

The numbers below were measured with Umbra 0.6.2 plus E1 (the model's streamed reasoning is sent
back to the server, so the server's prompt cache stays valid). Release 0.7.2 is Umbra 0.7.1 plus
E1 and E2 (Umbra records its own timings and cached token counts). E2 does not change what the
agent does. Compared with pi over all 61 tasks, 3 reps each:

### Where Umbra is better

| Metric | Umbra | pi | Difference |
|---|---|---|---|
| Uncached prompt tokens per passed task | 14,608 | 78,944 | 81.5% fewer |
| Prompt tokens, total | 17.2 million | 21.9 million | 21.2% fewer |
| Completion tokens, total | 560,671 | 848,449 | 33.9% fewer |
| Requests, total | 1,364 | 1,631 | 16.4% fewer |
| First request size (fixed overhead) | 1,320 tokens | 2,154 tokens | 38.7% smaller |
| Prompt cache hit rate | 95.7% | 82.3% | 13.4 points higher |
| Wall time per task | 61.4 s | 116.9 s | 90% faster |
| Time to the first reply text | 319 ms | 1,084 ms | 240% faster |
| Compaction tasks passed | 96.3% | 44.4% | 51.9 points higher |
| Cache hit rate just after a compaction | 77.5% | 32.4% | 45.1 points higher |
| Startup when resuming a long session | 26.7 ms | 252 ms | 844% faster |

Before E1, Umbra 0.6.2 had a cache hit rate of 10.3%, needed 137,820 uncached prompt tokens per
passed task, and took 4,153 ms to the first reply text. E1 is the change that moved those three
numbers.

### Where Umbra is not yet better

| Metric | Umbra | pi | Why, and what targets it |
|---|---|---|---|
| Pass rate, all tasks | 82.5% | 80.3% | Umbra is ahead, but by less than pi's 6.6 point noise band |
| Mean score, all tasks | 89.7% | 90.7% | Inside the noise band |
| Hard tier pass rate | 60.0% | 66.7% | Inside the 10 point noise band. The largest gap left |
| Hard tier mean score | 76.5% | 82.0% | Inside the noise band |
| Reasoning tokens | 341,784 | 315,292 | pi uses 8.4% fewer |
| Runs that exited with an error | 6 | 1 | Most are replies where thinking used all 8,192 tokens the server allows by default. E13 targets this |
| Session: branch from an earlier message | 0 of 3 | 3 of 3 | Umbra 0.7.2 has no way to branch. E11 adds `--at` |
| Session: resume after a crash | 0 of 3 | 2 of 3 | E11 adds a session journal that survives a crash |
| Bytes written to the session store per step | 351,546 | 190 | Umbra rewrites the whole session file each step. E11 appends one line instead |

A sign test over the 61 tasks finds Umbra better on 10, pi better on 8 and 43 tied, so neither
agent is ahead on pass rate by a margin the test can detect.

### Pass rate by category (Umbra / pi)

| Category | Umbra | pi |
|---|---|---|
| compaction | 96.3% | 44.4% |
| hard | 60.0% | 66.7% |
| session | 71.4% | 95.2% |
| edit-precision | 93.3% | 100% |
| large-output, navigate, bugfix, create, instructions | 100% | 100% |

## What is left

The experiments come from the hill climb design in `tasks/umbra-hillclimb-tdd.md`. Each is built
and committed on the `task-2617` branch, and none of E4 to E13 is in a release yet, because none
has been measured.

| Experiment | Change | Status |
|---|---|---|
| E1 | Send the streamed reasoning back | Kept, released in 0.7.2 |
| E2 | Umbra records its own timings and cache counts | Kept, released in 0.7.2. Its numbers match the proxy exactly |
| E3 | Ask the model to check its work before the final reply | Reverted. The hard tier gain (60.0% to 63.3%) was inside the noise band, and it cost 30% more prompt tokens |
| E4 | System prompt lines about line endings, encodings and Windows paths | Partly run (41 of 60 hard tier cells) when the PC crashes stopped the eval |
| E5 | An `edit` tool beside `bash` | Built, off by default, not measured |
| E6 | Save very long command output to a file | Built, not measured |
| E7 | Notice a repeated command and repair broken tool calls | Built, not measured |
| E8 | Compaction settings for a smaller working window | Built, not measured |
| E9 | A 120 second default command timeout | Built, not measured |
| E10 | One shell kept for the whole turn | Built, off by default, not measured |
| E11 | Append only session journal and `--at` to branch | Built, not measured. Targets the three session rows |
| E12 | A thinking setting for local models | Built, unset by default, not measured |
| E13 | Ask a local server for 16,000 reply tokens instead of its 8,192 default | Built, not measured. Targets the error exits and part of the hard tier gap |

After those, the last steps are one full 3 rep run of all 61 tasks with every kept change, the
scorecard from that run, and a release.

### Why the work stopped

On 2026-11-12 the PC crashed three times within minutes of the eval putting load on the model.
The likely cause is the PCIe link to the second GPU, which the model server uses. Windows logged
about 12,000 corrected PCIe errors from that slot, the card runs at x4 instead of x16, and it
showed about 18,800 link retries. The eval resumes once that link is fixed.

# Umbra against pi: where we stand

This page explains what the eval suite measures, how Umbra compares with pi today, and what is
left to do. The numbers are from 2026-11-14. The run by run record of every experiment is
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

These numbers are from the full 3 rep run of all 61 tasks on 2026-11-13 and 2026-11-14 (run
`head-6d0f328`, task-2643). That build has every change kept up to then: E1, E2, E4, E8, E9, E10
(the persistent shell, now on by default), E11, E13, E14, E15 and E16. E3, E5, E6 and E7 are off.
The later session store changes (E17 to E22) and the output limit (E23) are measured separately
below. pi is the reference run `pi-1.3.0-ref`.

### Head to head

| Metric | Umbra | pi | pi noise band | Better |
|---|---|---|---|---|
| Pass rate | 91.8% | 80.3% | 6.6 points | Umbra |
| Mean score | 98.7% | 90.7% | 5.4 points | Umbra |
| Hard tier pass rate | 78.3% | 66.7% | 10.0 points | Umbra |
| Hard tier mean score | 96.4% | 82.0% | 10.0 points | Umbra |
| Compaction pass rate | 92.6% | 44.4% | 22.2 points | Umbra |
| Prompt tokens per passed task | 299,982 | 447,300 | 22,849 | Umbra, 33% fewer |
| Uncached prompt tokens per passed task | 13,456 | 78,944 | 7,646 | Umbra, 83% fewer |
| Completion tokens per passed task | 10,025 | 17,343 | 1,780 | Umbra, 42% fewer |
| Requests per passed task | 22 | 33 | 3 | Umbra, 33% fewer |
| First request size (fixed overhead) | 1,744 tokens | 2,154 tokens | 0 | Umbra, 19% smaller |
| Prompt cache hit rate | 95.4% | 82.4% | 1.1 points | Umbra |
| Wall time per task | 67.4 s | 117.5 s | 22.1 s | Umbra, 74% faster |
| Time to the first reply text | 382 ms | 1,095 ms | 27 ms | Umbra, 187% faster |
| Total model seconds | 3,589 | 6,092 | 130 | Umbra, 70% faster |
| Cache hit rate just after a compaction | 74.2% | 32.4% | 1.7 points | Umbra |
| Startup when resuming a long session | 60 ms | 498 ms | 248 ms | Umbra |
| Reasoning tokens, total | 307,198 | 315,434 | 25,289 | tie inside the band |
| Session store bytes per step | 1,085 | 1,094 | 29 | tie inside the band |
| Runs that hit the step or time limit | 0 | 1 | 1 | Umbra at 0, inside the band |
| Runs with a nonzero exit | 0 | 1 | 1 | Umbra at 0, inside the band |
| Session: resume after a crash | 3 of 3 | 2 of 3 | 100 points | Umbra at 100%, inside the band |
| Runs that reached the eval's own task files | 1 of 183 | 0 | 0 | pi |

A paired sign test over the 61 tasks finds Umbra better on 14, pi better on 3 and 44 tied
(p = 0.013).

### What is still open

- **Reasoning tokens.** Umbra uses 2.6% fewer, inside pi's band. E25 (a line asking for short
  thinking on routine steps) was being measured when the PC crashed.
- **Session store bytes per step.** The run above predates E17 to E22. With them, and with the eval
  counting every file Umbra writes (Umbra is no longer given `-o`), a live run measures 984 bytes
  per step against pi's 1,094.
- **The eval's own task files.** One hard tier run of json-cleanup-30 overwrote its only copy of the
  data with a buggy cleaning script, then searched the home folder, other projects and the main
  checkout's evals folder for the originals. E24 (stay inside the working directory) did not stop
  it and was reverted. E26 (keep a copy of the originals before a script edits many files in place)
  is built and not yet measured.
- **Rows where Umbra is at the best possible value** (0 limit hits, 0 error exits, 100% on resume
  after a crash) cannot be beaten by any margin, because pi's noise band is as wide as the gap. The
  scorecard lists them as not strictly better.

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
| E10 | One shell kept for the session | Kept, now on by default. Requests and completion tokens per passed task fell beyond pi's band |
| E11 | Append only session journal and `--at` to branch | Kept. branch and cut-resume 3 of 3 |
| E12 | A thinking setting for local models | Not yet measured |
| E13 | Ask a local server for 16,000 reply tokens | Kept. Hard tier 58.3% to 80.0%, error exits 3 to 0 |
| E14 to E16 | Session index at turn ends, server timings kept once, no second trajectory write | Kept. 6,601 to 1,922 bytes per step |
| E17 to E22 | The journal is the session store; compact journal records; stats as differences | Kept. 984 bytes per step live |
| E23 | Keep at most 1 MB from each end of a command's output | Kept. One run had kept 420 MB |
| E24 | Stay inside the working directory | Reverted |
| E25 | Short thinking on routine steps | Measurement stopped by the crash |
| E26 | Copy the originals before a script edits many files in place | Built, not measured |

The eval itself changed in task-2643: the store bytes counter now reads the final write count
after the process exits (the old poll missed pi's last 259 KB write and reported pi at 190 bytes
per step), Umbra runs without `-o` so every file it writes counts, and long-300-resume counts tool
calls as steps.

## What is left

1. Measure E25 and E26 on the hard tier, and the E12 thinking arms.
2. One full 3 rep run of all 61 tasks with every kept change, the scorecard from it, and a release.

### Why the work stopped on 2026-11-14

The PC crashed at 04:13Z with bugcheck 0x20001 (HYPERVISOR_ERROR), while the eval was running E25
on the hard tier. The PCIe root port above GPU 1, the card the model server uses, logged corrected
PCIe errors all through the run: 388 in the 75 minutes before the crash, and about 1,400 in the
three hours before it. The same bugcheck was one of the crashes on 2026-11-11 and 2026-11-12. The
dump is `C:\Windows\Minidump\111326-27250-01.dmp`. The eval resumes once that link is fixed.

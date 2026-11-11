# Scorecard: baseline-0.5.2

Cells: 66. Agents: pi, umbra. Model: ninfer-qwen3.8-27b-coletti-v9. Versions: umbra umbra 0.5.2, pi 1.3.0. Sampling: {"temperature":0.6,"top_p":0.95,"top_k":20,"enableThinking":true}.

All token numbers come from the measuring proxy (requests.jsonl). "Umbra vs pi" is (Umbra - pi) / pi.

## Head to head

| Metric | Umbra | pi | Better | Umbra vs pi |
|---|---|---|---|---|
| Pass rate | 93.9% | 93.9% | tie | +0.0% |
| Mean score | 96.5% | 99.1% | pi | -2.7% |
| Prompt tokens, total | 4,777,085 | 6,868,431 | umbra | -30.4% |
| Uncached prompt tokens, total | 4,296,725 | 2,181,786 | pi | +96.9% |
| Completion tokens, total | 98,455 | 288,269 | umbra | -65.8% |
| Reasoning tokens, total | 35,872 | 54,492 | umbra | -34.2% |
| Requests, total | 479 | 652 | umbra | -26.5% |
| Prompt tokens per passed task | 154,100 | 221,562 | umbra | -30.4% |
| Uncached prompt tokens per passed task | 138,604 | 70,380 | pi | +96.9% |
| Completion tokens per passed task | 3,176 | 9,299 | umbra | -65.8% |
| Requests per passed task | 15 | 21 | umbra | -26.5% |
| First request prompt tokens (fixed overhead) | 1,059 | 1,893 | umbra | -44.0% |
| Cache hit rate | 10.1% | 68.2% | pi | -85.3% |
| Wall seconds per task | 83.2 | 93.4 | umbra | -10.8% |
| Mean time to first content, ms | 4,050.4 | 1,425.6 | pi | +184.1% |
| Total model seconds | 2,495.7 | 2,370.3 | pi | +5.3% |
| Runs with no tool call | 0 | 0 | tie | n/a |
| Runs that hit the step or time limit | 0 | 1 | umbra | -100.0% |
| Requests with HTTP errors | 0 | 0 | tie | n/a |
| Malformed or unknown tool calls | 0 | 0 | tie | n/a |
| Runs with a nonzero exit | 0 | 1 | umbra | -100.0% |
| Compaction pass rate | 100.0% | 75.0% | umbra | +33.3% |
| Compaction mean score | 100.0% | 94.3% | umbra | +6.1% |
| Requests over the context window | 0 | 0 | tie | n/a |
| Cache hit rate, 5 requests after a compaction | n/a | 32.5% | n/a | n/a |
| Peak prompt tokens on window tasks | 43,794 | 44,464 | umbra | -1.5% |
| Session pass rate: fork | 100.0% | 100.0% | tie | +0.0% |
| Session pass rate: interrupt | 100.0% | 100.0% | tie | +0.0% |
| Session pass rate: long-resume | 100.0% | 100.0% | tie | +0.0% |
| Session pass rate: resume | 100.0% | 100.0% | tie | +0.0% |


## Metrics where Umbra is not strictly better than pi

- Pass rate
- Mean score
- Uncached prompt tokens, total
- Uncached prompt tokens per passed task
- Cache hit rate
- Mean time to first content, ms
- Total model seconds

## Per category

| Category | Agent | Cells | Pass rate | Mean score | Prompt tokens | Uncached prompt | Completion | Requests | Cache hit | Wall s per task |
|---|---|---|---|---|---|---|---|---|---|---|
| large-output | pi | 3 | 100.0% | 100.0% | 67,234 | 10,857 | 5,463 | 16 | 83.9% | 15.1 |
| large-output | umbra | 3 | 100.0% | 100.0% | 49,660 | 31,996 | 5,152 | 21 | 35.6% | 19.3 |
| navigate | pi | 4 | 100.0% | 100.0% | 153,692 | 21,453 | 8,033 | 29 | 86.0% | 15.5 |
| navigate | umbra | 4 | 100.0% | 100.0% | 137,268 | 107,323 | 8,944 | 35 | 21.8% | 27.6 |
| bugfix | pi | 6 | 100.0% | 100.0% | 208,149 | 22,336 | 15,357 | 49 | 89.3% | 117.9 |
| bugfix | umbra | 6 | 100.0% | 100.0% | 175,326 | 131,272 | 17,749 | 50 | 25.1% | 36.1 |
| edit-precision | pi | 5 | 100.0% | 100.0% | 290,983 | 67,689 | 5,036 | 24 | 76.7% | 12.5 |
| edit-precision | umbra | 5 | 80.0% | 80.0% | 121,100 | 94,612 | 6,314 | 32 | 21.9% | 17 |
| compaction | pi | 4 | 75.0% | 94.3% | 4,366,822 | 1,966,430 | 227,372 | 397 | 55.0% | 489.1 |
| compaction | umbra | 4 | 100.0% | 100.0% | 3,365,967 | 3,121,939 | 30,540 | 235 | 7.2% | 434.7 |
| session | pi | 4 | 100.0% | 100.0% | 1,652,750 | 74,376 | 8,688 | 101 | 95.5% | 26.2 |
| session | umbra | 4 | 100.0% | 100.0% | 813,257 | 737,365 | 7,628 | 65 | 9.3% | 89.4 |
| create | pi | 4 | 75.0% | 98.4% | 63,176 | 10,247 | 11,063 | 16 | 83.8% | 19.8 |
| create | umbra | 4 | 75.0% | 95.8% | 65,217 | 41,560 | 15,385 | 21 | 36.3% | 28.3 |
| instructions | pi | 3 | 100.0% | 100.0% | 65,625 | 8,398 | 7,257 | 20 | 87.2% | 21.1 |
| instructions | umbra | 3 | 100.0% | 100.0% | 49,290 | 30,658 | 6,743 | 20 | 37.8% | 22.4 |

## Compaction and sessions

| Agent | Compaction pass rate | Compaction mean score | Requests over window | Compactions (message count drops) | Compactions the agent reported | Cache hit after compaction | Peak prompt tokens on window tasks |
|---|---|---|---|---|---|---|---|
| pi | 75.0% | 94.3% | 0 | 105 | 105 | 32.5% | 44,464 |
| umbra | 100.0% | 100.0% | 0 | 0 | 6 | n/a | 43,794 |

| Agent | Session scenario | Passed | Cells |
|---|---|---|---|
| pi | fork | 1 | 1 |
| pi | interrupt | 1 | 1 |
| pi | long-resume | 1 | 1 |
| pi | resume | 1 | 1 |
| umbra | fork | 1 | 1 |
| umbra | interrupt | 1 | 1 |
| umbra | long-resume | 1 | 1 |
| umbra | resume | 1 | 1 |

## Failed cells

| Task | Agent | Rep | Score | Reason |
|---|---|---|---|---|
| ledger-balance | pi | 1 | 0.77 | 27/35 days right; day 6: original text changed; day 11: original text changed; day 12: original text changed; day 13: original text changed |
| log-summary-cli | umbra | 1 | 0.83 | 2 of 12 failed, failed: test_empty_file: - DEBUG: 0 \| test_only_malformed: skipped: 2 |
| makefile-tab-recipe | umbra | 1 | 0.00 | bytes before billing-package changed |
| semver-compare | pi | 1 | 0.94 | 1 of 16 failed, failed: parse prerelease identifier 0a and 0: Expected values to be strictly deep-equal: |

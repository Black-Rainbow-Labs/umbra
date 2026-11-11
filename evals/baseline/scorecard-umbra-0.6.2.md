# Scorecard: baseline-0.6.2

Cells: 66. Agents: pi, umbra. Model: ninfer-qwen3.8-27b-coletti-v9. Versions: umbra umbra 0.6.2, pi 1.3.0. Sampling: {"temperature":0.6,"top_p":0.95,"top_k":20,"enableThinking":true}.

All token numbers come from the measuring proxy (requests.jsonl). "Umbra vs pi" is (Umbra - pi) / pi.

## Head to head

| Metric | Umbra | pi | Better | Umbra vs pi |
|---|---|---|---|---|
| Pass rate | 97.0% | 90.9% | umbra | +6.7% |
| Mean score | 99.9% | 99.4% | umbra | +0.5% |
| Prompt tokens, total | 4,915,887 | 6,871,463 | umbra | -28.5% |
| Uncached prompt tokens, total | 4,410,239 | 2,209,675 | pi | +99.6% |
| Completion tokens, total | 105,002 | 294,506 | umbra | -64.3% |
| Reasoning tokens, total | 43,586 | 64,247 | umbra | -32.2% |
| Requests, total | 498 | 652 | umbra | -23.6% |
| Prompt tokens per passed task | 153,621 | 229,049 | umbra | -32.9% |
| Uncached prompt tokens per passed task | 137,820 | 73,656 | pi | +87.1% |
| Completion tokens per passed task | 3,281 | 9,817 | umbra | -66.6% |
| Requests per passed task | 16 | 22 | umbra | -28.4% |
| First request prompt tokens (fixed overhead) | 1,059 | 1,893 | umbra | -44.0% |
| Cache hit rate | 10.3% | 67.8% | pi | -84.8% |
| Wall seconds per task | 90.1 | 78.6 | pi | +14.5% |
| Mean time to first content, ms | 4,152.9 | 1,457.6 | pi | +184.9% |
| Total model seconds | 2,652.3 | 2,417.5 | pi | +9.7% |
| Runs with no tool call | 0 | 0 | tie | n/a |
| Runs that hit the step or time limit | 0 | 0 | tie | n/a |
| Requests with HTTP errors | 0 | 0 | tie | n/a |
| Malformed or unknown tool calls | 0 | 0 | tie | n/a |
| Runs with a nonzero exit | 0 | 0 | tie | n/a |
| Compaction pass rate | 75.0% | 75.0% | tie | +0.0% |
| Compaction mean score | 99.3% | 98.1% | umbra | +1.2% |
| Requests over the context window | 0 | 0 | tie | n/a |
| Cache hit rate, 5 requests after a compaction | n/a | 32.2% | n/a | n/a |
| Peak prompt tokens on window tasks | 42,601 | 44,432 | umbra | -4.1% |
| Session pass rate: fork | 100.0% | 100.0% | tie | +0.0% |
| Session pass rate: interrupt | 100.0% | 100.0% | tie | +0.0% |
| Session pass rate: long-resume | 100.0% | 100.0% | tie | +0.0% |
| Session pass rate: resume | 100.0% | 100.0% | tie | +0.0% |


## Metrics where Umbra is not strictly better than pi

- Uncached prompt tokens, total
- Uncached prompt tokens per passed task
- Cache hit rate
- Wall seconds per task
- Mean time to first content, ms
- Total model seconds
- Compaction pass rate

## Per category

| Category | Agent | Cells | Pass rate | Mean score | Prompt tokens | Uncached prompt | Completion | Requests | Cache hit | Wall s per task |
|---|---|---|---|---|---|---|---|---|---|---|
| large-output | pi | 3 | 100.0% | 100.0% | 48,235 | 8,502 | 2,761 | 17 | 82.4% | 9.1 |
| large-output | umbra | 3 | 100.0% | 100.0% | 34,170 | 21,445 | 4,056 | 16 | 37.2% | 12.4 |
| navigate | pi | 4 | 100.0% | 100.0% | 167,476 | 21,842 | 9,772 | 32 | 87.0% | 22.1 |
| navigate | umbra | 4 | 100.0% | 100.0% | 144,261 | 116,197 | 8,182 | 33 | 19.5% | 32.9 |
| bugfix | pi | 6 | 100.0% | 100.0% | 188,823 | 24,067 | 15,448 | 45 | 87.3% | 22.1 |
| bugfix | umbra | 6 | 100.0% | 100.0% | 203,790 | 157,629 | 19,548 | 52 | 22.7% | 36.6 |
| edit-precision | pi | 5 | 100.0% | 100.0% | 237,361 | 67,669 | 4,214 | 22 | 71.5% | 11.9 |
| edit-precision | umbra | 5 | 100.0% | 100.0% | 119,845 | 87,729 | 6,486 | 38 | 26.8% | 17 |
| compaction | pi | 4 | 75.0% | 98.1% | 4,427,562 | 1,993,703 | 225,438 | 402 | 55.0% | 494.6 |
| compaction | umbra | 4 | 75.0% | 99.3% | 3,476,926 | 3,210,858 | 35,579 | 256 | 7.7% | 489.1 |
| session | pi | 4 | 100.0% | 100.0% | 1,605,970 | 74,168 | 8,132 | 94 | 95.4% | 32.1 |
| session | umbra | 4 | 100.0% | 100.0% | 842,852 | 761,160 | 8,728 | 65 | 9.7% | 98.2 |
| create | pi | 4 | 75.0% | 98.4% | 61,636 | 9,930 | 13,905 | 14 | 83.9% | 20.6 |
| create | umbra | 4 | 100.0% | 100.0% | 46,054 | 26,298 | 14,020 | 18 | 42.9% | 21.8 |
| instructions | pi | 3 | 66.7% | 98.1% | 134,400 | 9,794 | 14,836 | 26 | 92.7% | 32.7 |
| instructions | umbra | 3 | 100.0% | 100.0% | 47,989 | 28,923 | 8,403 | 20 | 39.7% | 20.8 |

## Compaction and sessions

| Agent | Compaction pass rate | Compaction mean score | Requests over window | Compactions (message count drops) | Compactions the agent reported | Cache hit after compaction | Peak prompt tokens on window tasks |
|---|---|---|---|---|---|---|---|
| pi | 75.0% | 98.1% | 0 | 106 | 106 | 32.2% | 44,432 |
| umbra | 75.0% | 99.3% | 0 | 0 | 6 | n/a | 42,601 |

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
| ledger-balance | umbra | 1 | 0.97 | 34/35 days right; day 5: got 735, expected 675 |
| no-new-dependencies | pi | 1 | 0.94 | 1 of 18 failed, failed: parseFlags(["--offset","-5"]): Expected values to be strictly deep-equal: |
| records-threshold | pi | 1 | 0.93 | 37/40 records right; rec-24: original text changed; rec-25: original text changed; rec-27: original text changed |
| semver-compare | pi | 1 | 0.94 | 1 of 16 failed, failed: spec ordering chain: 1.0.0-beta.2 vs 1.0.0-beta.11 |

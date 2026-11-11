# Prompt cache diagnosis for Umbra on the NInfer server (task-2614)

All numbers come from the scripts in this folder, run against http://127.0.0.1:8080/v1 one request at a time. Request bodies use the same tools, sampling (temperature 0.6, top_p 0.95, top_k 20) and `chat_template_kwargs.enable_thinking: true` as the eval proxy applies. B is always the request that follows A. "Coverage" is B cached_tokens divided by A prompt_tokens plus A completion_tokens, the most the server could have reused.

## Confirmed cause

Umbra does not send the assistant message back with its `reasoning_content`. With thinking enabled the model writes `<think>...</think>` before every reply and the server keeps the state it reached after generating that text. Umbra's `convert_message` sends only `content` and `tool_calls`, so the server renders the previous assistant turn without the thinking text, the tokens differ from what it generated, and the cache falls back to the last position that still matches. That position is the end of the first user message (1,092 to 1,093 tokens). Every later request reuses only the system prompt, the tools and the first user message.

Umbra receives the reasoning (`StreamAssembler` stores `reasoning_content` in `message["reasoning_content"]` of `raw`), but `parse_response` does not copy it into the `Message`, and `convert_message` has nowhere to read it from.

## Exact fix

Send the reasoning back on each assistant message, and send `preserve_thinking` so the template keeps reasoning from earlier user turns too (pi sends both):

```json
{
  "model": "ninfer-qwen3.8-27b-coletti-v9",
  "chat_template_kwargs": { "enable_thinking": true, "preserve_thinking": true },
  "messages": [
    { "role": "assistant", "content": null, "reasoning_content": "<exact text the server streamed as reasoning_content>",
      "tool_calls": [ { "id": "call_...", "type": "function", "function": { "name": "bash", "arguments": "{"command":"ls"}" } } ] }
  ]
}
```

In Umbra this means: keep `reasoning_content` from the response on the stored assistant `Message` (for example in `extra`), emit it in `convert_message` for `Role::Assistant`, and add `chat_template_kwargs.preserve_thinking: true` to the body. The text must be the unmodified text the server produced.

## Experiment 1: replay (A is a plain request, not generated)

Messages 0 to 9 of the Umbra trajectory, with the real tool outputs from `extra.raw_output`. A ends after tool result 2. B adds assistant 3 and tool result 3. C adds assistant 4 and tool result 4. Assistant messages are exactly what Umbra sends (no reasoning).

| Variant | prompt_tokens | cached_tokens | prompt ms |
|---|---|---|---|
| thinking on: A | 2770 | 0 | 1066 |
| thinking on: B | 4093 | 1093 | 1027 |
| thinking on: C | 5410 | 1093 | 1470 |
| thinking off: A | 2746 | 0 | 910 |
| thinking off: B | 4069 | 2746 | 413 |
| thinking off: C | 5386 | 4069 | 415 |

With thinking off, the same Umbra serialization caches fully, because the generation prompt then ends in an empty think block that the next request also contains. With thinking on, even a replay stops at 1,093. This is hypothesis 3, confirmed.

## Experiment 2: live (the server generates A, B re-sends A's reply in a variant form)

A is system plus user, thinking on, max_tokens 3000. B is A plus the assistant reply in the variant form plus a tool message, max_tokens 1.

| Variant | A prompt+completion | B prompt_tokens | B cached_tokens | coverage | B prompt ms |
|---|---|---|---|---|---|
| V0 omit reasoning (Umbra today) | 1314 | 1469 | 1093 | 83.2% | 193 |
| V1 reasoning_content | 1188 | 1484 | 1187 | 99.9% | 137 |
| V2 reasoning_content + preserve_thinking | 1202 | 1498 | 1201 | 99.9% | 177 |
| V3 omit reasoning + preserve_thinking | 1151 | 1426 | 1093 | 95.0% | 240 |
| V4 field named reasoning | 1201 | 1497 | 1200 | 99.9% | 142 |
| V5 reasoning inline in content as <think> | 1219 | 1515 | 1218 | 99.9% | 140 |
| V6 thinking off (A and B) | 1110 | 1408 | 1109 | 99.9% | 148 |

- V0 is what Umbra does today. V3 shows `preserve_thinking` alone does nothing when the reasoning is not sent.
- V1, V2, V4 and V5 all work: `reasoning_content`, `reasoning`, or a `<think>` block inside `content` are all rendered the same way. The server accepts either field name.
- V6 (thinking off) works but the model then does not think, which is not what the eval or Umbra's users want.
- The 1 token short of 100% is the last token of A, which the server always recomputes.

## Experiment 3: hypothesis 2 (content shape and argument serialization), reasoning_content sent back

Argument variants (A generated, B re-serialized):

| Variant | A prompt+completion | B cached_tokens | coverage | B prompt ms |
|---|---|---|---|---|
| H2d args reparsed and compact | 1184 | 1183 | 99.9% | 148 |
| H2e args with spaces (pretty printed) | 1217 | 1216 | 99.9% | 130 |
| H2f exact server reply untouched (control) | 1459 | 1458 | 99.9% | 131 |

Content shape, using only runs where the server returned `content: null` (a tool call with no text):

| Variant | A prompt+completion | B cached_tokens | coverage | B prompt ms |
|---|---|---|---|---|
| content null | 1208 | 1207 | 99.9% | 136 |
| content empty string | 1196 | 1195 | 99.9% | 144 |
| content omitted | 1224 | 1223 | 99.9% | 144 |

Neither matters. Compact, pretty printed or re-parsed arguments, and `null`, `""` or omitted content for an empty reply, all keep 99.9% coverage. Hypothesis 2 is rejected. (An earlier run replaced real non empty content with null or "". That drops text the model wrote and misses the cache, which is expected and not a serialization effect.)

## Experiment 4: pi

pi was run through a logging proxy (logproxy.mjs, runpi.mjs, output in pi-requests.jsonl). Differences from Umbra, taken from pi's actual request bodies:

| Field | pi | Umbra |
|---|---|---|
| assistant `reasoning_content` | present, the exact text streamed | absent |
| assistant `content` | `null` when the reply had only tool calls | the string, `""` when empty |
| assistant `tool_calls` | `id`, `type: "function"`, `function.name`, `function.arguments` as compact JSON text; several calls in one message | same shape |
| `chat_template_kwargs` | `{"enable_thinking": true, "preserve_thinking": true}` (compat `qwen-chat-template`) | none from Umbra (the eval proxy adds `enable_thinking`) |
| user `content` | array of `{type: "text", text}` parts | plain string |
| token limit | `max_completion_tokens` | `max_tokens` when configured |

The user content shape and the token limit field are not related to the cache problem. The reasoning field is.

## Experiment 5: six turn append only conversation

The model generates every assistant turn, requests stream with `include_usage` like Umbra, tool outputs are the real note files from the eval workspace, max_tokens 1500, thinking on. Each table is a separate conversation, so the generated text and token counts differ between tables.

Best fix, reasoning_content plus preserve_thinking:

| Request | messages | prompt_tokens | cached_tokens | tokens processed | prompt ms |
|---|---|---|---|---|---|
| 1 | 2 | 1098 | 1093 | 5 | 329 |
| 2 | 4 | 1406 | 1205 | 201 | 153 |
| 3 | 6 | 2750 | 1533 | 1217 | 428 |
| 4 | 8 | 4136 | 2909 | 1227 | 443 |
| 5 | 10 | 5489 | 4259 | 1230 | 438 |
| 6 | 12 | 6903 | 5715 | 1188 | 433 |

reasoning_content only:

| Request | messages | prompt_tokens | cached_tokens | tokens processed | prompt ms |
|---|---|---|---|---|---|
| 1 | 2 | 1098 | 1091 | 7 | 102 |
| 2 | 4 | 1419 | 1218 | 201 | 113 |
| 3 | 6 | 2795 | 1578 | 1217 | 381 |
| 4 | 8 | 3155 | 2954 | 201 | 107 |
| 5 | 10 | 3582 | 3381 | 201 | 119 |
| 6 | 12 | 4105 | 3904 | 201 | 104 |

Umbra today (no reasoning sent):

| Request | messages | prompt_tokens | cached_tokens | tokens processed | prompt ms |
|---|---|---|---|---|---|
| 1 | 2 | 1098 | 1093 | 5 | 55 |
| 2 | 4 | 1328 | 1093 | 235 | 174 |
| 3 | 6 | 2578 | 1093 | 1485 | 572 |
| 4 | 8 | 2812 | 1093 | 1719 | 681 |
| 5 | 10 | 3042 | 1093 | 1949 | 809 |
| 6 | 12 | 3278 | 1093 | 2185 | 945 |

With the fix, each request reuses everything up to the end of the previous request's generated reply. The tokens processed are the newest tool result (about 1,200 tokens for a note file, 201 for the short ones) and a few more. Cached tokens grow to 5,715 of 6,903. Without the fix, cached stays at 1,093 for all six requests and the prompt processing time rises from 174 ms to 945 ms as the conversation grows (with the fix it stays near 430 ms for the same size outputs).

## Things that did not behave as expected

- Thinking on with Umbra's serialization misses the cache even in a pure replay, before the question of generated versus re-serialized text arises.
- In the reasoning_content only run, requests 4 to 6 processed only 201 tokens because the model's tool commands produced short outputs there. That is the conversation, not a difference between the variants.
- `/props` returns no chat template, so how the server renders previous turns was inferred from cache behavior, not read.
- No change to Umbra source was made. Not tested: the case with several user turns (a later user message after earlier assistant turns). `preserve_thinking` is meant for that case and pi sends it, so it should be included with the fix.

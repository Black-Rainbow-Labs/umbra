//! Compact journal records for assistant replies and tool results (task-2643, E19). A message in
//! the journal used to be written whole, and an assistant reply held its text, its tool calls, its
//! finish reason and its usage twice: once on the message and once in the reply JSON Umbra builds
//! from the stream (`extra.response`), and the tool calls a third time as `extra.actions`. A tool
//! result held its output in `content` and again in `extra.raw_output`.
//!
//! A compact record leaves out the parts that can be built again from the rest and lists them by
//! code in `o`. Before a compact record is written, it is expanded again and compared with the
//! message; a part is only left out when the expanded message is equal to the original, so a
//! record never loses data. A reply in a form this code does not know (an Anthropic reply, a reply
//! that was not streamed, a tool call whose arguments are not in compact JSON) keeps those parts.
//!
//! Four fields of a llama.cpp style `timings` object are ratios of others (tokens per second and
//! milliseconds per token, for the prompt and for the reply). The server rounds the times it
//! prints, so no one formula gives back every value; a code such as `ps1` names the formula that
//! gives back that field exactly, and a field no formula matches is kept (task-2643, E21).
//!
//! A tool result's `content` is the observation template's short form of the output and return
//! code, so it can be built again from `raw_output` and `returncode` (`cw` with CRLF line ends,
//! `cl` with LF, which depends on how the binary was built); its `tool_call_id` is the id of the
//! next unanswered call of the assistant message before it (`id`); an empty `exception_info`
//! (`ei`) and an assistant cost of 0.0 (`c0`) are left out (task-2643, E22).
//!
//! `model`, `object` and `streamed` of the reply JSON are the same from one reply to the next, so
//! a record may take them from the assistant message before it in the file (the carry).

use serde_json::{json, Map, Value};

/// The codes an assistant record may leave out, in the order they are tried.
const ASSISTANT_CODES: [&str; 23] = ["c0", "ct", "ro", "tc", "fr", "ix", "mo", "ob", "st", "ru", "us", "ac", "ps0", "ps1", "ps2", "ps4", "ps5", "pt0", "is0", "is1", "is2", "is3", "it0"];
/// The codes a tool record may leave out.
const TOOL_CODES: [&str; 6] = ["cw", "cl", "rw", "rc", "id", "ei"];

/// The reply fields that usually stay the same from one assistant message to the next.
#[derive(Debug, Clone, Default)]
pub struct Carry {
    model: Value,
    object: Value,
    streamed: Value,
    /// The tool call ids of the last assistant message.
    calls: Vec<Value>,
    /// How many tool results followed that message so far.
    results: usize,
}

impl Carry {
    /// Takes the carried fields from an assistant message that was just written or read.
    /// @param message - the message JSON
    pub fn update(&mut self, message: &Value) {
        if message["role"] == "tool" {
            self.results += 1;
        }
        if message["role"] != "assistant" {
            return;
        }
        self.calls = message["tool_calls"].as_array().map(|calls| calls.iter().map(|c| c["id"].clone()).collect()).unwrap_or_default();
        self.results = 0;
        let response = &message["extra"]["response"];
        self.model = response["model"].clone();
        self.object = response["object"].clone();
        self.streamed = response["streamed"].clone();
    }
}

/// Returns a compact record for a message, or None when nothing can be left out.
/// @param message - the message JSON
/// @param carry - the carried reply fields before this message
pub fn compact(message: &Value, carry: &Carry) -> Option<Value> {
    let (kind, codes): (&str, &[&str]) = match message["role"].as_str() {
        Some("assistant") => ("a", &ASSISTANT_CODES),
        Some("tool") => ("r", &TOOL_CODES),
        _ => return None,
    };
    let mut record = message.clone();
    record.as_object_mut()?.remove("role");
    let mut left_out: Vec<&str> = Vec::new();
    for code in codes {
        let mut trial = record.clone();
        if !remove_part(&mut trial, code) {
            continue;
        }
        let mut trial_codes = left_out.clone();
        trial_codes.push(code);
        if expand_parts(kind, &trial, &trial_codes, carry).as_ref() == Some(message) {
            record = trial;
            left_out = trial_codes;
        }
    }
    if left_out.is_empty() {
        return None;
    }
    Some(json!({ "t": kind, "m": record, "o": left_out }))
}

/// Expands a compact record (`t` is `a` or `r`) back into the message JSON.
/// @param record - the journal line
/// @param carry - the carried reply fields before this record
pub fn expand(record: &Value, carry: &Carry) -> Option<Value> {
    let codes: Vec<&str> = record["o"].as_array()?.iter().filter_map(Value::as_str).collect();
    expand_parts(record["t"].as_str()?, &record["m"], &codes, carry)
}

/// Builds the message from a record and the codes of the parts it leaves out.
/// @param kind - `a` for an assistant reply, `r` for a tool result
/// @param body - the record's message part
/// @param codes - the codes of the parts left out
/// @param carry - the carried reply fields
fn expand_parts(kind: &str, body: &Value, codes: &[&str], carry: &Carry) -> Option<Value> {
    let mut message = body.clone();
    match kind {
        "a" => {
            message["role"] = json!("assistant");
            for code in codes {
                restore_assistant_part(&mut message, code, carry)?;
            }
        }
        "r" => {
            message["role"] = json!("tool");
            for code in codes {
                restore_tool_part(&mut message, code, carry)?;
            }
        }
        _ => return None,
    }
    Some(message)
}

/// Puts one left out part of a tool result back.
/// @param message - the message JSON being rebuilt
/// @param code - the part's code
/// @param carry - the carry, for the call id
fn restore_tool_part(message: &mut Value, code: &str, carry: &Carry) -> Option<()> {
    match code {
        "cw" | "cl" => {
            let line_end = if code == "cw" { "\r\n" } else { "\n" };
            let content = short_observation(&message["extra"]["returncode"], message["extra"]["raw_output"].as_str()?, line_end)?;
            message.as_object_mut()?.insert("content".into(), json!(content));
        }
        "rw" | "rc" => {
            let shown: Value = serde_json::from_str(message["content"].as_str()?).ok()?;
            let (key, from) = if code == "rw" { ("raw_output", "output") } else { ("returncode", "returncode") };
            message["extra"].as_object_mut()?.insert(key.into(), shown.get(from)?.clone());
        }
        "id" => {
            let id = carry.calls.get(carry.results)?.clone();
            message.as_object_mut()?.insert("tool_call_id".into(), id);
        }
        "ei" => {
            message["extra"].as_object_mut()?.insert("exception_info".into(), json!(""));
        }
        _ => return None,
    }
    Some(())
}

/// The observation template's short form for an output under 10,000 characters with no
/// exception and no saved output file. minijinja's `tojson` escapes the HTML characters.
/// @param returncode - the return code JSON
/// @param output - the output text
/// @param line_end - the template's line end
fn short_observation(returncode: &Value, output: &str, line_end: &str) -> Option<String> {
    if !returncode.is_number() {
        return None;
    }
    let shown = serde_json::to_string(output).ok()?.replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026").replace('\'', "\\u0027");
    Some(format!("{{{line_end}  \"returncode\": {returncode},{line_end}  \"output\": {shown}{line_end}}}"))
}

/// Puts one left out part of an assistant message back. The reply JSON parts come first in the
/// code order, so `us` and `ac` read a reply that is already whole.
/// @param message - the message JSON being rebuilt
/// @param code - the part's code
/// @param carry - the carried reply fields
fn restore_assistant_part(message: &mut Value, code: &str, carry: &Carry) -> Option<()> {
    match code {
        "c0" => {
            message["extra"].as_object_mut()?.insert("cost".into(), json!(0.0));
            return Some(());
        }
        "ru" => {
            let raw = raw_usage_from(&message["extra"]["usage"])?;
            message.pointer_mut("/extra/response")?.as_object_mut()?.insert("usage".into(), raw);
            return Some(());
        }
        "us" => {
            let usage = usage_from(&message["extra"]["response"]["usage"]);
            message["extra"].as_object_mut()?.insert("usage".into(), usage);
            return Some(());
        }
        "ac" => {
            let actions = bash_actions(&message["tool_calls"])?;
            message["extra"].as_object_mut()?.insert("actions".into(), actions);
            return Some(());
        }
        _ => {}
    }
    if let Some(field) = timing_field(code) {
        let timings = message.pointer_mut("/extra/response/timings")?;
        let value = timing_ratio(code, timings)?;
        timings.as_object_mut()?.insert(field.into(), json!(value));
        return Some(());
    }
    let content = message["content"].clone();
    let calls = message["tool_calls"].clone();
    let finish = message["extra"]["finish_reason"].clone();
    let response = message.get_mut("extra")?.get_mut("response")?;
    match code {
        "ct" => choice_message(response)?.insert("content".into(), content),
        "ro" => choice_message(response)?.insert("role".into(), json!("assistant")),
        "tc" => choice_message(response)?.insert("tool_calls".into(), wire_tool_calls(&calls)?),
        "fr" => first_choice(response)?.insert("finish_reason".into(), finish),
        "ix" => first_choice(response)?.insert("index".into(), json!(0)),
        "mo" => response.as_object_mut()?.insert("model".into(), carry.model.clone()),
        "ob" => response.as_object_mut()?.insert("object".into(), carry.object.clone()),
        "st" => response.as_object_mut()?.insert("streamed".into(), carry.streamed.clone()),
        _ => return None,
    };
    Some(())
}

/// Removes the part a code names from a record. Returns false when the part is not there.
/// @param record - the record's message part
/// @param code - the part's code
fn remove_part(record: &mut Value, code: &str) -> bool {
    match code {
        "cw" | "cl" | "id" => {
            let key = if code == "id" { "tool_call_id" } else { "content" };
            return record.as_object_mut().is_some_and(|map| map.remove(key).is_some());
        }
        "ei" => return record.pointer_mut("/extra").and_then(Value::as_object_mut).is_some_and(|map| map.get("exception_info") == Some(&json!("")) && map.remove("exception_info").is_some()),
        "c0" => return record.pointer_mut("/extra").and_then(Value::as_object_mut).is_some_and(|map| map.get("cost") == Some(&json!(0.0)) && map.remove("cost").is_some()),
        _ => {}
    }
    if let Some(field) = timing_field(code) {
        return record.pointer_mut("/extra/response/timings").and_then(Value::as_object_mut).is_some_and(|map| map.remove(field).is_some());
    }
    let (object, key) = match code {
        "ct" | "ro" | "tc" => (record.pointer_mut("/extra/response/choices/0/message"), code_key(code)),
        "fr" | "ix" => (record.pointer_mut("/extra/response/choices/0"), code_key(code)),
        "mo" | "ob" | "st" => (record.pointer_mut("/extra/response"), code_key(code)),
        "ru" => (record.pointer_mut("/extra/response"), code_key(code)),
        "us" | "ac" | "rw" | "rc" => (record.pointer_mut("/extra"), code_key(code)),
        _ => return false,
    };
    object.and_then(Value::as_object_mut).is_some_and(|map| map.remove(key).is_some())
}

/// The timings field a ratio code rebuilds, or None for any other code.
/// @param code - the code
fn timing_field(code: &str) -> Option<&'static str> {
    match code.get(..2)? {
        "ps" => Some("predicted_per_second"),
        "pt" => Some("predicted_per_token_ms"),
        "is" => Some("prompt_per_second"),
        "it" => Some("prompt_per_token_ms"),
        _ => None,
    }
}

/// Computes a timings ratio with the formula a code names. The reply rate counts the tokens after
/// the first, as the server does.
/// @param code - the code
/// @param timings - the timings object
fn timing_ratio(code: &str, timings: &Value) -> Option<f64> {
    let (ms, n) = if code.starts_with('p') { (timings["predicted_ms"].as_f64()?, timings["predicted_n"].as_f64()? - 1.0) } else { (timings["prompt_ms"].as_f64()?, timings["prompt_n"].as_f64()?) };
    Some(match code {
        "ps0" | "is0" => 1e3 / ms * n,
        "ps1" | "is1" => n * 1e3 / ms,
        "ps2" | "is2" => n / ms * 1e3,
        "ps4" => 1.0 / (ms / n) * 1e3,
        "ps5" | "is3" => 1e3 / (ms / n),
        "pt0" | "it0" => ms / n,
        _ => return None,
    })
}

/// The JSON key a code stands for.
/// @param code - the code
fn code_key(code: &str) -> &'static str {
    match code {
        "ct" => "content",
        "ro" => "role",
        "tc" => "tool_calls",
        "fr" => "finish_reason",
        "ix" => "index",
        "mo" => "model",
        "ob" => "object",
        "st" => "streamed",
        "us" | "ru" => "usage",
        "ac" => "actions",
        "rw" => "raw_output",
        _ => "returncode",
    }
}

/// The first choice of a reply JSON, as a map.
/// @param response - the reply JSON
fn first_choice(response: &mut Value) -> Option<&mut Map<String, Value>> {
    response.pointer_mut("/choices/0")?.as_object_mut()
}

/// The message of the first choice of a reply JSON, as a map.
/// @param response - the reply JSON
fn choice_message(response: &mut Value) -> Option<&mut Map<String, Value>> {
    response.pointer_mut("/choices/0/message")?.as_object_mut()
}

/// The tool calls in the form an OpenAI compatible server streams them, from the message's own
/// tool calls: arguments as compact JSON text.
/// @param calls - the message's `tool_calls`
fn wire_tool_calls(calls: &Value) -> Option<Value> {
    let calls = calls.as_array()?;
    let wire: Option<Vec<Value>> = calls
        .iter()
        .map(|call| {
            let arguments = match &call["arguments"] {
                Value::String(text) => text.clone(),
                other => serde_json::to_string(other).ok()?,
            };
            Some(json!({ "id": call["id"], "type": "function", "function": { "name": call["name"], "arguments": arguments } }))
        })
        .collect();
    wire.map(Value::Array)
}

/// The usage Umbra keeps in `extra.usage`, from the usage an OpenAI compatible server sends.
/// @param usage - the server's usage JSON
fn usage_from(usage: &Value) -> Value {
    let cached = usage["prompt_tokens_details"]["cached_tokens"].as_u64().unwrap_or(0);
    json!({
        "input_tokens": usage["prompt_tokens"].as_u64().unwrap_or(0).saturating_sub(cached),
        "output_tokens": usage["completion_tokens"].as_u64().unwrap_or(0),
        "cache_read_tokens": cached,
        "cache_write_tokens": 0,
        "reasoning_tokens": usage["completion_tokens_details"]["reasoning_tokens"].as_u64().unwrap_or(0),
    })
}

/// The usage an OpenAI compatible server sends, from the usage Umbra keeps. Only a server that
/// sends exactly these fields is matched; any other keeps its own usage.
/// @param usage - Umbra's `extra.usage`
fn raw_usage_from(usage: &Value) -> Option<Value> {
    let input = usage["input_tokens"].as_u64()?;
    let cached = usage["cache_read_tokens"].as_u64()?;
    let output = usage["output_tokens"].as_u64()?;
    let reasoning = usage["reasoning_tokens"].as_u64()?;
    Some(json!({
        "prompt_tokens": input + cached,
        "completion_tokens": output,
        "total_tokens": input + cached + output,
        "prompt_tokens_details": { "cached_tokens": cached },
        "completion_tokens_details": { "reasoning_tokens": reasoning },
    }))
}

/// The actions of a reply whose tool calls are all `bash` with a `command`, the way the tool call
/// parser builds them. None for any other reply, which then keeps its actions.
/// @param calls - the message's `tool_calls`
fn bash_actions(calls: &Value) -> Option<Value> {
    let actions: Option<Vec<Value>> = calls
        .as_array()?
        .iter()
        .map(|call| {
            if call["name"] != "bash" {
                return None;
            }
            let mut action = json!({ "kind": "bash", "command": call["arguments"]["command"].as_str()?, "tool_call_id": call["id"] });
            if let Some(timeout) = call["arguments"]["timeout"].as_u64() {
                action["timeout"] = json!(timeout);
            }
            Some(action)
        })
        .collect();
    actions.map(Value::Array)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An assistant message the way the agent stores a streamed reply with one bash call.
    fn streamed_reply() -> Value {
        json!({
            "role": "assistant", "content": "",
            "tool_calls": [{ "id": "call_1", "name": "bash", "arguments": { "command": "echo step 1 >> log.txt" } }],
            "extra": {
                "actions": [{ "kind": "bash", "command": "echo step 1 >> log.txt", "tool_call_id": "call_1" }],
                "cost": 0.0, "finish_reason": "tool_calls", "timestamp": 1.5,
                "timing": { "first_token_ms": 151, "total_ms": 394 },
                "usage": { "input_tokens": 34, "output_tokens": 44, "cache_read_tokens": 6144, "cache_write_tokens": 0, "reasoning_tokens": 10 },
                "response": {
                    "id": "chatcmpl-1", "model": "m", "object": "chat.completion", "streamed": true,
                    "choices": [{ "index": 0, "finish_reason": "tool_calls", "message": { "role": "assistant", "content": "", "reasoning_content": "Step 1.\n", "tool_calls": [{ "id": "call_1", "type": "function", "function": { "name": "bash", "arguments": "{\"command\":\"echo step 1 >> log.txt\"}" } }] } }],
                    "usage": { "prompt_tokens": 6178, "completion_tokens": 44, "total_tokens": 6222, "completion_tokens_details": { "reasoning_tokens": 10 }, "prompt_tokens_details": { "cached_tokens": 6144 } },
                    "timings": { "cache_n": 6144, "prompt_n": 34, "prompt_ms": 116.137635, "prompt_per_second": 292.7560906505458, "prompt_per_token_ms": 3.415812794117647, "predicted_n": 52, "predicted_ms": 229.984459, "predicted_per_second": 221.75411426386856, "predicted_per_token_ms": 4.509499196078432 }
                }
            }
        })
    }

    #[test]
    fn a_streamed_reply_leaves_out_every_copy_and_expands_to_the_same_message() {
        let message = streamed_reply();
        let mut carry = Carry::default();
        carry.update(&message);
        let record = compact(&message, &carry).expect("a streamed reply compacts");
        let codes: Vec<&str> = record["o"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert_eq!(codes, ["c0", "ct", "ro", "tc", "fr", "ix", "mo", "ob", "st", "ru", "ac", "is1", "it0"], "every copy is left out; the raw usage is rebuilt from extra.usage, so extra.usage stays; the prompt ratios match a formula, the reply ratios of this reply match none and are kept");
        assert!(record["m"]["extra"]["response"]["timings"].get("predicted_per_second").is_some());
        assert_eq!(expand(&record, &carry).unwrap(), message);
        assert!(record.to_string().len() < message.to_string().len() * 2 / 3, "the record is much shorter");
        assert_eq!(record["m"]["extra"]["response"]["choices"][0]["message"]["reasoning_content"], "Step 1.\n", "the reasoning text is kept");
    }

    #[test]
    fn a_part_that_does_not_rebuild_exactly_is_kept() {
        let mut message = streamed_reply();
        message["extra"]["response"]["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"] = json!("{ \"command\": \"echo step 1 >> log.txt\" }");
        let record = compact(&message, &Carry::default()).unwrap();
        let codes: Vec<&str> = record["o"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert!(!codes.contains(&"tc"), "spaced arguments are kept as sent");
        assert!(!codes.contains(&"mo"), "a carry that does not match keeps the model");
        assert_eq!(expand(&record, &Carry::default()).unwrap(), message);
    }

    #[test]
    fn a_tool_result_leaves_out_the_output_shown_in_its_content() {
        let message = json!({ "role": "tool", "tool_call_id": "call_1", "content": "{\r\n  \"returncode\": 0,\r\n  \"output\": \"hello\\n\"\r\n}", "extra": { "raw_output": "hello\n", "returncode": 0, "exception_info": "", "timestamp": 2.0 } });
        let mut carry = Carry::default();
        carry.update(&streamed_reply());
        let record = compact(&message, &carry).unwrap();
        assert_eq!(record["o"], json!(["cw", "id", "ei"]), "the content is rebuilt, the call id comes from the reply before it");
        assert!(record["m"].get("content").is_none());
        assert_eq!(expand(&record, &carry).unwrap(), message);
        let mut odd = message.clone();
        odd["content"] = json!("{\r\n  \"returncode\": 0,\r\n  \"output\": \"hello\\n\", \"exception_info\": \"x\"\r\n}");
        let record = compact(&odd, &Carry::default()).unwrap();
        assert_eq!(record["o"], json!(["rw", "rc", "ei"]), "a content that is not the short form is kept, and the output is taken from it");
        assert_eq!(expand(&record, &Carry::default()).unwrap(), odd);
        let cut = json!({ "role": "tool", "tool_call_id": "call_2", "content": "<output_head>...</output_head>", "extra": { "raw_output": "long", "returncode": 0 } });
        assert!(compact(&cut, &Carry::default()).is_none(), "content that is not the output keeps raw_output");
    }
}

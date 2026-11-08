//! Context compaction. Before each model call the agent estimates the size of the request. Past
//! the trigger, old command outputs are replaced by a placeholder first (the mask pass), because
//! that keeps every command and every reasoning step and costs no model call. When that is not
//! enough, the model writes a summary of the middle of the session and the history is rebuilt
//! around it (the summary pass). The design and the research behind it are in
//! `tasks/umbra-compaction-tdd.md`.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::prompt_service::render;
use super::providers::{Provider, ProviderError, ToolSpec};
use crate::models::compaction::{CompactionKind, CompactionReason, CompactionRecord};
use crate::models::config::CompactionConfig;
use crate::models::message::{Message, Role};
use crate::models::model_response::ModelResponse;
use crate::utils::tokens::{estimate_text_tokens, DEFAULT_CONTEXT_WINDOW};

/// The tokens counted for each message on top of its text: role markers and separators.
const MESSAGE_OVERHEAD_TOKENS: u64 = 4;

/// The key in `Message.extra` that holds an assistant's raw Anthropic content blocks.
const RAW_CONTENT_KEY: &str = super::providers::anthropic::RAW_CONTENT_KEY;

/// The compaction sizes for one window, in tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    pub window: u64,
    pub trigger: u64,
    pub target: u64,
    pub protect_recent: u64,
    pub min_reclaim: u64,
    pub keep_recent: u64,
}

/// Turns the config's ratios into token counts for a window.
/// @param config - the compaction settings
/// @param window - the model's context window in tokens
pub fn budget_for(config: &CompactionConfig, window: u64) -> Budget {
    let share = |ratio: f64| (window as f64 * ratio.clamp(0.0, 1.0)) as u64;
    Budget {
        window,
        trigger: share(config.trigger_ratio),
        target: share(config.target_ratio.min(config.trigger_ratio)),
        protect_recent: share(config.protect_recent_ratio).min(config.protect_recent_max),
        min_reclaim: share(config.min_reclaim_ratio).min(config.min_reclaim_max),
        keep_recent: share(config.keep_recent_ratio),
    }
}

/// Picks the window: the config's value, then what the provider reports, then the default.
/// @param config - the compaction settings
/// @param provider - the model provider, asked only when the config sets no window
pub fn resolve_window(config: &CompactionConfig, provider: &mut dyn Provider) -> u64 {
    if config.context_window > 0 {
        return config.context_window;
    }
    provider.context_window().filter(|w| *w > 0).unwrap_or(DEFAULT_CONTEXT_WINDOW)
}

/// Estimates the tokens one message adds to a request. For Anthropic the raw content blocks
/// are what is sent, thinking blocks included, so they are counted instead of the text.
/// @param message - one history entry
pub fn estimate_message_tokens(message: &Message) -> u64 {
    if message.role == Role::Exit {
        return 0;
    }
    let body = match message.extra.get(RAW_CONTENT_KEY) {
        Some(Value::Array(blocks)) if !blocks.is_empty() => estimate_text_tokens(&Value::Array(blocks.clone()).to_string()),
        _ => estimate_text_tokens(&message.content) + message.tool_calls.iter().map(|c| estimate_text_tokens(&c.name) + estimate_text_tokens(&c.arguments.to_string())).sum::<u64>(),
    };
    body + MESSAGE_OVERHEAD_TOKENS
}

/// Estimates the tokens in a list of messages.
/// @param messages - part or all of the history
pub fn estimate_history_tokens(messages: &[Message]) -> u64 {
    messages.iter().map(estimate_message_tokens).sum()
}

/// Estimates the tokens the tool definitions add to every request.
/// @param tools - the tools offered to the model
pub fn estimate_tools_tokens(tools: &[ToolSpec]) -> u64 {
    tools.iter().map(|t| estimate_text_tokens(&t.name) + estimate_text_tokens(&t.description) + estimate_text_tokens(&t.input_schema.to_string())).sum()
}

/// The size of the last request as the provider reported it, and how many messages it held.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReportedSize {
    pub tokens: u64,
    pub messages: usize,
}

/// Estimates the size of the next request. With a reported size, that figure plus an estimate
/// of the messages added since; without one, an estimate of everything.
/// @param messages - the whole history
/// @param tools_tokens - the estimate for the tool definitions
/// @param reported - the last reported size, None after a compaction or when none was reported
pub fn estimate_request_tokens(messages: &[Message], tools_tokens: u64, reported: Option<ReportedSize>) -> u64 {
    match reported {
        Some(size) if size.tokens > 0 && size.messages <= messages.len() => size.tokens + estimate_history_tokens(&messages[size.messages..]),
        _ => estimate_history_tokens(messages) + tools_tokens,
    }
}

/// Returns true for a message holding a command or tool output: a `tool` message, or in text
/// mode a `user` message that carries `raw_output`.
/// @param message - one history entry
pub fn is_output(message: &Message) -> bool {
    match message.role {
        Role::Tool => true,
        Role::User => message.extra.get("raw_output").is_some_and(|v| !v.is_null()),
        _ => false,
    }
}

/// Returns true when a message's content was already replaced by a placeholder.
/// @param message - one history entry
fn is_masked(message: &Message) -> bool {
    message.extra.get("masked").and_then(Value::as_bool).unwrap_or(false)
}

/// Finds the command that produced the output at `index`: the matching tool call, or in text
/// mode the action of the nearest assistant message before it.
/// @param messages - the whole history
/// @param index - the position of an output message
fn command_for(messages: &[Message], index: usize) -> String {
    let output = &messages[index];
    for message in messages[..index].iter().rev().filter(|m| m.role == Role::Assistant) {
        if let Some(id) = &output.tool_call_id {
            if let Some(call) = message.tool_calls.iter().find(|c| &c.id == id) {
                return call.arguments.to_string();
            }
            continue;
        }
        return message.extra.get("actions").map(|a| a.to_string()).unwrap_or_default();
    }
    String::new()
}

/// Chooses the outputs to mask, newest first. Outputs after the last assistant message are
/// never masked, because the model has not been shown them yet; a reply that asks for several
/// commands at once can produce more output than the whole protected window. The newest seen
/// outputs worth `protect_recent` tokens are kept, as are short outputs, outputs already
/// masked, and SKILL.md reads, which the model needs for the rest of the task. Returns nothing when the pass would free less than
/// `min_reclaim` tokens, because each pass costs one prompt cache miss.
/// @param messages - the whole history
/// @param budget - the sizes for this window
/// @param min_chars - outputs shorter than this are kept
pub fn plan_mask(messages: &[Message], budget: &Budget, min_chars: usize) -> (Vec<usize>, u64) {
    let mut seen = 0;
    let mut chosen = Vec::new();
    let mut reclaimed = 0;
    let last_reply = messages.iter().rposition(|m| m.role == Role::Assistant).unwrap_or(0);
    for (index, message) in messages[..last_reply].iter().enumerate().rev() {
        if !is_output(message) || is_masked(message) {
            continue;
        }
        let tokens = estimate_text_tokens(&message.content);
        seen += tokens;
        if seen <= budget.protect_recent || message.content.len() < min_chars || command_for(messages, index).contains("SKILL.md") {
            continue;
        }
        chosen.push(index);
        reclaimed += tokens.saturating_sub(estimate_text_tokens(&placeholder(message)));
    }
    if reclaimed < budget.min_reclaim.max(1) {
        return (Vec::new(), 0);
    }
    chosen.reverse();
    (chosen, reclaimed)
}

/// The text that replaces a masked output. It keeps the size and return code so the model
/// knows what it had and that it can run the command again.
/// @param message - the output message
fn placeholder(message: &Message) -> String {
    let chars = message.extra.get("raw_output").and_then(Value::as_str).map(|s| s.chars().count()).unwrap_or_else(|| message.content.chars().count());
    let code = message.extra.get("returncode").and_then(Value::as_i64).map(|c| format!(", return code {c}")).unwrap_or_default();
    format!("[Output removed to save context after you read it: {chars} characters{code}. Run the command again if you need it.]")
}

/// Replaces the content of the chosen outputs with the placeholder and marks them. The full
/// output stays in `extra.raw_output` for the trajectory.
/// @param messages - the whole history
/// @param indexes - the outputs to mask
pub fn apply_mask(messages: &mut [Message], indexes: &[usize]) {
    for &index in indexes {
        let text = placeholder(&messages[index]);
        let message = &mut messages[index];
        message.content = text;
        if !message.extra.is_object() {
            message.extra = json!({});
        }
        message.extra["masked"] = json!(true);
    }
}

/// Where a summary cuts the history: `messages[..head_end]` is kept, `messages[head_end..tail_start]`
/// is summarized, and `messages[tail_start..]` is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    pub head_end: usize,
    pub tail_start: usize,
}

/// Finds the cut. The head is everything up to and including the first user message (the system
/// prompt and the task). The tail is the newest turns worth at most `keep_recent` tokens, and it
/// always starts at an assistant message, so every tool result in it follows its tool call.
/// Returns None when there is nothing between the two to summarize.
/// @param messages - the whole history
/// @param keep_recent - the most tokens the tail may hold
pub fn find_cut(messages: &[Message], keep_recent: u64) -> Option<Cut> {
    let head_end = messages.iter().position(|m| m.role == Role::User)? + 1;
    let mut tail_start = messages.len();
    let mut tokens = 0;
    for index in (head_end..messages.len()).rev() {
        tokens += estimate_message_tokens(&messages[index]);
        if tokens > keep_recent {
            break;
        }
        if messages[index].role == Role::Assistant {
            tail_start = index;
        }
    }
    (tail_start > head_end).then_some(Cut { head_end, tail_start })
}

/// Returns true for a follow up task the person typed, which a summary must not replace.
/// @param message - one history entry
fn is_follow_up_task(message: &Message) -> bool {
    message.extra.get("interrupt_type").and_then(Value::as_str) == Some("UserNewTask")
}

/// Drops the oldest half of the part to summarize, starting the rest at an assistant message so
/// no tool result loses its call. Used when the summary request itself is too long.
/// @param middle - the messages to summarize
fn drop_oldest_half(middle: &[Message]) -> Vec<Message> {
    let start = (middle.len() / 2..middle.len()).find(|&i| middle[i].role == Role::Assistant).unwrap_or(middle.len());
    middle[start..].to_vec()
}

/// What a summary pass needs besides the history.
pub struct SummaryInputs<'a> {
    pub provider: &'a mut dyn Provider,
    pub tools: &'a [ToolSpec],
    /// The template variables, to which `focus` and `summary` are added.
    pub vars: &'a Value,
    pub summary_template: &'a str,
    pub resume_template: &'a str,
    /// What the person asked the summary to focus on, from `/compact <focus>`.
    pub focus: &'a str,
}

/// The outcome of a summary pass: the new history, the summary text, the removed messages and
/// the model reply, whose cost and tokens the caller adds to the run's totals.
pub struct SummaryOutcome {
    pub history: Vec<Message>,
    pub summary: String,
    pub removed: Vec<Message>,
    pub response: ModelResponse,
}

/// Asks the model for a summary of the middle of the history and builds the new history: the
/// head, the follow up tasks the person typed, the summary, then the tail.
/// @param messages - the whole history
/// @param keep_recent - the most tokens the kept tail may hold
/// @param inputs - the provider, tools and templates
pub fn summarize(messages: &[Message], keep_recent: u64, inputs: SummaryInputs) -> Result<SummaryOutcome> {
    let Some(cut) = find_cut(messages, keep_recent) else { bail!("nothing to summarize: the history is only the task and the newest turns") };
    let head = &messages[..cut.head_end];
    let middle = &messages[cut.head_end..cut.tail_start];
    let mut vars = inputs.vars.clone();
    vars["focus"] = json!(inputs.focus);
    let prompt = render(inputs.summary_template, &vars)?;
    let response = request_summary(inputs.provider, inputs.tools, head, middle, &prompt)?;
    let summary = response.content.trim().to_string();
    if summary.is_empty() {
        bail!("the model returned no summary text");
    }
    vars["summary"] = json!(summary);
    let resume = Message::user(render(inputs.resume_template, &vars)?).with_extra(json!({ "compaction_summary": true }));
    let mut history = head.to_vec();
    history.extend(middle.iter().filter(|m| is_follow_up_task(m)).cloned());
    history.push(resume);
    history.extend_from_slice(&messages[cut.tail_start..]);
    let removed = middle.iter().filter(|m| !is_follow_up_task(m)).cloned().collect();
    Ok(SummaryOutcome { history, summary, removed, response })
}

/// Sends the summary request: the head, the middle and the summary prompt, with the run's own
/// tools so Anthropic accepts the tool use blocks in the history and the cached prefix is reused.
/// When the request is too long, the oldest half of the middle is dropped and it is sent once more.
/// @param provider - the model provider
/// @param tools - the run's tools
/// @param head - the system prompt and the task
/// @param middle - the messages to summarize
/// @param prompt - the rendered summary request
fn request_summary(provider: &mut dyn Provider, tools: &[ToolSpec], head: &[Message], middle: &[Message], prompt: &str) -> Result<ModelResponse> {
    let build = |middle: &[Message]| {
        let mut request = head.to_vec();
        request.extend_from_slice(middle);
        request.push(Message::user(prompt));
        request
    };
    match provider.complete(&build(middle), tools) {
        Ok(response) => Ok(response),
        Err(ProviderError { context_overflow: true, .. }) => provider.complete(&build(&drop_oldest_half(middle)), tools).map_err(|e| anyhow::anyhow!("summary call failed: {}", e.message)),
        Err(error) => bail!("summary call failed: {}", error.message),
    }
}

/// Builds the trajectory record for a mask pass.
/// @param reason - why it ran
/// @param step - the model calls so far
/// @param sizes - the estimate before and after, and the window
/// @param indexes - the masked positions
pub fn mask_record(reason: CompactionReason, step: u32, sizes: (u64, u64, u64), indexes: Vec<usize>) -> CompactionRecord {
    CompactionRecord { kind: CompactionKind::Mask, reason, step, tokens_before: sizes.0, tokens_after: sizes.1, window: sizes.2, masked_message_indexes: indexes, removed_messages: Vec::new(), summary: String::new(), cost: 0.0 }
}

/// Builds the trajectory record for a summary pass.
/// @param reason - why it ran
/// @param step - the model calls so far
/// @param sizes - the estimate before and after, and the window
/// @param outcome - the summary pass's result
pub fn summary_record(reason: CompactionReason, step: u32, sizes: (u64, u64, u64), outcome: &SummaryOutcome) -> CompactionRecord {
    CompactionRecord { kind: CompactionKind::Summary, reason, step, tokens_before: sizes.0, tokens_after: sizes.1, window: sizes.2, masked_message_indexes: Vec::new(), removed_messages: outcome.removed.clone(), summary: outcome.summary.clone(), cost: outcome.response.cost }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::message::ToolCall;

    /// Builds an assistant message with one bash call per command.
    fn assistant(id: &str, commands: &[&str]) -> Message {
        let mut message = Message::new(Role::Assistant, "thinking");
        message.tool_calls = commands.iter().enumerate().map(|(i, c)| ToolCall { id: format!("{id}{i}"), name: "bash".into(), arguments: json!({ "command": c }) }).collect();
        message
    }

    /// Builds a tool output of `size` characters.
    fn output(id: &str, size: usize) -> Message {
        Message::tool(id, "x".repeat(size)).with_extra(json!({ "raw_output": "x".repeat(size), "returncode": 0 }))
    }

    /// A history of `turns` turns, each one command printing `size` characters.
    fn history(turns: usize, size: usize) -> Vec<Message> {
        let mut messages = vec![Message::system("sys"), Message::user("the task")];
        for turn in 0..turns {
            let id = format!("t{turn}_");
            messages.push(assistant(&id, &["cat big.txt"]));
            messages.push(output(&format!("{id}0"), size));
        }
        messages
    }

    fn test_budget(protect: u64, min_reclaim: u64) -> Budget {
        Budget { window: 10_000, trigger: 8_000, target: 6_000, protect_recent: protect, min_reclaim, keep_recent: 1_000 }
    }

    #[test]
    fn mask_keeps_the_newest_outputs_and_skips_small_passes() {
        let messages = history(6, 4_000);
        let (indexes, reclaimed) = plan_mask(&messages, &test_budget(2_000, 100), 400);
        assert_eq!(indexes, vec![3, 5, 7], "the unseen newest output and the two seen outputs before it (1,000 tokens each) are kept");
        assert!(reclaimed > 2_500);
        let (none, _) = plan_mask(&messages, &test_budget(2_000, 10_000), 400);
        assert!(none.is_empty(), "a pass that frees less than the minimum masks nothing");
    }

    #[test]
    fn mask_never_removes_outputs_the_model_has_not_seen() {
        let mut messages = history(2, 4_000);
        messages.push(assistant("b_", &["cat a", "cat b", "cat c"]));
        messages.extend(["b_0", "b_1", "b_2"].iter().map(|id| output(id, 8_000)));
        let (indexes, _) = plan_mask(&messages, &test_budget(100, 100), 400);
        assert_eq!(indexes, vec![3, 5], "the three outputs of the newest reply were never shown to the model");
    }

    #[test]
    fn mask_skips_short_outputs_masked_outputs_and_skill_reads() {
        let mut messages = history(4, 4_000);
        messages[3].content = "short".into();
        messages.push(assistant("s_", &["cat ~/.claude/skills/tdd/SKILL.md"]));
        messages.push(output("s_0", 4_000));
        messages.push(assistant("n_", &["ls"]));
        messages.push(output("n_0", 4_000));
        let (indexes, _) = plan_mask(&messages, &test_budget(1_000, 100), 400);
        assert_eq!(indexes, vec![5, 7, 9]);
        apply_mask(&mut messages, &indexes);
        assert!(messages[5].content.starts_with("[Output removed to save context after you read it: 4000 characters, return code 0."));
        assert_eq!(messages[5].extra["raw_output"].as_str().unwrap().len(), 4_000, "the full output stays in the trajectory");
        let (again, _) = plan_mask(&messages, &test_budget(1_000, 100), 400);
        assert!(again.is_empty(), "masked outputs are not masked twice");
    }

    #[test]
    fn the_cut_keeps_tool_results_with_their_calls() {
        let mut messages = history(3, 400);
        messages.push(assistant("m_", &["ls", "pwd"]));
        messages.push(output("m_0", 400));
        messages.push(output("m_1", 400));
        let cut = find_cut(&messages, 300).unwrap();
        assert_eq!(cut.head_end, 2);
        assert_eq!(cut.tail_start, 8, "the newest reply and both of its results are kept together");
        assert!(find_cut(&messages, 1_000_000).is_none(), "a tail that holds everything leaves nothing to summarize");
    }

    #[test]
    fn the_cut_works_in_text_mode() {
        let mut messages = vec![Message::system("sys"), Message::user("task")];
        for _ in 0..4 {
            messages.push(Message::new(Role::Assistant, "```umbra_bash\nls\n```"));
            messages.push(Message::user("y".repeat(800)).with_extra(json!({ "raw_output": "y".repeat(800), "returncode": 0 })));
        }
        let cut = find_cut(&messages, 300).unwrap();
        assert_eq!((cut.head_end, cut.tail_start), (2, 8));
        assert!(is_output(&messages[3]));
    }

    #[test]
    fn the_estimate_uses_reported_usage_when_there_is_some() {
        let messages = history(2, 400);
        let all = estimate_request_tokens(&messages, 50, None);
        assert_eq!(all, estimate_history_tokens(&messages) + 50);
        let reported = estimate_request_tokens(&messages, 50, Some(ReportedSize { tokens: 5_000, messages: 4 }));
        assert_eq!(reported, 5_000 + estimate_history_tokens(&messages[4..]));
    }

    #[test]
    fn budgets_scale_with_the_window_and_respect_the_caps() {
        let config = CompactionConfig::default();
        let small = budget_for(&config, 32_000);
        assert_eq!((small.trigger, small.protect_recent, small.min_reclaim), (25_600, 6_400, 3_200));
        let large = budget_for(&config, 1_000_000);
        assert_eq!((large.protect_recent, large.min_reclaim), (40_000, 20_000));
    }
}

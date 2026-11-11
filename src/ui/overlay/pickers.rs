//! The pickers the commands and the header open: the model, the mode, the skills and the saved
//! sessions. Each builds a `Picker` with its items and what choosing one does.

use super::picker::{PickItem, Picker};
use super::{Outcome, Overlay};
use crate::models::config::Mode;
use crate::models::session::SessionIndexEntry;
use crate::services::providers::{check_credentials, create_provider, resolve_model};
use crate::ui::live::{self, UiRequest};

/// Checks that a model can be used and asks the agent to switch to it from the next model call.
/// The header shows it at once. The config file is not changed.
/// @param spec - a model name from `[models]` or a `provider/model` string
pub fn switch_model(spec: &str) -> Result<(), String> {
    let config = live::config().ok_or_else(|| "the session has no config yet".to_string())?;
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("type a model as provider/model".into());
    }
    check_credentials(&config, spec).map_err(|e| format!("{e:#}"))?;
    create_provider(&config, spec).map_err(|e| format!("{e:#}"))?;
    live::request(UiRequest::SetModel(spec.to_string()));
    let shown = resolve_model(&config, spec);
    live::update_header(|header| header.model = shown);
    Ok(())
}

/// The model picker: the named models, then the `models` list, then the current model when it is
/// in neither, then `Other...` for a `provider/model` string.
pub fn model_picker() -> Box<dyn Overlay> {
    let config = live::config().unwrap_or_default();
    let current = live::header().map(|h| h.model).unwrap_or_default();
    let current_spec = resolve_model(&config, &current);
    let mut items: Vec<PickItem> = Vec::new();
    for (name, named) in &config.models.named {
        let label = if named.label.is_empty() { name.clone() } else { format!("{name} · {}", named.label) };
        items.push(PickItem { label, detail: named.spec(), current: named.spec() == current_spec, value: name.clone(), ..Default::default() });
    }
    for spec in &config.models.listed {
        if !items.iter().any(|i| i.detail == *spec) {
            items.push(PickItem { label: spec.clone(), current: *spec == current_spec, value: spec.clone(), ..Default::default() });
        }
    }
    if !items.iter().any(|i| i.current) && !current_spec.is_empty() {
        items.insert(0, PickItem { label: current_spec.clone(), current: true, value: current_spec.clone(), ..Default::default() });
    }
    items.push(PickItem { label: "Other...".into(), asks_text: Some("provider/model".into()), ..Default::default() });
    let title = format!("▾ Model: {current}");
    Box::new(Picker::new(&title, items, true, Box::new(|item, typed| {
        switch_model(typed.unwrap_or(&item.value))?;
        Ok(Outcome::Close)
    })))
}

/// Switches the mode: the agent applies it at its next step and the header shows it at once.
/// @param mode - the new mode
pub fn switch_mode(mode: Mode) {
    live::request(UiRequest::SetMode(mode));
    let name = mode_name(mode).to_string();
    live::update_header(|header| header.mode = name);
}

/// The word for a mode.
/// @param mode - the mode
pub fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Confirm => "confirm",
        Mode::Yolo => "yolo",
        Mode::Human => "human",
    }
}

/// Reads a mode word.
/// @param text - confirm, yolo or human
pub fn parse_mode(text: &str) -> Option<Mode> {
    match text.trim() {
        "confirm" => Some(Mode::Confirm),
        "yolo" => Some(Mode::Yolo),
        "human" => Some(Mode::Human),
        _ => None,
    }
}

/// The mode picker.
pub fn mode_picker() -> Box<dyn Overlay> {
    let current = live::header().map(|h| h.mode).unwrap_or_default();
    let items = [("confirm", "ask before each command"), ("yolo", "run commands without asking"), ("human", "you type the commands")].into_iter().map(|(name, detail)| PickItem { label: name.into(), detail: detail.into(), current: current == name, value: name.into(), ..Default::default() }).collect();
    Box::new(Picker::new(&format!("Mode: {current}"), items, true, Box::new(|item, _| {
        let mode = parse_mode(&item.value).ok_or_else(|| "unknown mode".to_string())?;
        switch_mode(mode);
        Ok(Outcome::Close)
    })))
}

/// The skills picker. Enter puts `/<skill> ` in the input, ready for what to do.
pub fn skills_picker() -> Box<dyn Overlay> {
    let items: Vec<PickItem> = live::commands().into_iter().filter(|c| c.kind == "skill").map(|c| PickItem { label: c.name.clone(), detail: c.description, value: c.name, ..Default::default() }).collect();
    let title = format!("Skills · {}", items.len());
    Box::new(Picker::new(&title, items, false, Box::new(|item, _| {
        let text = format!("/{} ", item.value);
        Ok(Outcome::CloseWith(vec![Box::new(move || crate::utils::screen::set_input(&text))]))
    })))
}

/// The session picker: two rows per session, the title with its age, model and call count, then
/// the summary. Choosing one sends `/resume <id>`.
/// @param sessions - the sessions, newest first
pub fn session_picker(sessions: Vec<SessionIndexEntry>) -> Box<dyn Overlay> {
    let items: Vec<PickItem> = sessions.iter().map(session_item).collect();
    let picker = Picker::new("Resume a session", items, false, Box::new(|item, _| Ok(Outcome::Line(format!("/resume {}", item.value))))).with_hint("↑↓ move · Enter resume · type to filter · Esc cancel");
    Box::new(picker)
}

/// The session picker `umbra resume` opens before a session starts: Esc ends the run.
/// @param sessions - the sessions, newest first
pub fn start_picker(sessions: Vec<SessionIndexEntry>) -> Box<dyn Overlay> {
    let items: Vec<PickItem> = sessions.iter().map(session_item).collect();
    let picker = Picker::new("Resume a session", items, false, Box::new(|item, _| Ok(Outcome::Line(format!("/resume {}", item.value))))).with_hint("↑↓ move · Enter resume · type to filter · Esc cancel").on_cancel("/quit");
    Box::new(picker)
}

/// One session as a picker item.
/// @param session - the index entry
pub fn session_item(session: &SessionIndexEntry) -> PickItem {
    let model = session.model.rsplit('/').next().unwrap_or("").to_string();
    let detail = format!("{} · {model} · {} {}", crate::services::session_service::time_ago(session.updated_at), session.api_calls, if session.api_calls == 1 { "call" } else { "calls" });
    let summary = (!session.summary.is_empty()).then(|| session.summary.clone()).or_else(|| Some(String::new()));
    PickItem { label: crate::services::session_title_service::display_title(session), detail, summary, value: session.id.clone(), ..Default::default() }
}

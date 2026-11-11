//! The configuration modal: sections on the left, the fields of the selected section on the
//! right, and a footer with the keys. Every setting in the settings schema is shown, with where
//! its value comes from. Ctrl+S writes the changed keys to the chosen file, keeping its comments,
//! and the running session takes the settings that apply at once. A provider has a Test button.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::{Outcome, Overlay, Placement};
use crate::models::config::UmbraConfig;
use crate::models::model_catalog::known_for_kind;
use crate::models::settings_schema::{in_section, spec_for, Applies, FieldKind, Section, SettingSpec};
use crate::services::settings_service::{current_value, display_value, parse_input, test_provider, value_origin, write_changes, Origin, TestStep};
use crate::ui::block::{cut_to, paint, LayoutCtx, Row, Target};
use crate::ui::live::{self, UiRequest};
use crate::utils::ansi;
use crate::utils::keys::Key;
use crate::utils::screen::visible_width;

/// The width of the sections pane.
const LEFT: usize = 17;

/// The target numbers of the modal's rows: sections, fields, and the scope switch.
const SECTION_ITEM: usize = 1000;
const FIELD_ITEM: usize = 2000;
const SCOPE_ITEM: usize = 3000;

/// Which file a save writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    User,
    Project,
}

/// What a row of the fields pane is.
#[derive(Debug, Clone)]
enum Field {
    /// A setting at a full path, such as `providers.local.base_url`.
    Setting { path: String, spec: &'static SettingSpec },
    /// A heading, such as a provider's name.
    Heading(String),
    /// A line to read, such as an MCP server and its file.
    Info(String),
    /// A button.
    Button { label: String, action: Button },
}

/// What a button does.
#[derive(Debug, Clone)]
enum Button {
    TestProvider(String),
    AddProvider,
    AddModel,
    RemoveModel(String),
}

/// A field being typed into.
struct Editor {
    /// The setting path, or `new provider` and `new model` for a name being typed.
    path: String,
    text: String,
    error: Option<String>,
}

/// A list of values to choose from, for a choice or a model.
struct Chooser {
    path: String,
    options: Vec<String>,
    selected: usize,
}

/// A provider test, filled in by its worker thread.
#[derive(Default)]
struct TestRun {
    provider: String,
    steps: Vec<TestStep>,
    running: bool,
}

/// The modal.
pub struct ConfigModal {
    config: UmbraConfig,
    user_file: PathBuf,
    project_file: PathBuf,
    scope: Scope,
    section: usize,
    /// True while the fields pane has the keyboard.
    in_fields: bool,
    field: usize,
    /// Changes not yet saved, by path. None removes the key.
    edits: BTreeMap<String, Option<Value>>,
    /// The paths saved in this modal whose settings apply to the next session.
    saved_later: Vec<String>,
    editor: Option<Editor>,
    chooser: Option<Chooser>,
    message: Option<(String, bool)>,
    /// True while Esc asks whether to save or discard.
    closing: bool,
    test: Arc<Mutex<TestRun>>,
}

impl ConfigModal {
    /// Opens the modal on the session's config.
    pub fn open() -> Box<dyn Overlay> {
        let config = live::config().unwrap_or_default();
        let cwd = live::header().map(|h| h.cwd).unwrap_or_default();
        let project_file = crate::utils::paths::project_root(&cwd).join(".umbra").join("config.toml");
        let user_file = crate::utils::paths::umbra_home().join("config.toml");
        Box::new(ConfigModal { config, user_file, project_file, scope: Scope::User, section: 0, in_fields: false, field: 0, edits: BTreeMap::new(), saved_later: Vec::new(), editor: None, chooser: None, message: None, closing: false, test: Arc::new(Mutex::new(TestRun::default())) })
    }

    /// The selected section.
    fn current_section(&self) -> Section {
        Section::ALL[self.section]
    }

    /// The file a save writes.
    fn target_file(&self) -> PathBuf {
        match self.scope {
            Scope::User => self.user_file.clone(),
            Scope::Project => self.project_file.clone(),
        }
    }

    /// A setting's value: the unsaved change, or the running config's value.
    /// @param path - the setting path
    fn value(&self, path: &str) -> Option<Value> {
        match self.edits.get(path) {
            Some(change) => change.clone(),
            None => current_value(&self.config, path),
        }
    }

    /// The names of a table in the config with the unsaved changes, such as the providers.
    /// @param table - `providers` or `models`
    fn names_in(&self, table: &str) -> Vec<String> {
        let mut names: Vec<String> = match table {
            "providers" => self.config.providers.keys().cloned().collect(),
            _ => self.config.models.named.keys().cloned().collect(),
        };
        for path in self.edits.keys() {
            if let Some(name) = path.strip_prefix(&format!("{table}.")).and_then(|rest| rest.split('.').next()) {
                if !names.contains(&name.to_string()) {
                    names.push(name.to_string());
                }
            }
        }
        names
    }

    /// The rows of the fields pane for the selected section.
    fn fields(&self) -> Vec<Field> {
        match self.current_section() {
            Section::Models => self.model_fields(),
            Section::Providers => self.provider_fields(),
            Section::Discovery => self.expanded_fields(Section::Discovery, "sources"),
            Section::Templates => self.expanded_fields(Section::Templates, "templates"),
            Section::Mcp => {
                let mut rows = plain_fields(Section::Mcp);
                rows.push(Field::Heading("Servers found".into()));
                for (name, origin) in live::mcp_origins() {
                    rows.push(Field::Info(format!("{name:<16} {origin}")));
                }
                rows
            }
            section => plain_fields(section),
        }
    }

    /// The Models section: the default model, then each named model with its fields.
    fn model_fields(&self) -> Vec<Field> {
        let mut rows = vec![setting("model")];
        let names = self.names_in("models");
        if names.is_empty() {
            rows.push(setting("models"));
        }
        rows.push(Field::Heading("Named models".into()));
        for name in names {
            rows.push(Field::Heading(format!("models.{name}")));
            for key in ["provider", "model", "label"] {
                rows.push(Field::Setting { path: format!("models.{name}.{key}"), spec: spec_for(&format!("models.{name}.{key}")).expect("a models spec") });
            }
            rows.push(Field::Button { label: format!("Remove {name}"), action: Button::RemoveModel(name) });
        }
        rows.push(Field::Button { label: "Add a model".into(), action: Button::AddModel });
        rows
    }

    /// The Providers section: each provider's fields and its Test button.
    fn provider_fields(&self) -> Vec<Field> {
        let mut rows = Vec::new();
        for name in self.names_in("providers") {
            rows.push(Field::Heading(format!("providers.{name}")));
            rows.push(Field::Button { label: format!("Test {name}"), action: Button::TestProvider(name.clone()) });
            for spec in in_section(Section::Providers) {
                let path = spec.path.replace('*', &name);
                rows.push(Field::Setting { path, spec });
            }
        }
        rows.push(Field::Button { label: "Add a provider".into(), action: Button::AddProvider });
        rows
    }

    /// A section with plain settings and one `table.*` setting expanded to each key of the table,
    /// such as every source or every template.
    /// @param section - the section
    /// @param table - the table whose keys are expanded
    fn expanded_fields(&self, section: Section, table: &str) -> Vec<Field> {
        let mut rows = Vec::new();
        for spec in in_section(section) {
            if spec.path == format!("{table}.*") {
                let keys: Vec<String> = current_value(&self.config, table).and_then(|v| v.as_object().map(|m| m.keys().cloned().collect())).unwrap_or_default();
                rows.extend(keys.into_iter().map(|key| Field::Setting { path: format!("{table}.{key}"), spec }));
            } else if !spec.path.contains('*') {
                rows.push(Field::Setting { path: spec.path.to_string(), spec });
            }
        }
        rows
    }

    /// Where a value comes from, as a short tag.
    /// @param path - the setting path
    fn origin_tag(&self, path: &str) -> &'static str {
        if self.edits.contains_key(path) {
            return "changed";
        }
        if self.saved_later.iter().any(|p| p == path) {
            return "applies to the next session";
        }
        match value_origin(path, &self.user_file, &self.project_file) {
            Origin::Default => "default",
            Origin::User => "user file",
            Origin::Project => "project file",
        }
    }
}

/// The rows of a section whose settings all have plain paths.
/// @param section - the section
fn plain_fields(section: Section) -> Vec<Field> {
    in_section(section).into_iter().filter(|s| !s.path.contains('*')).map(|spec| Field::Setting { path: spec.path.to_string(), spec }).collect()
}

/// The row of a plain setting.
/// @param path - the setting path
fn setting(path: &str) -> Field {
    Field::Setting { path: path.to_string(), spec: spec_for(path).expect("a spec for the path") }
}

impl ConfigModal {
    /// Acts on Enter or Space on the selected field: a toggle flips, a choice or a model opens a
    /// list, a button runs, anything else opens a text field.
    fn activate(&mut self) {
        let fields = self.fields();
        let Some(field) = fields.get(self.field).cloned() else { return };
        self.message = None;
        match field {
            Field::Setting { path, spec } => match spec.kind {
                FieldKind::Toggle => {
                    let on = self.value(&path).and_then(|v| v.as_bool()).unwrap_or(false);
                    self.edits.insert(path, Some(Value::Bool(!on)));
                }
                FieldKind::Choice(options) => self.open_chooser(&path, options.iter().map(|o| o.to_string()).collect()),
                FieldKind::Model => {
                    let options = self.model_options(&path);
                    self.open_chooser(&path, options);
                }
                _ => {
                    let text = if spec.kind == FieldKind::Secret { String::new() } else { display_value(spec, self.value(&path).as_ref()).trim_end_matches('%').to_string() };
                    self.editor = Some(Editor { path, text, error: None });
                }
            },
            Field::Button { action, .. } => self.press(action),
            Field::Heading(_) | Field::Info(_) => {}
        }
    }

    /// The values a model field offers: for the default model the named models and the known
    /// models, for a named model the known models of its provider's kind, then `Custom...`.
    /// @param path - the model field's path
    fn model_options(&self, path: &str) -> Vec<String> {
        let mut options: Vec<String> = Vec::new();
        if path == "model" {
            options.extend(self.names_in("models"));
            options.extend(crate::models::model_catalog::KNOWN_MODELS.iter().map(|m| format!("{}/{}", if m.kind == "anthropic" { "anthropic" } else { "openai" }, m.model)));
        } else {
            let entry = path.trim_end_matches(".model");
            let provider = self.value(&format!("{entry}.provider")).and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
            let kind = self.config.providers.get(&provider).map(|p| p.kind.clone()).unwrap_or(provider);
            options.extend(known_for_kind(&kind).into_iter().map(|m| m.model.to_string()));
        }
        options.push("Custom...".into());
        options
    }

    /// Opens a list of values with the current one selected.
    /// @param path - the setting path
    /// @param options - the values
    fn open_chooser(&mut self, path: &str, options: Vec<String>) {
        let current = self.value(path).and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        let selected = options.iter().position(|o| *o == current).unwrap_or(0);
        self.chooser = Some(Chooser { path: path.to_string(), options, selected });
    }

    /// Runs a button.
    /// @param action - what it does
    fn press(&mut self, action: Button) {
        match action {
            Button::TestProvider(name) => self.start_test(name),
            Button::AddProvider => self.editor = Some(Editor { path: "new provider".into(), text: String::new(), error: None }),
            Button::AddModel => self.editor = Some(Editor { path: "new model".into(), text: String::new(), error: None }),
            Button::RemoveModel(name) => {
                self.edits.retain(|path, _| !path.starts_with(&format!("models.{name}.")));
                self.edits.insert(format!("models.{name}"), None);
                self.config.models.named.remove(&name);
            }
        }
    }

    /// Tests a provider on a worker thread with the model the session uses for it, or its first
    /// known model.
    /// @param name - the provider name
    fn start_test(&mut self, name: String) {
        let config = self.config_with_edits();
        let model = provider_model(&config, &name);
        *self.test.lock().unwrap_or_else(|e| e.into_inner()) = TestRun { provider: name.clone(), steps: Vec::new(), running: true };
        let shared = self.test.clone();
        std::thread::spawn(move || {
            let progress = shared.clone();
            let steps = test_provider(&config, &name, &model, &mut |step| {
                progress.lock().unwrap_or_else(|e| e.into_inner()).steps.push(step.clone());
                crate::utils::screen::redraw();
            });
            let mut run = shared.lock().unwrap_or_else(|e| e.into_inner());
            run.steps = steps;
            run.running = false;
            drop(run);
            crate::utils::screen::redraw();
        });
    }

    /// The running config with the unsaved changes laid over it.
    fn config_with_edits(&self) -> UmbraConfig {
        apply_edits(&self.config, &self.edits).unwrap_or_else(|_| self.config.clone())
    }

    /// Confirms the text field: a new provider or model name adds its table, anything else is
    /// parsed for its setting. A parse error is shown under the field and nothing changes.
    fn confirm_editor(&mut self) {
        let Some(editor) = self.editor.take() else { return };
        let text = editor.text.trim().to_string();
        let name_ok = !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        match editor.path.as_str() {
            "new provider" | "new model" if !name_ok => self.editor = Some(Editor { error: Some("Use letters, digits, - and _ only.".into()), ..editor }),
            "new provider" => {
                self.edits.insert(format!("providers.{text}.kind"), Some(Value::String("openai".into())));
                self.edits.insert(format!("providers.{text}.base_url"), Some(Value::String("http://localhost:8080/v1".into())));
            }
            "new model" => {
                self.edits.insert(format!("models.{text}.provider"), Some(Value::String("anthropic".into())));
                self.edits.insert(format!("models.{text}.model"), Some(Value::String("claude-sonnet-5-5".into())));
            }
            path => {
                let Some(spec) = spec_for(path) else { return };
                match parse_input(spec, &text) {
                    Ok(value) => {
                        self.edits.insert(path.to_string(), Some(value));
                    }
                    Err(error) => self.editor = Some(Editor { error: Some(error), ..editor }),
                }
            }
        }
    }

    /// Writes the changed keys to the chosen file, then gives the session the new config. A
    /// failed write leaves the file as it was and the modal open with the error.
    fn save(&mut self) -> Outcome {
        if self.edits.is_empty() {
            self.message = Some(("Nothing to save.".into(), false));
            return Outcome::Keep;
        }
        let changes: Vec<(String, Option<Value>)> = self.edits.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let file = self.target_file();
        if let Err(error) = write_changes(&file, &changes) {
            self.message = Some((format!("Not saved: {error:#}"), true));
            return Outcome::Keep;
        }
        let updated = match apply_edits(&self.config, &self.edits) {
            Ok(config) => config,
            Err(error) => {
                self.message = Some((format!("Saved, but the session cannot use it: {error}"), true));
                return Outcome::Keep;
            }
        };
        for path in self.edits.keys() {
            if spec_for(path).is_some_and(|s| s.applies == Applies::NextSession) && !self.saved_later.contains(path) {
                self.saved_later.push(path.clone());
            }
        }
        self.edits.clear();
        self.config = updated.clone();
        live::set_config(updated.clone());
        live::request(UiRequest::ApplyConfig(Box::new(updated)));
        self.message = Some((format!("Saved to {}.", file.display()), false));
        Outcome::Keep
    }
}

/// Lays changes over a config: each path is set in the config's JSON form and the result is read
/// back, so a value of the wrong type is an error.
/// @param config - the config
/// @param edits - the changes by path; None removes the key
fn apply_edits(config: &UmbraConfig, edits: &BTreeMap<String, Option<Value>>) -> Result<UmbraConfig, String> {
    let mut json = serde_json::to_value(config).map_err(|e| e.to_string())?;
    for (path, value) in edits {
        set_path(&mut json, path, value.clone());
    }
    serde_json::from_value(json).map_err(|e| e.to_string())
}

/// Sets or removes one dotted path in a JSON object, creating the tables on the way.
/// @param json - the object
/// @param path - the dotted path
/// @param value - the value, or None to remove the key
fn set_path(json: &mut Value, path: &str, value: Option<Value>) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut node = json;
    for part in &parts[..parts.len() - 1] {
        if !node.get(*part).is_some_and(Value::is_object) {
            node[*part] = Value::Object(Default::default());
        }
        node = node.get_mut(*part).expect("just made");
    }
    let last = parts[parts.len() - 1];
    match (value, node.as_object_mut()) {
        (Some(value), Some(map)) => {
            map.insert(last.to_string(), value);
        }
        (None, Some(map)) => {
            map.remove(last);
        }
        _ => {}
    }
}

/// The model a provider test asks: the session's model when it uses that provider, a named model
/// of that provider, or the first known model of its kind.
/// @param config - the config
/// @param provider - the provider name
fn provider_model(config: &UmbraConfig, provider: &str) -> String {
    let current = crate::services::providers::resolve_model(config, &config.model);
    if let Some(model) = current.strip_prefix(&format!("{provider}/")) {
        return model.to_string();
    }
    if let Some(named) = config.models.named.values().find(|m| m.provider == provider) {
        return named.model.clone();
    }
    let kind = config.providers.get(provider).map(|p| p.kind.as_str()).unwrap_or("");
    known_for_kind(kind).first().map(|m| m.model.to_string()).unwrap_or_else(|| "default".into())
}

impl ConfigModal {
    /// Keys while a list of values is open.
    /// @param key - the key
    fn chooser_key(&mut self, key: &Key) {
        let Some(chooser) = self.chooser.as_mut() else { return };
        match key {
            Key::Up => chooser.selected = chooser.selected.saturating_sub(1),
            Key::Down => chooser.selected = (chooser.selected + 1).min(chooser.options.len().saturating_sub(1)),
            Key::Escape => self.chooser = None,
            Key::Enter | Key::Text(_) if matches!(key, Key::Enter) || matches!(key, Key::Text(t) if t == " ") => {
                let (path, choice) = (chooser.path.clone(), chooser.options[chooser.selected].clone());
                self.chooser = None;
                if choice == "Custom..." {
                    self.editor = Some(Editor { path, text: String::new(), error: None });
                } else {
                    self.edits.insert(path, Some(Value::String(choice)));
                }
            }
            _ => {}
        }
    }

    /// Keys while a text field is open.
    /// @param key - the key
    fn editor_key(&mut self, key: &Key) {
        let Some(editor) = self.editor.as_mut() else { return };
        match key {
            Key::Text(text) | Key::Paste(text) => {
                editor.text.push_str(text.trim_end_matches(['\r', '\n']));
                editor.error = None;
            }
            Key::Backspace => {
                editor.text.pop();
            }
            Key::CtrlU => editor.text.clear(),
            Key::Enter => self.confirm_editor(),
            Key::Escape => self.editor = None,
            _ => {}
        }
    }

    /// Keys while Esc asks about unsaved changes: `s` saves and closes, `d` discards and closes,
    /// Esc goes back to editing.
    /// @param key - the key
    fn closing_key(&mut self, key: &Key) -> Outcome {
        match key {
            Key::Text(t) if t == "s" => {
                self.closing = false;
                let outcome = self.save();
                if self.edits.is_empty() && !self.message.as_ref().is_some_and(|(_, e)| *e) { Outcome::Close } else { outcome }
            }
            Key::Text(t) if t == "d" => Outcome::Close,
            Key::Escape => {
                self.closing = false;
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }

    /// Moves within the pane that has the keyboard, skipping headings and lines to read.
    /// @param down - true for Down
    fn step(&mut self, down: bool) {
        if !self.in_fields {
            self.section = if down { (self.section + 1).min(Section::ALL.len() - 1) } else { self.section.saturating_sub(1) };
            self.field = 0;
            return;
        }
        let fields = self.fields();
        let mut next = self.field;
        loop {
            let moved = if down { (next + 1 < fields.len()).then(|| next + 1) } else { next.checked_sub(1) };
            let Some(index) = moved else { break };
            next = index;
            if matches!(fields[next], Field::Setting { .. } | Field::Button { .. }) {
                self.field = next;
                break;
            }
        }
    }

    /// Selects the first field that can be edited.
    fn first_field(&mut self) {
        self.field = self.fields().iter().position(|f| matches!(f, Field::Setting { .. } | Field::Button { .. })).unwrap_or(0);
    }
}

impl Overlay for ConfigModal {
    /// Centered over the transcript.
    fn placement(&self) -> Placement {
        Placement::Centered
    }

    /// The box: the title with the file, the two panes, the help or error line and the footer.
    /// @param width - the box width
    /// @param height - the rows available
    /// @param ctx - the frame
    fn rows(&mut self, width: usize, height: usize, ctx: &LayoutCtx) -> Vec<Row> {
        let body = height.saturating_sub(6).clamp(8, 30);
        let mut rows = vec![self.title_row(width, ctx)];
        let left = self.section_rows(body, ctx);
        let right = self.field_rows(width.saturating_sub(LEFT + 3), body, ctx);
        for i in 0..body {
            let (l, lt) = left.get(i).cloned().unwrap_or_default();
            let (r, rt) = right.get(i).cloned().unwrap_or_default();
            let text = format!("│{}│{}│", ansi::fit(&l, LEFT), ansi::fit(&r, width.saturating_sub(LEFT + 3)));
            let target = if self.editor.is_some() || self.chooser.is_some() { None } else { rt.or(lt) };
            rows.push(Row { text, target, ..Default::default() });
        }
        rows.push(Row::text(format!("├{}┴{}┤", "─".repeat(LEFT), "─".repeat(width.saturating_sub(LEFT + 3)))));
        rows.push(Row::text(format!("│{}│", ansi::fit(&self.note_line(ctx), width.saturating_sub(2)))));
        let keys = if self.closing { " Unsaved changes: s save · d discard · Esc keep editing" } else { " ↑↓ move · Tab switch pane · Enter edit · Ctrl+S save · Ctrl+T file · Esc close" };
        rows.push(Row::text(format!("│{}│", ansi::fit(&paint(ctx, "90", keys), width.saturating_sub(2)))));
        rows.push(Row::text(format!("└{}┘", "─".repeat(width.saturating_sub(2)))));
        rows
    }

    /// Handles a key.
    /// @param key - the key
    fn key(&mut self, key: &Key) -> Outcome {
        if self.closing {
            return self.closing_key(key);
        }
        if self.chooser.is_some() {
            self.chooser_key(key);
            return Outcome::Keep;
        }
        if self.editor.is_some() {
            self.editor_key(key);
            return Outcome::Keep;
        }
        match key {
            Key::Up | Key::BackTab if !matches!(key, Key::BackTab) || self.in_fields => self.step(false),
            Key::Down => self.step(true),
            Key::Tab | Key::Right | Key::Left | Key::BackTab => {
                self.in_fields = !self.in_fields;
                if self.in_fields {
                    self.first_field();
                }
            }
            Key::Enter | Key::Text(_) if matches!(key, Key::Enter) || matches!(key, Key::Text(t) if t == " ") => {
                if self.in_fields {
                    self.activate();
                } else {
                    self.in_fields = true;
                    self.first_field();
                }
            }
            Key::CtrlS => return self.save(),
            Key::CtrlT => self.scope = if self.scope == Scope::User { Scope::Project } else { Scope::User },
            Key::Escape if !self.edits.is_empty() => self.closing = true,
            Key::Escape => return Outcome::Close,
            _ => {}
        }
        Outcome::Keep
    }

    /// A click on a section selects it, on a field selects and activates it, on the file switches
    /// it.
    /// @param item - the row's item number
    fn click(&mut self, item: usize) -> Outcome {
        match item {
            SCOPE_ITEM => self.scope = if self.scope == Scope::User { Scope::Project } else { Scope::User },
            n if n >= FIELD_ITEM => {
                self.in_fields = true;
                self.field = n - FIELD_ITEM;
                self.activate();
            }
            n if n >= SECTION_ITEM => {
                self.section = (n - SECTION_ITEM).min(Section::ALL.len() - 1);
                self.in_fields = false;
                self.field = 0;
            }
            _ => {}
        }
        Outcome::Keep
    }

    /// The spinner moves while a provider test runs.
    fn animating(&self) -> bool {
        self.test.lock().is_ok_and(|t| t.running)
    }
}

impl ConfigModal {
    /// The top border with the title and the file a save writes.
    /// @param width - the box width
    /// @param ctx - the frame
    fn title_row(&self, width: usize, ctx: &LayoutCtx) -> Row {
        let scope = match self.scope {
            Scope::User => "scope: User ▾",
            Scope::Project => "scope: Project ▾",
        };
        let title = " Configure Umbra ";
        let fill = width.saturating_sub(2 + title.chars().count() + scope.chars().count() + 2);
        Row { text: format!("┌{}{} {scope} ┐", paint(ctx, "1", title), "─".repeat(fill)), target: Some(Target::Item(SCOPE_ITEM)), ..Default::default() }
    }

    /// The sections pane, with the selected section marked.
    /// @param body - the rows of the panes
    /// @param ctx - the frame
    fn section_rows(&self, body: usize, ctx: &LayoutCtx) -> Vec<(String, Option<Target>)> {
        Section::ALL.iter().enumerate().take(body).map(|(i, section)| {
            let selected = i == self.section;
            let marker = if selected { "›" } else { " " };
            let text = format!(" {marker} {}", section.label());
            let text = if selected && !self.in_fields { paint(ctx, "1;7", &ansi::fit(&text, LEFT)) } else if selected { paint(ctx, "1", &text) } else { text };
            (text, Some(Target::Item(SECTION_ITEM + i)))
        }).collect()
    }

    /// The fields pane: the selected section's rows around the selected field, the open list or
    /// text field under it, and the provider test's result.
    /// @param width - the pane width
    /// @param body - the rows of the panes
    /// @param ctx - the frame
    fn field_rows(&self, width: usize, body: usize, ctx: &LayoutCtx) -> Vec<(String, Option<Target>)> {
        let fields = self.fields();
        let mut rows: Vec<(String, Option<Target>)> = fields.iter().enumerate().map(|(i, f)| (self.field_text(f, i == self.field && self.in_fields, width, ctx), Some(Target::Item(FIELD_ITEM + i)))).collect();
        let mut extra = self.popup_rows(width, ctx);
        let at = (self.field + 1).min(rows.len());
        let extra_len = extra.len();
        rows.splice(at..at, extra.drain(..));
        rows.extend(self.test_rows(width, ctx));
        let anchor = at + extra_len;
        let top = anchor.saturating_sub(body.saturating_sub(1)).min(rows.len().saturating_sub(body));
        rows.into_iter().skip(top).take(body).collect()
    }

    /// One field's row: the label, the value and where it comes from.
    /// @param field - the field
    /// @param selected - true for the selected field
    /// @param width - the pane width
    /// @param ctx - the frame
    fn field_text(&self, field: &Field, selected: bool, width: usize, ctx: &LayoutCtx) -> String {
        let marker = if selected { "›" } else { " " };
        let text = match field {
            Field::Heading(text) => return paint(ctx, "1", &format!(" {text}")),
            Field::Info(text) => return paint(ctx, "90", &format!("   {}", cut_to(text, width.saturating_sub(4)))),
            Field::Button { label, .. } => format!(" {marker} [{label}]"),
            Field::Setting { path, spec } => {
                let value = display_value(spec, self.value(path).as_ref());
                let shown = match spec.kind {
                    FieldKind::Toggle => if value == "true" { "[x]".to_string() } else { "[ ]".to_string() },
                    FieldKind::Choice(_) | FieldKind::Model => format!("{value} ▾"),
                    _ => value,
                };
                let tag = self.origin_tag(path);
                let room = width.saturating_sub(4 + 22 + tag.len() + 3);
                let left = format!(" {marker} {:<22} {}", cut_to(spec.label, 22), cut_to(&shown, room));
                let pad = width.saturating_sub(visible_width(&left) + tag.len() + 1);
                format!("{left}{}{}", " ".repeat(pad.max(1)), paint(ctx, if tag == "changed" { "33" } else { "90" }, tag))
            }
        };
        if selected { paint(ctx, "1;7", &ansi::fit(&text, width)) } else { text }
    }

    /// The open list of values or text field, drawn under the selected field.
    /// @param width - the pane width
    /// @param ctx - the frame
    fn popup_rows(&self, width: usize, ctx: &LayoutCtx) -> Vec<(String, Option<Target>)> {
        if let Some(chooser) = &self.chooser {
            return chooser.options.iter().enumerate().map(|(i, option)| {
                let text = format!("      {} {option}", if i == chooser.selected { "●" } else { "○" });
                (if i == chooser.selected { paint(ctx, "1", &text) } else { text }, None)
            }).collect();
        }
        let Some(editor) = &self.editor else { return Vec::new() };
        let label = match editor.path.as_str() {
            "new provider" => "provider name".to_string(),
            "new model" => "model name".to_string(),
            path => spec_for(path).map(|s| s.label.to_string()).unwrap_or_default(),
        };
        let mut rows = vec![(ansi::fit(&format!("      {label}: {}{}", editor.text, paint(ctx, "7", " ")), width), None)];
        if let Some(error) = &editor.error {
            rows.push((paint(ctx, "31", &format!("      {error}")), None));
        }
        rows
    }

    /// The provider test's steps, at the end of the Providers section.
    /// @param width - the pane width
    /// @param ctx - the frame
    fn test_rows(&self, width: usize, ctx: &LayoutCtx) -> Vec<(String, Option<Target>)> {
        if self.current_section() != Section::Providers {
            return Vec::new();
        }
        let test = self.test.lock().unwrap_or_else(|e| e.into_inner());
        if test.provider.is_empty() {
            return Vec::new();
        }
        let mut rows = vec![(paint(ctx, "1", &format!(" Test of {}{}", test.provider, if test.running { format!(" {}", ctx.spinner) } else { String::new() })), None)];
        for step in &test.steps {
            let mark = if step.ok { paint(ctx, "32", "✔") } else { paint(ctx, "31", "✖") };
            rows.push((format!("   {mark} {:<13} {}", step.name, cut_to(&crate::ui::markdown::sanitize(&step.text), width.saturating_sub(20))), None));
        }
        rows
    }

    /// The line under the panes: a message, the open field's error, or the selected field's help.
    /// @param ctx - the frame
    fn note_line(&self, ctx: &LayoutCtx) -> String {
        if let Some((text, error)) = &self.message {
            return paint(ctx, if *error { "31" } else { "32" }, &format!(" {text}"));
        }
        let fields = self.fields();
        match fields.get(self.field) {
            Some(Field::Setting { spec, .. }) if self.in_fields => {
                let mut help = format!(" {}", spec.help);
                if spec.kind == FieldKind::Secret {
                    help.push_str(" Prefer API key variable, which keeps the key out of the file.");
                }
                paint(ctx, "90", &help)
            }
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_of_the_box_is_as_wide_as_the_box() {
        let mut modal = ConfigModal::open();
        let ctx = LayoutCtx { color: true, now: std::time::Instant::now(), spinner: '⠋' };
        for (width, height) in [(70, 18), (160, 50)] {
            for row in modal.rows(width, height, &ctx) {
                assert_eq!(visible_width(&row.text), width, "{:?}", ansi::plain(&row.text));
            }
        }
    }

    #[test]
    fn a_cut_of_a_header_row_keeps_its_width() {
        let info = crate::utils::banner::BannerInfo { version: "1".into(), model: "m".into(), folder: "f".into(), mode: "yolo".into(), skills: 1, mcp: vec![("", "off".into())], model_menu: true };
        for row in crate::utils::banner::render_banner_step(&info, 200, true, 5) {
            let base = ansi::fit(&row, 200);
            assert_eq!(visible_width(&base), 200);
            for left in [19, 20, 40] {
                assert_eq!(visible_width(&ansi::slice(&base, 0, left)), left, "{:?}", ansi::plain(&base));
                assert_eq!(visible_width(&ansi::slice(&base, left + 100, 200)), 200 - left - 100, "{:?}", ansi::plain(&base));
            }
        }
    }

    #[test]
    fn edits_are_laid_over_the_config_and_a_removed_key_is_gone() {
        let config = UmbraConfig::default();
        let mut edits = BTreeMap::new();
        edits.insert("compaction.trigger_ratio".to_string(), Some(Value::from(0.7)));
        edits.insert("providers.x.kind".to_string(), Some(Value::String("openai".into())));
        let updated = apply_edits(&config, &edits).unwrap();
        assert_eq!(updated.compaction.trigger_ratio, 0.7);
        assert_eq!(updated.providers["x"].kind, "openai");
        let mut wrong = BTreeMap::new();
        wrong.insert("step_limit".to_string(), Some(Value::String("abc".into())));
        assert!(apply_edits(&config, &wrong).is_err());
    }
}

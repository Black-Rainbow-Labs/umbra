//! The command line forms of Umbra's settings and built in commands: `umbra config` reads and
//! changes settings, `umbra mcp enable` and `disable` change which MCP servers start, `umbra
//! models` lists the models, `umbra session` controls a running session, and `umbra update`
//! installs a newer release. The configuration modal and these commands share the settings schema,
//! the value parser and the file writer, so a value means the same thing in both.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::control_service::{self, ControlRequest};
use super::mcp_settings_service as mcp_settings;
use super::settings_service::{current_value, display_value, parse_input, value_origin, write_changes, Origin};
use crate::cli::{ConfigAction, ConfigArgs, ConfigFileArgs, InspectArgs, McpAction, SessionAction, SessionArgs, UpdateArgs};
use crate::models::config::UmbraConfig;
use crate::models::settings_schema::{settings, spec_for};
use crate::repositories::config_repository::load_config;
use crate::utils::paths::{absolute_path, project_root, umbra_home};

/// The folder a command works in: `--cwd`, or the current folder.
/// @param cwd - the `--cwd` option
fn working_folder(cwd: &Option<PathBuf>) -> Result<PathBuf> {
    match cwd {
        Some(dir) => Ok(absolute_path(dir)),
        None => Ok(std::env::current_dir()?),
    }
}

/// The config file a command changes: the project's with `--project`, else the user's.
/// @param file - the file options
fn target_file(file: &ConfigFileArgs) -> Result<PathBuf> {
    let cwd = working_folder(&file.cwd)?;
    Ok(if file.project { mcp_settings::project_file(&cwd) } else { umbra_home().join("config.toml") })
}

/// The merged config of the config files alone, without command line options.
/// @param file - the file options, for the working folder
fn files_config(file: &ConfigFileArgs) -> Result<UmbraConfig> {
    load_config(&project_root(&working_folder(&file.cwd)?), &[])
}

/// Runs `umbra config` and its subcommands. With none it prints the merged config, as before.
/// @param args - the options
pub fn config_command(args: &ConfigArgs) -> Result<i32> {
    match &args.action {
        None => super::inspect_service::show_config(&args.inspect).map(|_| 0),
        Some(ConfigAction::List(list)) => config_list(list.prefix.as_deref(), &list.file, list.json),
        Some(ConfigAction::Get(key)) => config_get(&key.key, &key.file),
        Some(ConfigAction::Set(set)) => config_set(&set.key, Some(&set.value), &set.file),
        Some(ConfigAction::Unset(key)) => config_set(&key.key, None, &key.file),
        Some(ConfigAction::Path(file)) => {
            println!("{}", target_file(file)?.display());
            Ok(0)
        }
    }
}

/// Prints every setting in the schema: path, value, where it comes from and its help.
/// @param prefix - only paths that start with this
/// @param file - the file options
/// @param as_json - print JSON
fn config_list(prefix: Option<&str>, file: &ConfigFileArgs, as_json: bool) -> Result<i32> {
    let config = files_config(file)?;
    let cwd = working_folder(&file.cwd)?;
    let (user, project) = (umbra_home().join("config.toml"), mcp_settings::project_file(&cwd));
    let mut rows = Vec::new();
    for spec in settings().iter().filter(|s| !s.path.contains('*') && prefix.is_none_or(|p| s.path.starts_with(p))) {
        let value = display_value(spec, current_value(&config, spec.path).as_ref());
        let origin = match value_origin(spec.path, &user, &project) {
            Origin::Default => "default",
            Origin::User => "user",
            Origin::Project => "project",
        };
        rows.push((spec.path, value, origin, spec.help));
    }
    if as_json {
        let list: Vec<Value> = rows.iter().map(|(path, value, origin, help)| json!({ "key": path, "value": value, "from": origin, "help": help })).collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(0);
    }
    for (path, value, origin, help) in rows {
        println!("{path:<38} {:<24} {origin:<8} {help}", if value.is_empty() { "(empty)".to_string() } else { value });
    }
    Ok(0)
}

/// Prints one setting's value: a string as it is, anything else as JSON. A secret is masked.
/// @param key - the setting path
/// @param file - the file options
fn config_get(key: &str, file: &ConfigFileArgs) -> Result<i32> {
    let config = files_config(file)?;
    let value = current_value(&config, key);
    if spec_for(key).is_none() && value.is_none() {
        bail!("{key} is not a setting. `umbra config list` lists them");
    }
    let shown = match (spec_for(key), &value) {
        (Some(spec), Some(v)) if spec.kind == crate::models::settings_schema::FieldKind::Secret => display_value(spec, Some(v)),
        (_, Some(Value::String(s))) => s.clone(),
        (_, Some(other)) => other.to_string(),
        (_, None) => String::new(),
    };
    println!("{shown}");
    Ok(0)
}

/// Sets or removes one setting in a config file, then loads the config again. When the file no
/// longer makes a valid config, the file is put back as it was and the error is returned.
/// @param key - the setting path
/// @param value - the typed value, or None to remove the key
/// @param file - the file options
fn config_set(key: &str, value: Option<&str>, file: &ConfigFileArgs) -> Result<i32> {
    let path = target_file(file)?;
    let parsed = match value {
        Some(text) => Some(parse_value(key, text)?),
        None => None,
    };
    let before = std::fs::read_to_string(&path).ok();
    write_changes(&path, &[(key.to_string(), parsed.clone())])?;
    if let Err(error) = files_config(file) {
        match &before {
            Some(text) => std::fs::write(&path, text)?,
            None => std::fs::remove_file(&path)?,
        }
        return Err(error).context(format!("{key} was not changed"));
    }
    match parsed {
        Some(v) => println!("{key} = {} in {}", match &v { Value::String(s) => s.clone(), other => other.to_string() }, path.display()),
        None => println!("{key} removed from {}", path.display()),
    }
    Ok(0)
}

/// Turns typed text into a setting's value: the schema's parser when the setting has a spec,
/// else JSON, else the text as a string.
/// @param key - the setting path
/// @param text - the typed text
fn parse_value(key: &str, text: &str) -> Result<Value> {
    match spec_for(key) {
        Some(spec) => parse_input(spec, text).map_err(|e| anyhow::anyhow!("{key}: {e}")),
        None => Ok(serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))),
    }
}

/// Runs `umbra mcp enable` or `umbra mcp disable`.
/// @param action - which and for which server
pub fn mcp_command(action: &McpAction) -> Result<i32> {
    let (toggle, on) = match action {
        McpAction::Enable(t) => (t, true),
        McpAction::Disable(t) => (t, false),
    };
    let cwd = working_folder(&toggle.cwd)?;
    let file = if toggle.project { mcp_settings::project_file(&cwd) } else { mcp_settings::user_file() };
    mcp_settings::set_saved(&file, &toggle.name, on)?;
    println!("{} {} for every session in {}", toggle.name, if on { "on" } else { "off" }, file.display());
    if on && !mcp_settings::saved_enabled(&cwd, &toggle.name) {
        println!("The project file {} still turns it off. Run with --project to change that file.", mcp_settings::project_file(&cwd).display());
    }
    println!("A running session keeps its servers. `umbra session mcp {} {}` changes a running session.", if on { "on" } else { "off" }, toggle.name);
    Ok(0)
}

/// Prints the models: the default, the named models, the listed models and the known models.
/// @param args - the listing options
pub fn list_models(args: &InspectArgs) -> Result<i32> {
    let (config, _) = super::run_service::prepare_config(&args.common)?;
    let known = crate::models::model_catalog::KNOWN_MODELS;
    if args.json {
        let named: Vec<Value> = config.models.named.iter().map(|(name, m)| json!({ "name": name, "provider": m.provider, "model": m.model, "label": m.label })).collect();
        let known: Vec<Value> = known.iter().map(|m| json!({ "label": m.label, "kind": m.kind, "model": m.model, "context_window": m.context_window })).collect();
        println!("{}", serde_json::to_string_pretty(&json!({ "default": config.model, "named": named, "listed": config.models.listed, "known": known }))?);
        return Ok(0);
    }
    println!("default  {}", if config.model.is_empty() { "(none; run umbra setup)" } else { &config.model });
    if !config.models.named.is_empty() {
        println!("\nnamed models ([models.<name>] in the config)");
        for (name, model) in &config.models.named {
            println!("  {name:<16} {}/{}", model.provider, model.model);
        }
    }
    if !config.models.listed.is_empty() {
        println!("\nlisted models");
        for spec in &config.models.listed {
            println!("  {spec}");
        }
    }
    println!("\nknown models (use as provider/model, for example anthropic/claude-haiku-5-5)");
    for model in known {
        println!("  {:<12} {:<10} {}", model.label, model.kind, model.model);
    }
    Ok(0)
}

/// Runs `umbra session` and its subcommands.
/// @param args - the options
pub fn session_command(args: &SessionArgs) -> Result<i32> {
    if let SessionAction::List = args.action {
        for (id, dir) in control_service::running() {
            let state = control_service::read_state(&dir)?;
            println!("{id}  {}  {}  {}", state.model, state.mode, state.cwd);
        }
        return Ok(0);
    }
    let dir = control_service::find(args.session.as_deref())?;
    let (request, what) = match &args.action {
        SessionAction::Status { json } => return print_status(&dir, *json),
        SessionAction::List => unreachable!("handled above"),
        SessionAction::Model { name } => (ControlRequest::Model { name: name.clone() }, format!("the model changes to {name} from the next model call")),
        SessionAction::Mode { mode } => match crate::ui::overlay::pickers::parse_mode(mode) {
            Some(_) => (ControlRequest::Mode { mode: mode.clone() }, format!("the mode changes to {mode} before the next model call")),
            None => bail!("unknown mode '{mode}'. Use confirm, yolo or human"),
        },
        SessionAction::Mcp { state, name } => {
            let on = match state.as_str() {
                "on" | "enable" => true,
                "off" | "disable" => false,
                other => bail!("'{other}' is not on or off"),
            };
            (ControlRequest::Mcp { name: name.clone(), on }, format!("MCP server {name} turns {state} for this session before the next model call"))
        }
        SessionAction::Compact { focus } => (ControlRequest::Compact { focus: focus.join(" ") }, "the history is summarized before the next model call".to_string()),
        SessionAction::Quit => (ControlRequest::Quit, "the session ends after the current step".to_string()),
    };
    control_service::send(&dir, &request)?;
    println!("sent: {what}");
    Ok(0)
}

/// Prints a running session's state.
/// @param dir - the session's control folder
/// @param as_json - print JSON
fn print_status(dir: &Path, as_json: bool) -> Result<i32> {
    let state = control_service::read_state(dir)?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&state)?);
        return Ok(0);
    }
    println!("session  {}\nfolder   {}\nmodel    {}\nmode     {}", state.id, state.cwd, state.model, state.mode);
    if state.mcp.is_empty() {
        println!("mcp      none");
    }
    for (name, word) in &state.mcp {
        println!("mcp      {name:<20} {word}");
    }
    Ok(0)
}

/// Runs `umbra update`: checks for a newer release and, unless `--check` is given, installs it.
/// @param args - the options
pub fn update_command(args: &UpdateArgs) -> Result<i32> {
    let current = super::update_service::VERSION;
    let latest = match &args.version {
        Some(version) => version.trim_start_matches('v').to_string(),
        None => super::update_service::check_now()?,
    };
    let newer = args.version.is_some() || super::update_service::is_newer(&latest, current);
    if !newer {
        println!("umbra {current} is the newest release");
        return Ok(0);
    }
    if args.check {
        println!("umbra {latest} is available. You have {current}. Run `umbra update` to install it.");
        return Ok(0);
    }
    let installed = super::update_service::install(args.version.as_deref(), &mut |line| println!("{line}"))?;
    println!("umbra {installed} is installed");
    Ok(0)
}

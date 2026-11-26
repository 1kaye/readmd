use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui_kit::{Action, App, Global, KeyBinding, KeyBindingContextPredicate};

use crate::document::{
    CommandCompleteHint, CommandNextHint, CommandPrevHint, EnterInsertMode, EnterNormalMode,
    NewFile, OpenCommand, OpenFile, Quit, SaveFile, ScrollDown, ScrollUp, ToggleHelp,
};

const WORKSPACE_CONTEXT: &str = "DocumentWorkspace";
const NORMAL_PREDICATE: &str = "DocumentWorkspace && mode == normal";
const INSERT_PREDICATE: &str = "DocumentWorkspace && mode == insert";
const INSERT_INPUT_PREDICATE: &str = "DocumentWorkspace && mode == insert > Input";
const COMMAND_PREDICATE: &str = "DocumentWorkspace && mode == command";
const COMMAND_INPUT_PREDICATE: &str = "DocumentWorkspace && mode == command > Input";

pub const DEFAULT_KEYMAPS_TOML: &str = "\
# readmd keybindings
# Uses GPUI keystroke syntax:
#   single key : \"ctrl-s\", \"secondary-o\", \"i\", \"escape\", \":\"
#   chord      : \"space w\", \"j k\"
#   multiple   : [\"i\", \"a\"]

new_file = \"secondary-n\"
open_file = \"secondary-o\"
save_file = \"secondary-s\"
quit = \"ctrl-q\"
toggle_help = \"shift-/\"
enter_insert = \"i\"
enter_normal = \"escape\"
open_command = \":\"
scroll_down = [\"j\", \"down\"]
scroll_up = [\"k\", \"up\"]
";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeymapConfig {
    pub new_file: Vec<String>,
    pub open_file: Vec<String>,
    pub save_file: Vec<String>,
    pub quit: Vec<String>,
    pub toggle_help: Vec<String>,
    pub enter_insert: Vec<String>,
    pub enter_normal: Vec<String>,
    pub open_command: Vec<String>,
    pub scroll_down: Vec<String>,
    pub scroll_up: Vec<String>,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self {
            new_file: vec!["secondary-n".to_string()],
            open_file: vec!["secondary-o".to_string()],
            save_file: vec!["secondary-s".to_string()],
            quit: vec!["ctrl-q".to_string()],
            toggle_help: vec!["shift-/".to_string()],
            enter_insert: vec!["i".to_string()],
            enter_normal: vec!["escape".to_string()],
            open_command: vec![":".to_string()],
            scroll_down: vec!["j".to_string(), "down".to_string()],
            scroll_up: vec!["k".to_string(), "up".to_string()],
        }
    }
}

impl KeymapConfig {
    pub fn uses_escape_for_normal(&self) -> bool {
        self.enter_normal
            .iter()
            .any(|key| key.eq_ignore_ascii_case("escape"))
    }
}

/// Application-wide keymap registry retaining GPUI Base's initial bindings
/// so reloading user keybindings never strips built-in Input/TextView/Root shortcuts.
struct KeymapRegistry {
    base_bindings: Vec<KeyBinding>,
    config_path: Option<PathBuf>,
    active: KeymapConfig,
    startup_notice: Option<String>,
}

impl Global for KeymapRegistry {}

/// Initializes the keymap registry right after `gpui_kit::init(cx)` and loads
/// user overrides from `~/.config/readmd/keymaps.toml` (falling back to defaults).
pub fn init(cx: &mut App) {
    let base_bindings: Vec<KeyBinding> = cx.key_bindings().borrow().bindings().cloned().collect();
    let config_path = default_config_path();

    let (config, startup_notice) = match &config_path {
        Some(path) => load_or_create_config(path),
        None => (KeymapConfig::default(), None),
    };

    let (active, startup_notice) = match apply_bindings(&base_bindings, &config, cx) {
        Ok(()) => (config, startup_notice),
        Err(err) => {
            let defaults = KeymapConfig::default();
            let _ = apply_bindings(&base_bindings, &defaults, cx);
            (defaults, Some(err))
        }
    };

    cx.set_global(KeymapRegistry {
        base_bindings,
        config_path,
        active,
        startup_notice,
    });
}

pub fn config_path(cx: &App) -> Option<PathBuf> {
    cx.try_global::<KeymapRegistry>()
        .and_then(|registry| registry.config_path.clone())
}

pub fn is_config_path(path: &Path, cx: &App) -> bool {
    config_path(cx).is_some_and(|config| config == path)
}

pub fn uses_escape_for_normal(cx: &App) -> bool {
    cx.try_global::<KeymapRegistry>()
        .is_none_or(|registry| registry.active.uses_escape_for_normal())
}

pub fn take_startup_notice(cx: &mut App) -> Option<String> {
    if !cx.has_global::<KeymapRegistry>() {
        return None;
    }
    cx.global_mut::<KeymapRegistry>().startup_notice.take()
}

/// Atomically reloads keybindings from disk. If parsing or keystroke validation fails,
/// keeps the previously active keymap intact and returns the error message.
pub fn reload(cx: &mut App) -> Result<(), String> {
    let (base_bindings, path) = {
        let registry = cx
            .try_global::<KeymapRegistry>()
            .ok_or_else(|| "Keymap registry is not initialized".to_string())?;
        let path = registry
            .config_path
            .clone()
            .ok_or_else(|| "Could not determine config directory".to_string())?;
        (registry.base_bindings.clone(), path)
    };

    let content = std::fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read {}: {err}", path.display()))?;
    let config = parse_keymaps_toml(&content)?;
    apply_bindings(&base_bindings, &config, cx)?;

    if cx.has_global::<KeymapRegistry>() {
        cx.global_mut::<KeymapRegistry>().active = config;
    }
    Ok(())
}

pub fn default_config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(xdg).join("readmd").join("keymaps.toml"));
    }
    let home = std::env::var_os("HOME").filter(|v| !v.is_empty())?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("readmd")
            .join("keymaps.toml"),
    )
}

fn load_or_create_config(path: &Path) -> (KeymapConfig, Option<String>) {
    if !path.exists() {
        let _ = write_default_config(path);
        return (KeymapConfig::default(), None);
    }

    match std::fs::read_to_string(path) {
        Ok(content) => match parse_keymaps_toml(&content) {
            Ok(config) => (config, None),
            Err(err) => (
                KeymapConfig::default(),
                Some(format!("keymaps.toml: {err}")),
            ),
        },
        Err(err) => (
            KeymapConfig::default(),
            Some(format!("Failed to read {}: {err}", path.display())),
        ),
    }
}

fn write_default_config(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create {}: {err}", parent.display()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, DEFAULT_KEYMAPS_TOML)
        .map_err(|err| format!("Failed to write {}: {err}", path.display()))?;
    std::fs::rename(&tmp, path).map_err(|err| {
        let _ = std::fs::remove_file(&tmp);
        format!("Failed to write {}: {err}", path.display())
    })
}

fn apply_bindings(
    base_bindings: &[KeyBinding],
    config: &KeymapConfig,
    cx: &mut App,
) -> Result<(), String> {
    let mut app_bindings = Vec::new();
    let workspace_ctx = parse_context(WORKSPACE_CONTEXT)?;
    let normal_ctx = parse_context(NORMAL_PREDICATE)?;
    let insert_ctx = parse_context(INSERT_PREDICATE)?;
    let insert_input_ctx = parse_context(INSERT_INPUT_PREDICATE)?;
    let command_ctx = parse_context(COMMAND_PREDICATE)?;
    let command_input_ctx = parse_context(COMMAND_INPUT_PREDICATE)?;

    push_bindings(
        &mut app_bindings,
        &config.new_file,
        || Box::new(NewFile),
        &workspace_ctx,
        cx,
    )?;
    push_bindings(
        &mut app_bindings,
        &config.open_file,
        || Box::new(OpenFile),
        &workspace_ctx,
        cx,
    )?;
    push_bindings(
        &mut app_bindings,
        &config.save_file,
        || Box::new(SaveFile),
        &workspace_ctx,
        cx,
    )?;
    push_bindings(
        &mut app_bindings,
        &config.quit,
        || Box::new(Quit),
        &workspace_ctx,
        cx,
    )?;
    push_bindings(
        &mut app_bindings,
        &config.toggle_help,
        || Box::new(ToggleHelp),
        &normal_ctx,
        cx,
    )?;
    push_bindings(
        &mut app_bindings,
        &config.enter_insert,
        || Box::new(EnterInsertMode),
        &normal_ctx,
        cx,
    )?;
    push_bindings(
        &mut app_bindings,
        &config.open_command,
        || Box::new(OpenCommand),
        &normal_ctx,
        cx,
    )?;
    push_bindings(
        &mut app_bindings,
        &config.scroll_down,
        || Box::new(ScrollDown),
        &normal_ctx,
        cx,
    )?;
    push_bindings(
        &mut app_bindings,
        &config.scroll_up,
        || Box::new(ScrollUp),
        &normal_ctx,
        cx,
    )?;
    app_bindings.push(
        KeyBinding::load(
            "escape",
            Box::new(EnterNormalMode),
            Some(normal_ctx),
            false,
            None,
            cx,
        )
        .map_err(|err| format!("Invalid keybinding 'escape': {err}"))?,
    );

    // Command bar navigation and completion (supersedes single-line Input up/down/tab).
    for ctx in [&command_ctx, &command_input_ctx] {
        for (key, make_action) in [
            (
                "down",
                (|| Box::new(CommandNextHint) as Box<dyn Action>) as fn() -> Box<dyn Action>,
            ),
            ("ctrl-n", || Box::new(CommandNextHint)),
            ("up", || Box::new(CommandPrevHint)),
            ("ctrl-p", || Box::new(CommandPrevHint)),
            ("shift-tab", || Box::new(CommandPrevHint)),
            ("tab", || Box::new(CommandCompleteHint)),
        ] {
            let binding = KeyBinding::load(key, make_action(), Some(ctx.clone()), false, None, cx)
                .map_err(|err| format!("Invalid keybinding '{key}': {err}"))?;
            app_bindings.push(binding);
        }
    }

    app_bindings.push(
        KeyBinding::load(
            "escape",
            Box::new(EnterNormalMode),
            Some(command_ctx),
            false,
            None,
            cx,
        )
        .map_err(|err| format!("Invalid keybinding 'escape': {err}"))?,
    );

    for key in &config.enter_normal {
        let binding = KeyBinding::load(
            key,
            Box::new(EnterNormalMode),
            Some(insert_ctx.clone()),
            false,
            None,
            cx,
        )
        .map_err(|err| format!("Invalid keybinding '{key}': {err}"))?;
        app_bindings.push(binding);

        // Leave `escape` inside `Input` to gpui-base first so IME/multi-cursor dismissal runs
        // before propagating `base::input::Escape`. Any custom key (e.g. `ctrl-[` or `j k`)
        // binds inside `DocumentWorkspace && mode == insert > Input` so it works while typing.
        if !key.eq_ignore_ascii_case("escape") {
            let input_binding = KeyBinding::load(
                key,
                Box::new(EnterNormalMode),
                Some(insert_input_ctx.clone()),
                false,
                None,
                cx,
            )
            .map_err(|err| format!("Invalid keybinding '{key}': {err}"))?;
            app_bindings.push(input_binding);
        }
    }

    cx.clear_key_bindings();
    cx.bind_keys(base_bindings.iter().cloned());
    cx.bind_keys(app_bindings);
    Ok(())
}

fn parse_context(source: &str) -> Result<Rc<KeyBindingContextPredicate>, String> {
    KeyBindingContextPredicate::parse(source)
        .map(Rc::new)
        .map_err(|err| format!("Invalid key context '{source}': {err}"))
}

fn push_bindings(
    out: &mut Vec<KeyBinding>,
    keys: &[String],
    make_action: impl Fn() -> Box<dyn Action>,
    context: &Rc<KeyBindingContextPredicate>,
    cx: &App,
) -> Result<(), String> {
    for key in keys {
        let binding = KeyBinding::load(key, make_action(), Some(context.clone()), false, None, cx)
            .map_err(|err| format!("Invalid keybinding '{key}': {err}"))?;
        out.push(binding);
    }
    Ok(())
}

/// Parses a flat TOML keymap file supporting string values (`key = "ctrl-s"`)
/// and string array values (`key = ["i", "a"]`). Unspecified keys retain defaults.
pub fn parse_keymaps_toml(source: &str) -> Result<KeymapConfig, String> {
    let mut config = KeymapConfig::default();

    for (ix, raw_line) in source.lines().enumerate() {
        let line_no = ix + 1;
        let line = strip_toml_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            return Err(format!("line {line_no}: expected `key = \"value\"`"));
        };
        let key = raw_key.trim();
        let values = parse_toml_string_or_array(raw_value.trim())
            .map_err(|err| format!("line {line_no} ({key}): {err}"))?;

        match key {
            "new_file" => config.new_file = values,
            "open_file" => config.open_file = values,
            "save_file" => config.save_file = values,
            "quit" => config.quit = values,
            "toggle_help" => config.toggle_help = values,
            "enter_insert" => config.enter_insert = values,
            "enter_normal" => config.enter_normal = values,
            "open_command" => config.open_command = values,
            "scroll_down" => config.scroll_down = values,
            "scroll_up" => config.scroll_up = values,
            _ => return Err(format!("line {line_no}: unknown keybind '{key}'")),
        }
    }

    Ok(config)
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (ix, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            '#' if !in_quotes => return &line[..ix],
            _ => {}
        }
    }
    line
}

fn parse_toml_string_or_array(raw: &str) -> Result<Vec<String>, String> {
    if raw.starts_with('"') {
        let (value, rest) = parse_quoted_string(raw)?;
        if !rest.trim().is_empty() {
            return Err("unexpected trailing characters after string".to_string());
        }
        if value.trim().is_empty() {
            return Err("keybinding string cannot be empty".to_string());
        }
        return Ok(vec![value]);
    }

    if let Some(inner) = raw.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let mut rest = inner.trim();
        let mut items = Vec::new();
        while !rest.is_empty() {
            let (value, after) = parse_quoted_string(rest)?;
            if value.trim().is_empty() {
                return Err("keybinding string cannot be empty".to_string());
            }
            items.push(value);
            rest = after.trim();
            if let Some(next) = rest.strip_prefix(',') {
                rest = next.trim();
            } else if !rest.is_empty() {
                return Err("expected ',' between array elements".to_string());
            }
        }
        return Ok(items);
    }

    Err("expected a quoted string or array of strings".to_string())
}

fn parse_quoted_string(raw: &str) -> Result<(String, &str), String> {
    let Some(rest) = raw.strip_prefix('"') else {
        return Err("expected '\"'".to_string());
    };
    let mut out = String::new();
    let mut escaped = false;
    for (ix, ch) in rest.char_indices() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Ok((out, &rest[ix + 1..])),
            _ => out.push(ch),
        }
    }
    Err("unterminated string literal".to_string())
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_KEYMAPS_TOML, KeymapConfig, parse_keymaps_toml};

    #[test]
    fn default_toml_parses_to_default_config() {
        let parsed = parse_keymaps_toml(DEFAULT_KEYMAPS_TOML).expect("default toml must parse");
        assert_eq!(parsed, KeymapConfig::default());
        assert!(parsed.uses_escape_for_normal());
    }

    #[test]
    fn supports_single_strings_arrays_comments_and_partial_overrides() {
        let src = r#"
            # User overrides
            save_file = "ctrl-s" # inline comment
            enter_insert = ["i", "a"]
            enter_normal = ["escape", "j k"]
            open_command = ";"
        "#;
        let parsed = parse_keymaps_toml(src).expect("valid overrides");
        assert_eq!(parsed.open_file, vec!["secondary-o"]);
        assert_eq!(parsed.save_file, vec!["ctrl-s"]);
        assert_eq!(parsed.enter_insert, vec!["i", "a"]);
        assert_eq!(parsed.enter_normal, vec!["escape", "j k"]);
        assert_eq!(parsed.open_command, vec![";"]);
    }

    #[test]
    fn allows_unbinding_with_empty_array_and_rejects_invalid_entries() {
        let parsed = parse_keymaps_toml("open_command = []").expect("empty array allowed");
        assert!(parsed.open_command.is_empty());

        assert!(parse_keymaps_toml("unknown_action = \"ctrl-x\"").is_err());
        assert!(parse_keymaps_toml("save_file = ctrl-s").is_err());
        assert!(parse_keymaps_toml("save_file = \"\"").is_err());
        assert!(parse_keymaps_toml("save_file = \"unterminated").is_err());
    }
}

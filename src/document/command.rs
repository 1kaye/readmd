use gpui_kit::SharedString;

/// Maximum number of hint rows shown below the command box at once.
pub const MAX_VISIBLE_HINTS: usize = 10;

/// Commands accepted by the top-center `:` command line (LazyVim style).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Noop,
    Write { path: Option<String> },
    Quit { force: bool },
    WriteQuit { path: Option<String> },
    Open { path: Option<String> },
    SetFont { name: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandHint {
    pub label: SharedString,
    pub description: SharedString,
    /// Text inserted into the command line when completed/selected.
    /// Trailing space indicates the command expects an argument (keeps command bar open).
    pub insert_text: SharedString,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HintList {
    pub items: Vec<CommandHint>,
    pub footer: Option<SharedString>,
}

struct CommandSpec {
    name: &'static str,
    aliases: &'static [&'static str],
    label: &'static str,
    description: &'static str,
    insert_text: &'static str,
}

const COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        name: "w",
        aliases: &["write"],
        label: "w [path]",
        description: "Save current file (or open save dialog)",
        insert_text: "w",
    },
    CommandSpec {
        name: "wq",
        aliases: &["wq!", "x"],
        label: "wq [path]",
        description: "Save current file and quit",
        insert_text: "wq",
    },
    CommandSpec {
        name: "q",
        aliases: &["quit"],
        label: "q",
        description: "Quit if there are no unsaved changes",
        insert_text: "q",
    },
    CommandSpec {
        name: "q!",
        aliases: &["quit!"],
        label: "q!",
        description: "Force quit without saving",
        insert_text: "q!",
    },
    CommandSpec {
        name: "e",
        aliases: &["edit", "o", "open"],
        label: "e [path]",
        description: "Open a Markdown file",
        insert_text: "e",
    },
    CommandSpec {
        name: "Font",
        aliases: &["font"],
        label: "Font <name>",
        description: "Change document font (system fonts or default)",
        insert_text: "Font ",
    },
];

pub fn parse_command(raw: &str) -> Result<Command, String> {
    let trimmed = raw.trim().trim_start_matches(':').trim();
    if trimmed.is_empty() {
        return Ok(Command::Noop);
    }

    let (name, arg) = match trimmed.split_once(char::is_whitespace) {
        Some((head, tail)) => {
            let rest = tail.trim();
            (head, (!rest.is_empty()).then(|| rest.to_string()))
        }
        None => (trimmed, None),
    };

    if name.eq_ignore_ascii_case("font") {
        return Ok(Command::SetFont { name: arg });
    }

    match name {
        "w" | "write" => Ok(Command::Write { path: arg }),
        "q" | "quit" => ensure_no_arg(name, arg, Command::Quit { force: false }),
        "q!" | "quit!" => ensure_no_arg(name, arg, Command::Quit { force: true }),
        "wq" | "wq!" | "x" => Ok(Command::WriteQuit { path: arg }),
        "e" | "edit" | "o" | "open" => Ok(Command::Open { path: arg }),
        _ => Err(format!("Unknown command: :{name}")),
    }
}

/// Builds live English hints for the current command-line text.
/// Typing `:Font ` switches hints into a live-filtered list of installed system fonts.
pub fn command_hints(
    raw: &str,
    available_fonts: &[SharedString],
    current_font: Option<&str>,
) -> HintList {
    let input = raw.trim_start().trim_start_matches(':').trim_start();
    if input.is_empty() {
        return HintList {
            items: COMMAND_SPECS.iter().map(spec_to_hint).collect(),
            footer: None,
        };
    }

    if let Some((head, tail)) = input.split_once(char::is_whitespace) {
        if head.eq_ignore_ascii_case("font") {
            return font_hints(tail.trim(), available_fonts, current_font);
        }
        return active_command_arg_hint(head, tail.trim());
    }

    let query = input.to_ascii_lowercase();
    let mut prefix_matches = Vec::new();
    let mut contains_matches = Vec::new();

    for spec in COMMAND_SPECS {
        let name_lower = spec.name.to_ascii_lowercase();
        let alias_prefix = spec
            .aliases
            .iter()
            .any(|alias| alias.to_ascii_lowercase().starts_with(&query));
        let alias_contains = spec
            .aliases
            .iter()
            .any(|alias| alias.to_ascii_lowercase().contains(&query));

        if name_lower.starts_with(&query) || alias_prefix {
            prefix_matches.push(spec_to_hint(spec));
        } else if name_lower.contains(&query) || alias_contains {
            contains_matches.push(spec_to_hint(spec));
        }
    }

    prefix_matches.extend(contains_matches);
    HintList {
        items: prefix_matches,
        footer: None,
    }
}

/// Resolves a requested font name case-insensitively against installed system fonts.
/// Returns `Ok(None)` for `"default"` / `"reset"`, `Ok(Some(canonical))` for an installed font,
/// or an error if no installed family matches.
pub fn resolve_font_name(
    requested: &str,
    available_fonts: &[SharedString],
) -> Result<Option<SharedString>, String> {
    let query = requested.trim();
    if query.eq_ignore_ascii_case("default") || query.eq_ignore_ascii_case("reset") {
        return Ok(None);
    }

    if let Some(exact) = available_fonts
        .iter()
        .find(|font| font.as_ref().eq_ignore_ascii_case(query))
    {
        return Ok(Some(exact.clone()));
    }

    let query_lower = query.to_ascii_lowercase();
    let mut prefix_matches = available_fonts
        .iter()
        .filter(|font| font.as_ref().to_ascii_lowercase().starts_with(&query_lower));
    if let Some(first) = prefix_matches.next()
        && prefix_matches.next().is_none()
    {
        return Ok(Some(first.clone()));
    }

    Err(format!("Font not found: {query}"))
}

fn spec_to_hint(spec: &CommandSpec) -> CommandHint {
    CommandHint {
        label: SharedString::from(spec.label),
        description: SharedString::from(spec.description),
        insert_text: SharedString::from(spec.insert_text),
    }
}

fn active_command_arg_hint(head: &str, arg: &str) -> HintList {
    let items = match head {
        "w" | "write" => vec![CommandHint {
            label: SharedString::from("w [path]"),
            description: SharedString::from(if arg.is_empty() {
                "Save current file (or type a file path)"
            } else {
                "Save buffer to the specified file path"
            }),
            insert_text: SharedString::from(if arg.is_empty() {
                "w".to_string()
            } else {
                format!("w {arg}")
            }),
        }],
        "wq" | "wq!" | "x" => vec![CommandHint {
            label: SharedString::from("wq [path]"),
            description: SharedString::from(if arg.is_empty() {
                "Save current file and quit"
            } else {
                "Save buffer to the specified path and quit"
            }),
            insert_text: SharedString::from(if arg.is_empty() {
                "wq".to_string()
            } else {
                format!("wq {arg}")
            }),
        }],
        "e" | "edit" | "o" | "open" => vec![CommandHint {
            label: SharedString::from("e [path]"),
            description: SharedString::from(if arg.is_empty() {
                "Open a Markdown file (or type a file path)"
            } else {
                "Open the specified file path"
            }),
            insert_text: SharedString::from(if arg.is_empty() {
                "e".to_string()
            } else {
                format!("e {arg}")
            }),
        }],
        _ => Vec::new(),
    };

    HintList {
        items,
        footer: None,
    }
}

fn font_hints(
    query: &str,
    available_fonts: &[SharedString],
    current_font: Option<&str>,
) -> HintList {
    let query_lower = query.to_ascii_lowercase();
    let mut items = Vec::new();

    if query_lower.is_empty() || "default".contains(&query_lower) {
        items.push(CommandHint {
            label: SharedString::from("default"),
            description: SharedString::from(if current_font.is_none() {
                "Reset to system default font (active)"
            } else {
                "Reset to system default font"
            }),
            insert_text: SharedString::from("Font default"),
        });
    }

    let mut prefix_matches = Vec::new();
    let mut contains_matches = Vec::new();
    for font in available_fonts {
        let lower = font.as_ref().to_ascii_lowercase();
        if query_lower.is_empty() || lower.starts_with(&query_lower) {
            prefix_matches.push(font);
        } else if lower.contains(&query_lower) {
            contains_matches.push(font);
        }
    }

    let total_fonts = prefix_matches.len() + contains_matches.len();
    let remaining_slots = MAX_VISIBLE_HINTS.saturating_sub(items.len());

    for font in prefix_matches
        .into_iter()
        .chain(contains_matches)
        .take(remaining_slots)
    {
        let is_active = current_font.is_some_and(|active| active == font.as_ref());
        items.push(CommandHint {
            label: font.clone(),
            description: SharedString::from(if is_active {
                "Active document font"
            } else {
                "System font"
            }),
            insert_text: SharedString::from(format!("Font {font}")),
        });
    }

    let footer = if total_fonts > remaining_slots {
        Some(SharedString::from(format!(
            "Showing {remaining_slots} of {total_fonts} system fonts — type to filter"
        )))
    } else if items.is_empty() {
        Some(SharedString::from("No matching system fonts"))
    } else {
        None
    };

    HintList { items, footer }
}

fn ensure_no_arg(name: &str, arg: Option<String>, cmd: Command) -> Result<Command, String> {
    if arg.is_some() {
        Err(format!(":{name} does not take arguments"))
    } else {
        Ok(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, command_hints, parse_command, resolve_font_name};
    use gpui_kit::SharedString;

    fn sample_fonts() -> Vec<SharedString> {
        vec![
            SharedString::from("DejaVu Sans"),
            SharedString::from("Inter"),
            SharedString::from("JetBrains Mono"),
            SharedString::from("Noto Sans"),
            SharedString::from("Noto Serif"),
        ]
    }

    #[test]
    fn parses_write_quit_open_and_font_commands() {
        assert_eq!(parse_command(""), Ok(Command::Noop));
        assert_eq!(parse_command("   "), Ok(Command::Noop));
        assert_eq!(parse_command("w"), Ok(Command::Write { path: None }));
        assert_eq!(parse_command(":write"), Ok(Command::Write { path: None }));
        assert_eq!(
            parse_command("w notes.md"),
            Ok(Command::Write {
                path: Some("notes.md".to_string())
            })
        );
        assert_eq!(parse_command("q"), Ok(Command::Quit { force: false }));
        assert_eq!(parse_command("q!"), Ok(Command::Quit { force: true }));
        assert_eq!(parse_command("wq"), Ok(Command::WriteQuit { path: None }));
        assert_eq!(parse_command("x"), Ok(Command::WriteQuit { path: None }));
        assert_eq!(
            parse_command("wq ~/out.md"),
            Ok(Command::WriteQuit {
                path: Some("~/out.md".to_string())
            })
        );
        assert_eq!(parse_command("e"), Ok(Command::Open { path: None }));
        assert_eq!(
            parse_command("o README.md"),
            Ok(Command::Open {
                path: Some("README.md".to_string())
            })
        );
        assert_eq!(parse_command("Font"), Ok(Command::SetFont { name: None }));
        assert_eq!(
            parse_command("font JetBrains Mono"),
            Ok(Command::SetFont {
                name: Some("JetBrains Mono".to_string())
            })
        );
    }

    #[test]
    fn rejects_unknown_commands_and_unexpected_arguments() {
        assert!(parse_command("foo").is_err());
        assert!(parse_command("keymap").is_err());
        assert!(parse_command("reload").is_err());
        assert!(parse_command("q extra").is_err());
    }

    #[test]
    fn command_hints_filter_w_to_w_and_wq_with_english_descriptions() {
        let fonts = sample_fonts();
        let hints = command_hints("w", &fonts, None);
        let labels: Vec<&str> = hints.items.iter().map(|h| h.label.as_ref()).collect();
        assert_eq!(labels, vec!["w [path]", "wq [path]"]);
        assert!(hints.items[0].description.contains("Save"));
        assert!(hints.items[1].description.contains("quit"));
    }

    #[test]
    fn font_hints_filter_installed_fonts_and_mark_active_font() {
        let fonts = sample_fonts();
        let all = command_hints("Font ", &fonts, Some("Inter"));
        assert_eq!(all.items[0].label.as_ref(), "default");
        assert!(
            all.items
                .iter()
                .any(|h| h.label.as_ref() == "Inter" && h.description.contains("Active"))
        );

        let filtered = command_hints("font noto", &fonts, None);
        let labels: Vec<&str> = filtered.items.iter().map(|h| h.label.as_ref()).collect();
        assert_eq!(labels, vec!["Noto Sans", "Noto Serif"]);
    }

    #[test]
    fn resolve_font_name_matches_case_insensitively_and_supports_default() {
        let fonts = sample_fonts();
        assert_eq!(resolve_font_name("default", &fonts), Ok(None));
        assert_eq!(resolve_font_name("reset", &fonts), Ok(None));
        assert_eq!(
            resolve_font_name("jetbrains mono", &fonts),
            Ok(Some(SharedString::from("JetBrains Mono")))
        );
        assert_eq!(
            resolve_font_name("jet", &fonts),
            Ok(Some(SharedString::from("JetBrains Mono")))
        );
        // Ambiguous prefix ("noto" matches both Noto Sans and Noto Serif) returns an error.
        assert!(resolve_font_name("noto", &fonts).is_err());
        assert!(resolve_font_name("Missing Font", &fonts).is_err());
    }
}

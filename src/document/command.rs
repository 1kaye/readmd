/// Commands accepted by the top-center `:` command line (LazyVim style).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Noop,
    Write { path: Option<String> },
    Quit { force: bool },
    WriteQuit { path: Option<String> },
    Open { path: Option<String> },
    EditKeymaps,
    ReloadKeymaps,
}

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

    match name {
        "w" | "write" => Ok(Command::Write { path: arg }),
        "q" | "quit" => ensure_no_arg(name, arg, Command::Quit { force: false }),
        "q!" | "quit!" => ensure_no_arg(name, arg, Command::Quit { force: true }),
        "wq" | "wq!" | "x" => Ok(Command::WriteQuit { path: arg }),
        "e" | "edit" | "o" | "open" => Ok(Command::Open { path: arg }),
        "keymap" | "keymaps" => ensure_no_arg(name, arg, Command::EditKeymaps),
        "reload" => ensure_no_arg(name, arg, Command::ReloadKeymaps),
        _ => Err(format!("Unknown command: :{name}")),
    }
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
    use super::{Command, parse_command};

    #[test]
    fn parses_write_quit_open_and_keymap_commands() {
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
        assert_eq!(parse_command("keymap"), Ok(Command::EditKeymaps));
        assert_eq!(parse_command("reload"), Ok(Command::ReloadKeymaps));
    }

    #[test]
    fn rejects_unknown_commands_and_unexpected_arguments() {
        assert!(parse_command("foo").is_err());
        assert!(parse_command("q extra").is_err());
        assert!(parse_command("reload extra").is_err());
    }
}

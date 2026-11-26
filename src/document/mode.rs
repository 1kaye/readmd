use std::path::{Path, PathBuf};

pub const APP_TITLE: &str = "readmd";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditorMode {
    /// Plain-text Markdown editing mode (default when opening a blank session).
    #[default]
    Insert,
    /// Rendered Markdown reading mode.
    Normal,
    /// Top-center `:` command-line overlay active over Normal mode.
    Command,
}

impl EditorMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Insert => "INSERT",
            Self::Normal => "NORMAL",
            Self::Command => "COMMAND",
        }
    }

    pub fn key_context(self) -> &'static str {
        match self {
            Self::Insert => "DocumentWorkspace mode=insert",
            Self::Normal => "DocumentWorkspace mode=normal",
            Self::Command => "DocumentWorkspace mode=command",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Insert => Self::Normal,
            Self::Normal | Self::Command => Self::Insert,
        }
    }

    /// Loading a Markdown file always enters Normal mode to render it immediately.
    pub fn on_file_loaded() -> Self {
        Self::Normal
    }
}

const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mkd", "mdx"];

pub fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            MARKDOWN_EXTENSIONS
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        })
}

pub fn select_dropped_markdown(paths: &[PathBuf]) -> Result<PathBuf, String> {
    paths
        .iter()
        .find(|path| is_markdown_path(path))
        .cloned()
        .ok_or_else(|| "Drop a Markdown file (.md, .markdown, .mkd)".to_string())
}

pub fn load_markdown_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| format!("Failed to open {}: {err}", path.display()))
}

pub fn save_markdown_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create {}: {err}", parent.display()))?;
    }
    std::fs::write(path, content).map_err(|err| format!("Failed to save {}: {err}", path.display()))
}

pub fn expand_user_path(raw: &str, base_dir: Option<&Path>) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty())
    {
        return PathBuf::from(home).join(rest);
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return path;
    }
    match base_dir {
        Some(dir) => dir.join(path),
        None => path,
    }
}

pub fn file_display_name(path: &Path) -> Option<&str> {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
}

pub fn window_title_for_state(path: Option<&Path>, is_dirty: bool) -> String {
    let base = match path.and_then(file_display_name) {
        Some(name) => format!("{name} — {APP_TITLE}"),
        None => APP_TITLE.to_string(),
    };
    if is_dirty {
        format!("• {base}")
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::{
        APP_TITLE, EditorMode, expand_user_path, file_display_name, is_markdown_path,
        load_markdown_file, save_markdown_file, select_dropped_markdown, window_title_for_state,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn select_dropped_markdown_picks_first_markdown_and_rejects_others() {
        assert!(is_markdown_path(Path::new("README.MD")));
        assert!(is_markdown_path(Path::new("notes.markdown")));
        assert!(is_markdown_path(Path::new("doc.mkd")));
        assert!(!is_markdown_path(Path::new("image.png")));

        let mixed = vec![PathBuf::from("photo.png"), PathBuf::from("/docs/guide.md")];
        assert_eq!(
            select_dropped_markdown(&mixed),
            Ok(PathBuf::from("/docs/guide.md"))
        );

        let non_md = vec![PathBuf::from("notes.txt"), PathBuf::from("folder")];
        assert!(select_dropped_markdown(&non_md).is_err());
        assert!(select_dropped_markdown(&[]).is_err());
    }

    #[test]
    fn default_mode_is_insert_and_file_load_enters_normal() {
        assert_eq!(EditorMode::default(), EditorMode::Insert);
        assert_eq!(EditorMode::on_file_loaded(), EditorMode::Normal);
    }

    #[test]
    fn mode_labels_contexts_and_toggle_match_modal_contract() {
        assert_eq!(EditorMode::Insert.label(), "INSERT");
        assert_eq!(EditorMode::Normal.label(), "NORMAL");
        assert_eq!(EditorMode::Command.label(), "COMMAND");
        assert_eq!(
            EditorMode::Insert.key_context(),
            "DocumentWorkspace mode=insert"
        );
        assert_eq!(
            EditorMode::Normal.key_context(),
            "DocumentWorkspace mode=normal"
        );
        assert_eq!(
            EditorMode::Command.key_context(),
            "DocumentWorkspace mode=command"
        );
        assert_eq!(EditorMode::Insert.toggle(), EditorMode::Normal);
        assert_eq!(EditorMode::Normal.toggle(), EditorMode::Insert);
        assert_eq!(EditorMode::Command.toggle(), EditorMode::Insert);
    }

    #[test]
    fn window_title_and_display_name_handle_dirty_and_empty_paths() {
        assert_eq!(
            file_display_name(Path::new("/docs/notes.md")),
            Some("notes.md")
        );
        assert_eq!(file_display_name(Path::new("/")), None);
        assert_eq!(
            window_title_for_state(Some(Path::new("/docs/README.md")), false),
            format!("README.md — {APP_TITLE}")
        );
        assert_eq!(
            window_title_for_state(Some(Path::new("/docs/README.md")), true),
            format!("• README.md — {APP_TITLE}")
        );
        assert_eq!(window_title_for_state(None, false), APP_TITLE);
        assert_eq!(window_title_for_state(None, true), format!("• {APP_TITLE}"));
    }

    #[test]
    fn expand_user_path_resolves_relative_against_base_dir() {
        assert_eq!(
            expand_user_path("notes.md", Some(Path::new("/tmp/docs"))),
            Path::new("/tmp/docs/notes.md")
        );
        assert_eq!(
            expand_user_path("/abs/notes.md", Some(Path::new("/tmp/docs"))),
            Path::new("/abs/notes.md")
        );
    }

    #[test]
    fn load_and_save_markdown_file_round_trip_and_report_errors() {
        let dir = std::env::temp_dir().join(format!("readmd-test-{}", std::process::id()));
        let nested = dir.join("nested").join("sample.md");

        save_markdown_file(&nested, "# Hello\n\nBody text.").expect("save markdown");
        assert_eq!(
            load_markdown_file(&nested).expect("read saved markdown"),
            "# Hello\n\nBody text."
        );

        let missing = dir.join("missing.md");
        let missing_err = load_markdown_file(&missing).expect_err("missing file must fail");
        assert!(missing_err.contains("missing.md"));

        let invalid_utf8 = dir.join("invalid.md");
        std::fs::write(&invalid_utf8, [0xff, 0xfe, 0x00]).expect("write invalid utf8");
        let utf8_err = load_markdown_file(&invalid_utf8).expect_err("invalid utf8 must fail");
        assert!(utf8_err.contains("invalid.md"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

use std::path::Path;

pub const APP_TITLE: &str = "readmd";

/// LazyVim-style modal editing mode for a Markdown document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditorMode {
    /// Plain-text Markdown editing mode (default when opening a blank session).
    #[default]
    Insert,
    /// Rendered Markdown reading mode.
    Normal,
}

impl EditorMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Insert => "INSERT",
            Self::Normal => "NORMAL",
        }
    }

    pub fn key_context(self) -> &'static str {
        match self {
            Self::Insert => "DocumentWorkspace mode=insert",
            Self::Normal => "DocumentWorkspace mode=normal",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Insert => Self::Normal,
            Self::Normal => Self::Insert,
        }
    }

    /// Loading a Markdown file always enters Normal mode to render it immediately.
    pub fn on_file_loaded() -> Self {
        Self::Normal
    }
}

pub fn load_markdown_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| format!("Failed to open {}: {err}", path.display()))
}

pub fn file_display_name(path: &Path) -> Option<&str> {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
}

pub fn window_title_for_path(path: &Path) -> String {
    match file_display_name(path) {
        Some(name) => format!("{name} — {APP_TITLE}"),
        None => APP_TITLE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        APP_TITLE, EditorMode, file_display_name, load_markdown_file, window_title_for_path,
    };
    use std::path::Path;

    #[test]
    fn default_mode_is_insert_and_file_load_enters_normal() {
        assert_eq!(EditorMode::default(), EditorMode::Insert);
        assert_eq!(EditorMode::on_file_loaded(), EditorMode::Normal);
    }

    #[test]
    fn mode_labels_contexts_and_toggle_match_modal_contract() {
        assert_eq!(EditorMode::Insert.label(), "INSERT");
        assert_eq!(EditorMode::Normal.label(), "NORMAL");
        assert_eq!(
            EditorMode::Insert.key_context(),
            "DocumentWorkspace mode=insert"
        );
        assert_eq!(
            EditorMode::Normal.key_context(),
            "DocumentWorkspace mode=normal"
        );
        assert_eq!(EditorMode::Insert.toggle(), EditorMode::Normal);
        assert_eq!(EditorMode::Normal.toggle(), EditorMode::Insert);
    }

    #[test]
    fn window_title_and_display_name_handle_valid_and_empty_paths() {
        assert_eq!(
            file_display_name(Path::new("/docs/notes.md")),
            Some("notes.md")
        );
        assert_eq!(file_display_name(Path::new("/")), None);
        assert_eq!(
            window_title_for_path(Path::new("/docs/README.md")),
            format!("README.md — {APP_TITLE}")
        );
        assert_eq!(window_title_for_path(Path::new("/")), APP_TITLE);
    }

    #[test]
    fn load_markdown_file_reads_valid_utf8_and_reports_errors() {
        let dir = std::env::temp_dir().join(format!("readmd-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let valid = dir.join("sample.md");
        std::fs::write(&valid, "# Hello\n\nBody text.").expect("write sample markdown");
        assert_eq!(
            load_markdown_file(&valid).expect("read valid markdown"),
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

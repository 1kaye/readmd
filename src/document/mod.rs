mod mode;
mod status_bar;
mod workspace;

use gpui_kit::{App, KeyBinding, actions};

pub use mode::{APP_TITLE, EditorMode};
pub use workspace::DocumentWorkspace;

const WORKSPACE_CONTEXT: &str = "DocumentWorkspace";
const NORMAL_PREDICATE: &str = "DocumentWorkspace && mode == normal";
const INSERT_PREDICATE: &str = "DocumentWorkspace && mode == insert";

actions!(readmd, [OpenFile, EnterInsertMode, EnterNormalMode]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-o", OpenFile, Some(WORKSPACE_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-o", OpenFile, Some(WORKSPACE_CONTEXT)),
        KeyBinding::new("i", EnterInsertMode, Some(NORMAL_PREDICATE)),
        KeyBinding::new("escape", EnterNormalMode, Some(INSERT_PREDICATE)),
    ]);
}

mod command;
mod command_bar;
mod mode;
mod scroll_sync;
mod status_bar;
mod workspace;

use gpui_kit::actions;

pub use mode::{APP_TITLE, EditorMode};
pub use workspace::DocumentWorkspace;

actions!(
    readmd,
    [
        NewFile,
        OpenFile,
        SaveFile,
        EnterInsertMode,
        EnterNormalMode,
        OpenCommand,
        ScrollDown,
        ScrollUp,
        CommandNextHint,
        CommandPrevHint,
        CommandCompleteHint,
    ]
);

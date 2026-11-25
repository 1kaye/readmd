mod command;
mod command_bar;
mod mode;
mod status_bar;
mod workspace;

use gpui_kit::actions;

pub use mode::{APP_TITLE, EditorMode};
pub use workspace::DocumentWorkspace;

actions!(
    readmd,
    [
        OpenFile,
        SaveFile,
        EnterInsertMode,
        EnterNormalMode,
        OpenCommand,
    ]
);

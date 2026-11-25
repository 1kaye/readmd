use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Pixels, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, base, div, point,
    prelude::FluentBuilder as _, rems,
};

use super::command::{Command, HintList, command_hints, parse_command, resolve_font_name};
use super::command_bar::CommandBar;
use super::mode::{
    EditorMode, expand_user_path, file_display_name, load_markdown_file, save_markdown_file,
    window_title_for_state,
};
use super::status_bar::{StatusBar, StatusNotice};
use super::{
    CommandCompleteHint, CommandNextHint, CommandPrevHint, EnterInsertMode, EnterNormalMode,
    OpenCommand, OpenFile, SaveFile,
};
use crate::{keymap, theme};

const NOTICE_TIMEOUT: Duration = Duration::from_secs(3);

/// Retained workspace owning the active Markdown buffer, modal state,
/// plain-text editor, rendered preview, top-center command line, font state,
/// auto-dismissing notices, file I/O tasks, and theme synchronization.
pub struct DocumentWorkspace {
    mode: EditorMode,
    path: Option<PathBuf>,
    saved_source: String,
    is_dirty: bool,
    notice: Option<StatusNotice>,
    notice_id: u64,
    _notice_timer: Option<Task<()>>,
    document_font: Option<SharedString>,
    available_fonts: Vec<SharedString>,
    selected_hint_ix: Option<usize>,
    editor: Entity<base::input::TextareaState>,
    preview: Entity<base::TextViewState>,
    command_input: Entity<base::input::InputState>,
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    io_in_flight: bool,
    request_id: u64,
    _io_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl DocumentWorkspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial_theme = base::Theme::global(cx);
        let editor_style = theme::editor_style(&initial_theme);

        let editor = cx.new(|cx| {
            let mut state = base::input::TextareaState::new(window, cx);
            state.set_editor_style(editor_style.clone());
            state
        });
        let command_input = cx.new(|cx| {
            let mut state = base::input::InputState::new(window, cx);
            state.set_editor_style(editor_style);
            state
        });
        let preview = cx.new(|cx| base::TextViewState::markdown("", cx));
        let focus_handle = cx.focus_handle();

        // Start in Insert mode with the plain-text editor focused immediately.
        editor.update(cx, |state, cx| state.focus(window, cx));

        let available_fonts = collect_system_fonts(cx);

        let weak_editor = editor.downgrade();
        let weak_command = command_input.downgrade();
        let appearance_sub = window.observe_window_appearance(move |window, cx| {
            theme::sync_with_appearance(window.appearance(), cx);
            let updated_style = theme::editor_style(&base::Theme::global(cx));
            let _ = weak_editor.update(cx, |state, _| {
                state.set_editor_style(updated_style.clone());
            });
            let _ = weak_command.update(cx, |state, _| {
                state.set_editor_style(updated_style);
            });
            window.refresh();
        });

        let editor_change_sub = cx.subscribe_in(
            &editor,
            window,
            |this, state, event: &base::input::InputEvent, window, cx| {
                if matches!(event, base::input::InputEvent::Change) {
                    let dirty_now = state.read(cx).value().as_ref() != this.saved_source.as_str();
                    let cleared_notice = this.clear_notice();
                    if dirty_now != this.is_dirty {
                        this.is_dirty = dirty_now;
                        this.update_window_title(window);
                        cx.notify();
                    } else if cleared_notice {
                        cx.notify();
                    }
                }
            },
        );

        let command_event_sub = cx.subscribe_in(
            &command_input,
            window,
            |this, _, event: &base::input::InputEvent, window, cx| match event {
                base::input::InputEvent::Change => {
                    this.selected_hint_ix = None;
                    cx.notify();
                }
                base::input::InputEvent::PressEnter { .. } => {
                    this.submit_command_line(window, cx);
                }
                _ => {}
            },
        );

        let startup_notice = keymap::take_startup_notice(cx);

        let mut this = Self {
            mode: EditorMode::default(),
            path: None,
            saved_source: String::new(),
            is_dirty: false,
            notice: None,
            notice_id: 0,
            _notice_timer: None,
            document_font: None,
            available_fonts,
            selected_hint_ix: None,
            editor,
            preview,
            command_input,
            focus_handle,
            scroll_handle: ScrollHandle::new(),
            io_in_flight: false,
            request_id: 0,
            _io_task: None,
            _subscriptions: vec![appearance_sub, editor_change_sub, command_event_sub],
        };

        if let Some(msg) = startup_notice {
            this.show_notice(StatusNotice::Error(SharedString::from(msg)), cx);
        }

        this
    }

    fn show_notice(&mut self, notice: StatusNotice, cx: &mut Context<Self>) {
        self.notice = Some(notice);
        self.notice_id = self.notice_id.wrapping_add(1);
        let id = self.notice_id;

        self._notice_timer = Some(cx.spawn(async move |weak_this, cx| {
            cx.background_executor().timer(NOTICE_TIMEOUT).await;
            _ = weak_this.update(cx, |this, cx| {
                if this.notice_id == id && this.notice.take().is_some() {
                    cx.notify();
                }
            });
        }));
    }

    fn clear_notice(&mut self) -> bool {
        self.notice_id = self.notice_id.wrapping_add(1);
        self._notice_timer = None;
        self.notice.take().is_some()
    }

    fn current_directory(&self) -> Option<PathBuf> {
        self.path
            .as_deref()
            .and_then(Path::parent)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf)
    }

    fn update_window_title(&self, window: &mut Window) {
        window.set_window_title(&window_title_for_state(self.path.as_deref(), self.is_dirty));
    }

    fn current_command_hints(&self, cx: &Context<Self>) -> HintList {
        let raw = self.command_input.read(cx).value();
        command_hints(&raw, &self.available_fonts, self.document_font.as_deref())
    }

    fn set_command_input_text(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value: SharedString = text.into();
        let utf16_end = value.as_ref().encode_utf16().count() as u32;
        self.selected_hint_ix = None;
        self.command_input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
            input.set_cursor_position(base::input::Position::new(0, utf16_end), window, cx);
        });
        cx.notify();
    }

    fn enter_insert_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == EditorMode::Insert {
            return;
        }
        if self.mode == EditorMode::Command {
            self.selected_hint_ix = None;
            self.command_input
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.mode = EditorMode::Insert;
        self.editor
            .update(cx, |editor, cx| editor.focus(window, cx));
        cx.notify();
    }

    fn enter_normal_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == EditorMode::Normal {
            return;
        }
        if self.mode == EditorMode::Command {
            self.selected_hint_ix = None;
            self.command_input
                .update(cx, |input, cx| input.set_value("", window, cx));
        } else {
            let source = self.editor.read(cx).value();
            self.preview
                .update(cx, |preview, cx| preview.set_text(&source, cx));
        }
        self.mode = EditorMode::Normal;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn open_command_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode != EditorMode::Normal {
            return;
        }
        self.clear_notice();
        self.selected_hint_ix = None;
        self.mode = EditorMode::Command;
        self.command_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn close_command_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode != EditorMode::Command {
            return;
        }
        self.selected_hint_ix = None;
        self.command_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.mode = EditorMode::Normal;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn on_command_next_hint(
        &mut self,
        _: &CommandNextHint,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode != EditorMode::Command {
            return;
        }
        let len = self.current_command_hints(cx).items.len();
        if len == 0 {
            return;
        }
        self.selected_hint_ix = Some(match self.selected_hint_ix {
            Some(ix) => (ix + 1) % len,
            None => 0,
        });
        cx.notify();
    }

    fn on_command_prev_hint(
        &mut self,
        _: &CommandPrevHint,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode != EditorMode::Command {
            return;
        }
        let len = self.current_command_hints(cx).items.len();
        if len == 0 {
            return;
        }
        self.selected_hint_ix = Some(match self.selected_hint_ix {
            Some(0) | None => len - 1,
            Some(ix) => ix - 1,
        });
        cx.notify();
    }

    fn on_command_complete_hint(
        &mut self,
        _: &CommandCompleteHint,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode != EditorMode::Command {
            return;
        }
        let hints = self.current_command_hints(cx);
        let ix = self.selected_hint_ix.unwrap_or(0);
        if let Some(hint) = hints.items.get(ix) {
            self.set_command_input_text(hint.insert_text.clone(), window, cx);
        }
    }

    fn select_hint(
        &mut self,
        insert_text: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if insert_text.ends_with(' ') {
            self.set_command_input_text(insert_text, window, cx);
            return;
        }
        self.selected_hint_ix = None;
        self.command_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.mode = EditorMode::Normal;
        self.focus_handle.focus(window, cx);
        self.execute_command_text(&insert_text, window, cx);
    }

    fn submit_command_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode != EditorMode::Command {
            return;
        }

        // If the user explicitly highlighted a hint with Up/Down, execute or expand it.
        if let Some(ix) = self.selected_hint_ix {
            let hints = self.current_command_hints(cx);
            if let Some(hint) = hints.items.get(ix) {
                let insert_text = hint.insert_text.clone();
                self.select_hint(insert_text, window, cx);
                return;
            }
        }

        let raw = self.command_input.read(cx).value();

        // `:Font` with no argument expands to `Font ` so the user can pick from system fonts.
        if matches!(parse_command(&raw), Ok(Command::SetFont { name: None })) {
            self.set_command_input_text("Font ", window, cx);
            return;
        }

        self.selected_hint_ix = None;
        self.command_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.mode = EditorMode::Normal;
        self.focus_handle.focus(window, cx);

        self.execute_command_text(&raw, window, cx);
    }

    fn execute_command_text(&mut self, raw: &str, window: &mut Window, cx: &mut Context<Self>) {
        match parse_command(raw) {
            Ok(Command::Noop) => cx.notify(),
            Ok(Command::Write { path }) => {
                let target = path.map(|raw_path| {
                    expand_user_path(&raw_path, self.current_directory().as_deref())
                });
                self.save_document(target, false, window, cx);
            }
            Ok(Command::WriteQuit { path }) => {
                let target = path.map(|raw_path| {
                    expand_user_path(&raw_path, self.current_directory().as_deref())
                });
                self.save_document(target, true, window, cx);
            }
            Ok(Command::Quit { force }) => {
                if !force && self.is_dirty {
                    self.show_notice(
                        StatusNotice::Error(SharedString::from(
                            "No write since last change (use :q! to override)",
                        )),
                        cx,
                    );
                    cx.notify();
                } else {
                    window.remove_window();
                }
            }
            Ok(Command::Open { path: None }) => {
                self.open_file_dialog(window, cx);
            }
            Ok(Command::Open {
                path: Some(raw_path),
            }) => {
                let target = expand_user_path(&raw_path, self.current_directory().as_deref());
                self.load_file_from_path(target, EditorMode::on_file_loaded(), window, cx);
            }
            Ok(Command::SetFont { name: Some(name) }) => {
                match resolve_font_name(&name, &self.available_fonts) {
                    Ok(resolved) => {
                        let label = match &resolved {
                            Some(family) => format!("Font set to {family}"),
                            None => "Font reset to default".to_string(),
                        };
                        self.document_font = resolved;
                        self.show_notice(StatusNotice::Info(SharedString::from(label)), cx);
                        window.refresh();
                        cx.notify();
                    }
                    Err(err) => {
                        self.show_notice(StatusNotice::Error(err.into()), cx);
                        cx.notify();
                    }
                }
            }
            Ok(Command::SetFont { name: None }) => cx.notify(),
            Err(err) => {
                self.show_notice(StatusNotice::Error(err.into()), cx);
                cx.notify();
            }
        }
    }

    fn toggle_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.mode.toggle() {
            EditorMode::Insert => self.enter_insert_mode(window, cx),
            EditorMode::Normal | EditorMode::Command => self.enter_normal_mode(window, cx),
        }
    }

    fn focus_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editor.read(cx).focus_handle(cx).is_focused(window) {
            self.editor
                .update(cx, |editor, cx| editor.focus(window, cx));
        }
    }

    fn on_enter_insert_mode(
        &mut self,
        _: &EnterInsertMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.enter_insert_mode(window, cx);
    }

    fn on_enter_normal_mode(
        &mut self,
        _: &EnterNormalMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.enter_normal_mode(window, cx);
    }

    fn on_open_command(&mut self, _: &OpenCommand, window: &mut Window, cx: &mut Context<Self>) {
        self.open_command_line(window, cx);
    }

    /// Catches `base::input::Escape` propagated from `InputState` / `TextareaState`.
    fn on_input_escape(
        &mut self,
        _: &base::input::Escape,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.mode {
            EditorMode::Command => self.close_command_line(window, cx),
            EditorMode::Insert if keymap::uses_escape_for_normal(cx) => {
                self.enter_normal_mode(window, cx);
            }
            _ => cx.propagate(),
        }
    }

    fn on_open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        self.open_file_dialog(window, cx);
    }

    fn on_save_file(&mut self, _: &SaveFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == EditorMode::Command {
            self.close_command_line(window, cx);
        }
        self.save_document(None, false, window, cx);
    }

    fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.io_in_flight {
            return;
        }

        self.io_in_flight = true;
        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;
        let start_dir = self.current_directory();

        let task = cx.spawn_in(window, async move |weak_this, cx| {
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title("Open Markdown")
                .add_filter("Markdown", &["md", "markdown", "mkd"]);
            if let Some(dir) = start_dir {
                dialog = dialog.set_directory(dir);
            }

            let picked = dialog.pick_file().await;
            let Some(handle) = picked else {
                _ = weak_this.update_in(cx, |this, _, _| {
                    if this.request_id == request_id {
                        this.io_in_flight = false;
                    }
                });
                return;
            };

            let path = handle.path().to_path_buf();
            let read_result = cx
                .background_executor()
                .spawn({
                    let path = path.clone();
                    async move { load_markdown_file(&path) }
                })
                .await;

            _ = weak_this.update_in(cx, |this, window, cx| {
                if this.request_id != request_id {
                    return;
                }
                this.io_in_flight = false;
                this.apply_loaded_file(path, read_result, EditorMode::on_file_loaded(), window, cx);
            });
        });

        self._io_task = Some(task);
    }

    fn load_file_from_path(
        &mut self,
        path: PathBuf,
        target_mode: EditorMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.io_in_flight {
            return;
        }

        self.io_in_flight = true;
        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;

        let task = cx.spawn_in(window, async move |weak_this, cx| {
            let read_result = cx
                .background_executor()
                .spawn({
                    let path = path.clone();
                    async move { load_markdown_file(&path) }
                })
                .await;

            _ = weak_this.update_in(cx, |this, window, cx| {
                if this.request_id != request_id {
                    return;
                }
                this.io_in_flight = false;
                this.apply_loaded_file(path, read_result, target_mode, window, cx);
            });
        });

        self._io_task = Some(task);
    }

    fn save_document(
        &mut self,
        explicit_path: Option<PathBuf>,
        quit_after_save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.io_in_flight {
            return;
        }

        let target_path = explicit_path.or_else(|| self.path.clone());
        let start_dir = self.current_directory();
        let content = self.editor.read(cx).value().to_string();

        self.io_in_flight = true;
        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;

        let task = cx.spawn_in(window, async move |weak_this, cx| {
            let resolved_path = match target_path {
                Some(path) => path,
                None => {
                    let mut dialog = rfd::AsyncFileDialog::new()
                        .set_title("Save Markdown")
                        .set_file_name("untitled.md")
                        .add_filter("Markdown", &["md", "markdown", "mkd"]);
                    if let Some(dir) = start_dir {
                        dialog = dialog.set_directory(dir);
                    }
                    let Some(handle) = dialog.save_file().await else {
                        _ = weak_this.update_in(cx, |this, _, _| {
                            if this.request_id == request_id {
                                this.io_in_flight = false;
                            }
                        });
                        return;
                    };
                    handle.path().to_path_buf()
                }
            };

            let write_result = cx
                .background_executor()
                .spawn({
                    let path = resolved_path.clone();
                    let content = content.clone();
                    async move { save_markdown_file(&path, &content) }
                })
                .await;

            _ = weak_this.update_in(cx, |this, window, cx| {
                if this.request_id != request_id {
                    return;
                }
                this.io_in_flight = false;
                this.apply_saved_file(
                    resolved_path,
                    content,
                    write_result,
                    quit_after_save,
                    window,
                    cx,
                );
            });
        });

        self._io_task = Some(task);
    }

    fn apply_loaded_file(
        &mut self,
        path: PathBuf,
        result: Result<String, String>,
        target_mode: EditorMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(source) => {
                self.editor.update(cx, |editor, cx| {
                    editor.set_value(source.clone(), window, cx);
                });
                self.preview.update(cx, |preview, cx| {
                    preview.set_text(&source, cx);
                });
                self.saved_source = source;
                self.is_dirty = false;
                self.path = Some(path);
                self.clear_notice();
                self.mode = target_mode;
                self.scroll_handle
                    .set_offset(point(Pixels::ZERO, Pixels::ZERO));
                self.update_window_title(window);
                match target_mode {
                    EditorMode::Insert => {
                        self.editor
                            .update(cx, |editor, cx| editor.focus(window, cx));
                    }
                    EditorMode::Normal | EditorMode::Command => {
                        self.focus_handle.focus(window, cx);
                    }
                }
            }
            Err(message) => {
                // Keep existing buffer intact while surfacing the load failure in the status bar.
                self.show_notice(StatusNotice::Error(message.into()), cx);
            }
        }
        cx.notify();
    }

    fn apply_saved_file(
        &mut self,
        path: PathBuf,
        saved_content: String,
        result: Result<(), String>,
        quit_after_save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(()) => {
                self.saved_source = saved_content;
                self.is_dirty = self.editor.read(cx).value().as_ref() != self.saved_source.as_str();
                self.path = Some(path.clone());
                self.update_window_title(window);

                let display = file_display_name(&path).unwrap_or("file");
                let mut reload_failed = false;

                if keymap::is_config_path(&path, cx) {
                    match keymap::reload(cx) {
                        Ok(()) => {
                            self.show_notice(
                                StatusNotice::Info(SharedString::from(format!(
                                    "Saved {display} (keymaps reloaded)"
                                ))),
                                cx,
                            );
                        }
                        Err(err) => {
                            reload_failed = true;
                            self.show_notice(
                                StatusNotice::Error(SharedString::from(format!(
                                    "Saved {display}, keymap error: {err}"
                                ))),
                                cx,
                            );
                        }
                    }
                } else {
                    self.show_notice(
                        StatusNotice::Info(SharedString::from(format!("Saved {display}"))),
                        cx,
                    );
                }

                if quit_after_save && !reload_failed {
                    window.remove_window();
                    return;
                }
            }
            Err(message) => {
                self.show_notice(StatusNotice::Error(message.into()), cx);
            }
        }
        cx.notify();
    }

    fn render_editor_pane(&self, theme: &base::Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let editor_font = self
            .document_font
            .clone()
            .unwrap_or_else(|| theme.tokens.typography.mono.clone());

        div()
            .id("document-editor-pane")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .size_full()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.focus_editor(window, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .max_w(rems(52.))
                    .mx_auto()
                    .px_8()
                    .pt_8()
                    .pb_6()
                    .font_family(editor_font)
                    .text_sm()
                    .child(base::input::Textarea::new(&self.editor)),
            )
    }

    fn render_preview_pane(&self, theme: &base::Theme) -> impl IntoElement {
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .size_full()
            .child(
                div()
                    .id("document-preview-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(
                        div()
                            .w_full()
                            .max_w(rems(52.))
                            .min_w_0()
                            .mx_auto()
                            .px_8()
                            .py_8()
                            .when_some(self.document_font.clone(), |this, font| {
                                this.font_family(font)
                            })
                            .child(
                                base::TextView::new(&self.preview)
                                    .style(theme::document_style(theme)),
                            ),
                    ),
            )
            .child(
                div().absolute().inset_0().child(
                    base::Scrollbar::vertical(&self.scroll_handle)
                        .id("document-preview-scrollbar")
                        .viewport_from_layout(),
                ),
            )
    }
}

fn collect_system_fonts(cx: &Context<DocumentWorkspace>) -> Vec<SharedString> {
    let mut names = cx.text_system().all_font_names();
    names.sort_by_key(|a| a.to_ascii_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names.into_iter().map(SharedString::from).collect()
}

impl Render for DocumentWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = base::Theme::global(cx);
        let colors = theme.tokens.colors;
        let file_name = self
            .path
            .as_deref()
            .and_then(file_display_name)
            .map(|name| SharedString::from(name.to_string()));

        let content = match self.mode {
            EditorMode::Insert => self.render_editor_pane(&theme, cx).into_any_element(),
            EditorMode::Normal | EditorMode::Command => {
                self.render_preview_pane(&theme).into_any_element()
            }
        };

        let status_bar = StatusBar::new(self.mode)
            .file_name(file_name)
            .dirty(self.is_dirty)
            .notice(self.notice.clone())
            .on_toggle_mode(cx.listener(|this, _, window, cx| this.toggle_mode(window, cx)));

        let command_hints = (self.mode == EditorMode::Command).then(|| {
            let raw = self.command_input.read(cx).value();
            command_hints(&raw, &self.available_fonts, self.document_font.as_deref())
        });

        div()
            .id("document-workspace")
            .key_context(self.mode.key_context())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_open_file))
            .on_action(cx.listener(Self::on_save_file))
            .on_action(cx.listener(Self::on_enter_insert_mode))
            .on_action(cx.listener(Self::on_enter_normal_mode))
            .on_action(cx.listener(Self::on_open_command))
            .on_action(cx.listener(Self::on_command_next_hint))
            .on_action(cx.listener(Self::on_command_prev_hint))
            .on_action(cx.listener(Self::on_command_complete_hint))
            .on_action(cx.listener(Self::on_input_escape))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .text_color(colors.foreground)
            .child(content)
            .child(status_bar)
            .when_some(command_hints, |this, hints| {
                let entity = cx.entity().downgrade();
                this.child(
                    CommandBar::new(&self.command_input, hints)
                        .selected_ix(self.selected_hint_ix)
                        .on_dismiss(cx.listener(|this, _, window, cx| {
                            this.close_command_line(window, cx);
                        }))
                        .on_select_hint(move |insert_text, window, cx| {
                            _ = entity.update(cx, |this, cx| {
                                this.select_hint(insert_text, window, cx);
                            });
                        }),
                )
            })
    }
}

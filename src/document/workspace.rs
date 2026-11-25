use std::path::{Path, PathBuf};

use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Pixels, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, base, div, point,
    rems,
};

use super::mode::{
    APP_TITLE, EditorMode, file_display_name, load_markdown_file, window_title_for_path,
};
use super::status_bar::StatusBar;
use super::{EnterInsertMode, EnterNormalMode, OpenFile};
use crate::theme;

/// Retained workspace owning the active Markdown buffer, modal state,
/// plain-text editor, rendered preview, file picker task, and theme sync.
pub struct DocumentWorkspace {
    mode: EditorMode,
    path: Option<PathBuf>,
    error_message: Option<SharedString>,
    editor: Entity<base::input::TextareaState>,
    preview: Entity<base::TextViewState>,
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    is_picking: bool,
    request_id: u64,
    _open_task: Option<Task<()>>,
    _appearance_subscription: Subscription,
}

impl DocumentWorkspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial_theme = base::Theme::global(cx);
        let editor = cx.new(|cx| {
            let mut state = base::input::TextareaState::new(window, cx);
            state.set_editor_style(theme::editor_style(&initial_theme));
            state
        });
        let preview = cx.new(|cx| base::TextViewState::markdown("", cx));
        let focus_handle = cx.focus_handle();

        // Start in Insert mode with the plain-text editor focused immediately.
        editor.update(cx, |state, cx| state.focus(window, cx));

        let weak_editor = editor.downgrade();
        let appearance_subscription = window.observe_window_appearance(move |window, cx| {
            theme::sync_with_appearance(window.appearance(), cx);
            let updated_theme = base::Theme::global(cx);
            let _ = weak_editor.update(cx, |state, _| {
                state.set_editor_style(theme::editor_style(&updated_theme));
            });
            window.refresh();
        });

        Self {
            mode: EditorMode::default(),
            path: None,
            error_message: None,
            editor,
            preview,
            focus_handle,
            scroll_handle: ScrollHandle::new(),
            is_picking: false,
            request_id: 0,
            _open_task: None,
            _appearance_subscription: appearance_subscription,
        }
    }

    fn current_directory(&self) -> Option<PathBuf> {
        self.path
            .as_deref()
            .and_then(Path::parent)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf)
    }

    fn enter_insert_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == EditorMode::Insert {
            return;
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
        let source = self.editor.read(cx).value();
        self.preview
            .update(cx, |preview, cx| preview.set_text(&source, cx));
        self.mode = EditorMode::Normal;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn toggle_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.mode.toggle() {
            EditorMode::Insert => self.enter_insert_mode(window, cx),
            EditorMode::Normal => self.enter_normal_mode(window, cx),
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

    /// Catches `base::input::Escape` propagated from `TextareaState` while in Insert mode.
    fn on_input_escape(
        &mut self,
        _: &base::input::Escape,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode == EditorMode::Insert {
            self.enter_normal_mode(window, cx);
        } else {
            cx.propagate();
        }
    }

    fn on_open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        self.open_file_dialog(window, cx);
    }

    fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_picking {
            return;
        }

        self.is_picking = true;
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
                        this.is_picking = false;
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
                this.is_picking = false;
                this.apply_loaded_file(path, read_result, window, cx);
            });
        });

        self._open_task = Some(task);
    }

    fn apply_loaded_file(
        &mut self,
        path: PathBuf,
        result: Result<String, String>,
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
                self.path = Some(path.clone());
                self.error_message = None;
                self.mode = EditorMode::on_file_loaded();
                self.scroll_handle
                    .set_offset(point(Pixels::ZERO, Pixels::ZERO));
                window.set_window_title(&window_title_for_path(&path));
                self.focus_handle.focus(window, cx);
            }
            Err(message) => {
                // Keep existing buffer intact while surfacing the load failure in the status bar.
                self.error_message = Some(message.into());
                if self.path.is_none() {
                    window.set_window_title(APP_TITLE);
                }
            }
        }
        cx.notify();
    }

    fn render_editor_pane(&self, theme: &base::Theme, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .font_family(theme.tokens.typography.mono.clone())
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
            EditorMode::Normal => self.render_preview_pane(&theme).into_any_element(),
        };

        let status_bar = StatusBar::new(self.mode)
            .file_name(file_name)
            .error_message(self.error_message.clone())
            .on_toggle_mode(cx.listener(|this, _, window, cx| this.toggle_mode(window, cx)));

        div()
            .id("document-workspace")
            .key_context(self.mode.key_context())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_open_file))
            .on_action(cx.listener(Self::on_enter_insert_mode))
            .on_action(cx.listener(Self::on_enter_normal_mode))
            .on_action(cx.listener(Self::on_input_escape))
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .text_color(colors.foreground)
            .child(content)
            .child(status_bar)
    }
}

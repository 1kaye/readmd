use std::path::{Path, PathBuf};

use gpui_kit::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyBinding, Overflow, ParentElement as _, Pixels, Render, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled as _, Subscription,
    Task, Window, WindowAppearance, WindowBounds, WindowOptions, actions, base, div, point, px,
    rems, size,
};

const APP_TITLE: &str = "readmd";
const KEY_CONTEXT: &str = "ReadMd";

// Physical window sizing exceptions (platform window geometry, not UI layout).
const INITIAL_WINDOW_WIDTH: Pixels = px(960.);
const INITIAL_WINDOW_HEIGHT: Pixels = px(720.);
const MIN_WINDOW_WIDTH: Pixels = px(360.);
const MIN_WINDOW_HEIGHT: Pixels = px(240.);

actions!(readmd, [OpenFile]);

enum DocumentState {
    Blank,
    Loaded {
        path: PathBuf,
        text_view: Entity<base::TextViewState>,
    },
    Error {
        path: Option<PathBuf>,
        message: SharedString,
    },
}

struct ReadMdView {
    document: DocumentState,
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    is_picking: bool,
    request_id: u64,
    _open_task: Option<Task<()>>,
    _appearance_subscription: Subscription,
}

impl ReadMdView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        let appearance_subscription = window.observe_window_appearance(|window, cx| {
            sync_theme_with_appearance(window.appearance(), cx);
            window.refresh();
        });

        Self {
            document: DocumentState::Blank,
            focus_handle,
            scroll_handle: ScrollHandle::new(),
            is_picking: false,
            request_id: 0,
            _open_task: None,
            _appearance_subscription: appearance_subscription,
        }
    }

    fn current_directory(&self) -> Option<PathBuf> {
        let path = match &self.document {
            DocumentState::Loaded { path, .. } => Some(path.as_path()),
            DocumentState::Error { path, .. } => path.as_deref(),
            DocumentState::Blank => None,
        }?;
        path.parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf)
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
                let text_view = cx.new(|cx| base::TextViewState::markdown(&source, cx));
                self.document = DocumentState::Loaded {
                    path: path.clone(),
                    text_view,
                };
                self.scroll_handle
                    .set_offset(point(Pixels::ZERO, Pixels::ZERO));
                window.set_window_title(&window_title_for_path(&path));
            }
            Err(message) => {
                self.document = DocumentState::Error {
                    path: Some(path),
                    message: message.into(),
                };
                window.set_window_title(APP_TITLE);
            }
        }
        self.focus_handle.focus(window, cx);
        cx.notify();
    }
}

impl Render for ReadMdView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = base::Theme::global(cx);
        let colors = theme.tokens.colors;

        let body = match &self.document {
            DocumentState::Blank => div()
                .id("readmd-blank")
                .flex_1()
                .size_full()
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_file_dialog(window, cx);
                }))
                .into_any_element(),

            DocumentState::Loaded { text_view, .. } => div()
                .relative()
                .flex_1()
                .min_h_0()
                .size_full()
                .child(
                    div()
                        .id("readmd-scroll")
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
                                .py_10()
                                .child(
                                    base::TextView::new(text_view).style(document_style(&theme)),
                                ),
                        ),
                )
                .child(
                    div().absolute().inset_0().child(
                        base::Scrollbar::vertical(&self.scroll_handle)
                            .id("readmd-scrollbar")
                            .viewport_from_layout(),
                    ),
                )
                .into_any_element(),

            DocumentState::Error { message, .. } => div()
                .id("readmd-error")
                .flex_1()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_8()
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_file_dialog(window, cx);
                }))
                .child(
                    div()
                        .max_w(rems(36.))
                        .text_sm()
                        .text_color(colors.destructive)
                        .child(message.clone()),
                )
                .into_any_element(),
        };

        div()
            .id("readmd-surface")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_open_file))
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .text_color(colors.foreground)
            .child(body)
    }
}

fn is_dark_appearance(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

fn sync_theme_with_appearance(appearance: WindowAppearance, cx: &mut App) {
    let is_dark = is_dark_appearance(appearance);
    let theme = base::Theme::global_mut(cx);
    theme.appearance = if is_dark {
        base::ThemeAppearance::Dark
    } else {
        base::ThemeAppearance::Light
    };
    theme.tokens.colors = if is_dark {
        base::ColorTokens::dark()
    } else {
        base::ColorTokens::light()
    };
}

fn document_style(theme: &base::Theme) -> base::TextViewStyle {
    let mut table = StyleRefinement::default();
    table.overflow.x = Some(Overflow::Scroll);
    base::TextViewStyle::from_theme(theme)
        .with_table(table)
        .with_table_row_hover(theme.tokens.colors.muted)
}

fn load_markdown_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| format!("Failed to open {}: {err}", path.display()))
}

fn window_title_for_path(path: &Path) -> String {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(name) if !name.is_empty() => format!("{name} — {APP_TITLE}"),
        _ => APP_TITLE.to_string(),
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);

        cx.bind_keys([
            KeyBinding::new("ctrl-o", OpenFile, Some(KEY_CONTEXT)),
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-o", OpenFile, Some(KEY_CONTEXT)),
        ]);

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT),
                cx,
            ))),
            window_min_size: Some(size(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)),
            app_id: Some(APP_TITLE.to_string()),
            ..WindowOptions::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            window.set_window_title(APP_TITLE);
            sync_theme_with_appearance(window.appearance(), cx);
            cx.new(|cx| ReadMdView::new(window, cx))
        })
        .expect("Failed to open readmd window");
    });
}

#[cfg(test)]
mod tests {
    use super::{APP_TITLE, is_dark_appearance, load_markdown_file, window_title_for_path};
    use gpui_kit::WindowAppearance;
    use std::path::Path;

    #[test]
    fn dark_appearance_maps_all_variants() {
        assert!(!is_dark_appearance(WindowAppearance::Light));
        assert!(!is_dark_appearance(WindowAppearance::VibrantLight));
        assert!(is_dark_appearance(WindowAppearance::Dark));
        assert!(is_dark_appearance(WindowAppearance::VibrantDark));
    }

    #[test]
    fn window_title_formats_file_name_with_fallback() {
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

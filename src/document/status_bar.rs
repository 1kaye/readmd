use std::rc::Rc;

use gpui_kit::{
    App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    base::{self, StyledExt as _},
    div,
    prelude::FluentBuilder as _,
};

use super::EditorMode;
use crate::theme;

type ToggleModeHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusNotice {
    Info(SharedString),
    Error(SharedString),
}

/// Dead-simple LazyVim-style bottom status bar:
/// mode box (`INSERT` / `NORMAL` / `COMMAND`) anchored bottom-left, subtle file/dirty/status
/// beside it, and reserved open space on the right for future settings.
#[derive(IntoElement)]
pub struct StatusBar {
    mode: EditorMode,
    file_name: Option<SharedString>,
    is_dirty: bool,
    notice: Option<StatusNotice>,
    on_toggle_mode: Option<ToggleModeHandler>,
}

impl StatusBar {
    pub fn new(mode: EditorMode) -> Self {
        Self {
            mode,
            file_name: None,
            is_dirty: false,
            notice: None,
            on_toggle_mode: None,
        }
    }

    pub fn file_name(mut self, file_name: Option<SharedString>) -> Self {
        self.file_name = file_name;
        self
    }

    pub fn dirty(mut self, is_dirty: bool) -> Self {
        self.is_dirty = is_dirty;
        self
    }

    pub fn notice(mut self, notice: Option<StatusNotice>) -> Self {
        self.notice = notice;
        self
    }

    pub fn on_toggle_mode(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_mode = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = base::Theme::global(cx);
        let colors = theme.tokens.colors;
        let badge = theme::mode_badge_colors(&theme, self.mode);
        let mono = theme.tokens.typography.mono.clone();

        let file_label = match (self.file_name, self.is_dirty) {
            (Some(name), true) => Some(SharedString::from(format!("{name} [+]"))),
            (Some(name), false) => Some(name),
            (None, true) => Some(SharedString::from("[No Name] [+]")),
            (None, false) => None,
        };

        div()
            .id("document-status-bar")
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .w_full()
            .bg(colors.surface)
            .border_t_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .min_w_0()
                    .flex_1()
                    .gap_3()
                    .child(
                        div()
                            .id("status-bar-mode")
                            .flex_shrink_0()
                            .px_3()
                            .py_1()
                            .bg(badge.background)
                            .text_color(badge.foreground)
                            .font_family(mono)
                            .text_xs()
                            .font_bold()
                            .when_some(self.on_toggle_mode, |this, handler| {
                                this.on_click(move |event, window, cx| handler(event, window, cx))
                            })
                            .child(self.mode.label()),
                    )
                    .when_some(file_label, |this, label| {
                        this.child(
                            div()
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(label),
                        )
                    })
                    .when_some(self.notice, |this, notice| {
                        let (text, color) = match notice {
                            StatusNotice::Info(msg) => (msg, colors.muted_foreground),
                            StatusNotice::Error(msg) => (msg, colors.destructive),
                        };
                        this.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(color)
                                .child(text),
                        )
                    }),
            )
            // Reserved right-side slot for future settings controls.
            .child(
                div()
                    .id("status-bar-right")
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_shrink_0()
                    .gap_2()
                    .px_3(),
            )
    }
}

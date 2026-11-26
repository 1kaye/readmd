use std::rc::Rc;

use gpui_kit::{
    App, ClickEvent, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    base::{self, StyledExt as _},
    canvas, div, point,
    prelude::FluentBuilder as _,
    relative, rems,
};

use super::EditorMode;
use crate::theme;

type ToggleModeHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusNotice {
    Info(SharedString),
    Error(SharedString),
}

/// and transient status/error notices right-aligned on the right.
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

fn mode_ribbon_tip(color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let mut builder = PathBuilder::fill();
            builder.move_to(bounds.origin);
            builder.line_to(point(
                bounds.right(),
                bounds.origin.y + bounds.size.height / 2.0,
            ));
            builder.line_to(point(bounds.origin.x, bounds.bottom()));
            builder.close();
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        },
    )
    .w(rems(0.45))
    .h_full()
    .flex_shrink_0()
}

impl RenderOnce for StatusBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = base::Theme::global(cx);
        let colors = theme.tokens.colors;
        let badge = theme::mode_badge_colors(&theme, self.mode);

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
            .h(rems(1.375))
            .gap_3()
            .bg(colors.surface)
            .border_t_1()
            .border_color(colors.border)
            .font_family(theme::APP_UI_FONT)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .gap_2p5()
                    .child(
                        div()
                            .id("status-bar-mode")
                            .flex()
                            .flex_row()
                            .items_stretch()
                            .h_full()
                            .flex_shrink_0()
                            .when_some(self.on_toggle_mode, |this, handler| {
                                this.on_click(move |event, window, cx| handler(event, window, cx))
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .h_full()
                                    .pl_2p5()
                                    .pr_1p5()
                                    .bg(badge.background)
                                    .text_color(badge.foreground)
                                    .text_size(rems(0.6875))
                                    .line_height(relative(1.0))
                                    .font_bold()
                                    .child(self.mode.label()),
                            )
                            .child(mode_ribbon_tip(badge.background)),
                    )
                    .when_some(file_label, |this, label| {
                        this.child(
                            div()
                                .flex_shrink_0()
                                .text_size(rems(0.6875))
                                .line_height(relative(1.0))
                                .text_color(colors.muted_foreground)
                                .child(label),
                        )
                    }),
            )
            // Right-side status area: displays transient saved/font/error notices.
            .child(
                div()
                    .id("status-bar-right")
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .h_full()
                    .min_w_0()
                    .flex_1()
                    .gap_2()
                    .px_2p5()
                    .when_some(self.notice, |this, notice| {
                        let (text, color) = match notice {
                            StatusNotice::Info(msg) => (msg, colors.muted_foreground),
                            StatusNotice::Error(msg) => (msg, colors.destructive),
                        };
                        this.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(rems(0.6875))
                                .line_height(relative(1.0))
                                .text_color(color)
                                .child(text),
                        )
                    }),
            )
    }
}

use std::rc::Rc;

use gpui_kit::{
    App, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement as _,
    RenderOnce, Styled as _, Window,
    base::{self, StyledExt as _},
    div,
    prelude::FluentBuilder as _,
    rems,
};

use crate::theme;

type DismissHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>;

const SHORTCUTS: &[(&str, &str)] = &[
    ("i", "Enter Insert mode (edit Markdown)"),
    ("Esc", "Return to Normal mode / close overlay"),
    (":", "Open command bar (:w, :q, :e, :n, :Font, :Theme)"),
    ("j / k  or  ↓ / ↑", "Scroll document in Normal mode"),
    ("Ctrl+N", "Create a new blank file"),
    ("Ctrl+O", "Open a Markdown file"),
    ("Ctrl+S", "Save current file"),
    ("Ctrl+Q", "Quit (guards unsaved changes)"),
    ("?", "Toggle this shortcuts overlay"),
];

/// Modal keyboard-shortcuts overlay rendered in Normal mode when `?` is pressed.
#[derive(IntoElement)]
pub struct HelpOverlay {
    on_dismiss: Option<DismissHandler>,
}

impl HelpOverlay {
    pub fn new() -> Self {
        Self { on_dismiss: None }
    }

    pub fn on_dismiss(
        mut self,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for HelpOverlay {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = base::Theme::global(cx);
        let colors = theme.tokens.colors;

        div()
            .id("help-overlay-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .bg(colors.background.alpha(0.65))
            .when_some(self.on_dismiss, |this, handler| {
                this.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    handler(event, window, cx);
                })
            })
            .child(
                div()
                    .id("help-overlay-card")
                    .w_full()
                    .max_w(rems(32.))
                    .flex()
                    .flex_col()
                    .bg(colors.surface)
                    .text_color(colors.foreground)
                    .border_1()
                    .border_color(colors.border)
                    .shadow(theme.tokens.shadow.md.clone())
                    .font_family(theme::APP_UI_FONT)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_2p5()
                            .border_b_1()
                            .border_color(colors.border)
                            .child(div().text_sm().font_bold().child("Keyboard Shortcuts"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .child("Esc / ? to close"),
                            ),
                    )
                    .child(div().flex().flex_col().py_2().children(
                        SHORTCUTS.iter().enumerate().map(|(ix, (key, desc))| {
                            div()
                                .id(("help-shortcut-row", ix))
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .px_4()
                                .py_1()
                                .text_xs()
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .px_1p5()
                                        .py_0p5()
                                        .bg(colors.muted)
                                        .border_1()
                                        .border_color(colors.border)
                                        .font_bold()
                                        .text_color(colors.primary)
                                        .child(*key),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_color(colors.muted_foreground)
                                        .child(*desc),
                                )
                        }),
                    )),
            )
    }
}

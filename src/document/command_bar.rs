use std::rc::Rc;

use gpui_kit::{
    App, Entity, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, RenderOnce, Styled as _, Window,
    base::{self, StyledExt as _},
    div,
    prelude::FluentBuilder as _,
    rems,
};

use super::EditorMode;
use crate::theme;

type DismissHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>;

/// Dead-simple top-center LazyVim-style command-line input (`:w`, `:wq`, `:q`, `:keymap`, ...).
#[derive(IntoElement)]
pub struct CommandBar {
    input: Entity<base::input::InputState>,
    on_dismiss: Option<DismissHandler>,
}

impl CommandBar {
    pub fn new(input: &Entity<base::input::InputState>) -> Self {
        Self {
            input: input.clone(),
            on_dismiss: None,
        }
    }

    pub fn on_dismiss(
        mut self,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CommandBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = base::Theme::global(cx);
        let colors = theme.tokens.colors;
        let badge = theme::mode_badge_colors(&theme, EditorMode::Command);
        let mono = theme.tokens.typography.mono.clone();

        div()
            .id("command-bar-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_start()
            .pt_12()
            .px_4()
            .when_some(self.on_dismiss, |this, handler| {
                this.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    handler(event, window, cx);
                })
            })
            .child(
                div()
                    .id("command-bar-box")
                    .w_full()
                    .max_w(rems(32.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .bg(colors.surface)
                    .text_color(colors.foreground)
                    .border_1()
                    .border_color(colors.ring)
                    .rounded(theme.tokens.radius.md)
                    .shadow(theme.tokens.shadow.md.clone())
                    .font_family(mono)
                    .text_sm()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        // Keep clicks inside the command input from triggering backdrop dismissal.
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .flex_shrink_0()
                            .font_bold()
                            .text_color(badge.background)
                            .child(":"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(base::input::Input::new(&self.input)),
                    ),
            )
    }
}

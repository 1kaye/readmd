use std::rc::Rc;

use gpui_kit::{
    App, Entity, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window,
    base::{self, StyledExt as _},
    div,
    prelude::FluentBuilder as _,
    rems,
};

use super::EditorMode;
use super::command::HintList;
use crate::theme;

type DismissHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>;
type SelectHintHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct CommandBar {
    input: Entity<base::input::InputState>,
    hints: HintList,
    selected_ix: Option<usize>,
    on_dismiss: Option<DismissHandler>,
    on_select_hint: Option<SelectHintHandler>,
}

impl CommandBar {
    pub fn new(input: &Entity<base::input::InputState>, hints: HintList) -> Self {
        Self {
            input: input.clone(),
            hints,
            selected_ix: None,
            on_dismiss: None,
            on_select_hint: None,
        }
    }

    pub fn selected_ix(mut self, selected_ix: Option<usize>) -> Self {
        self.selected_ix = selected_ix;
        self
    }

    pub fn on_dismiss(
        mut self,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    pub fn on_select_hint(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select_hint = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CommandBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = base::Theme::global(cx);
        let colors = theme.tokens.colors;
        let badge = theme::mode_badge_colors(cx, EditorMode::Command);
        let has_hints_panel = !self.hints.items.is_empty() || self.hints.footer.is_some();
        let on_select_hint = self.on_select_hint;
        let selected_ix = self.selected_ix;

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
                    .id("command-bar-container")
                    .w_full()
                    .max_w(rems(36.))
                    .flex()
                    .flex_col()
                    .bg(colors.surface)
                    .text_color(colors.foreground)
                    .border_1()
                    .border_color(colors.ring)
                    .shadow(theme.tokens.shadow.md.clone())
                    .font_family(theme::APP_UI_FONT)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        // Keep clicks inside the command box and hints from dismissing it.
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .id("command-bar-box")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .text_sm()
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
                    .when(has_hints_panel, |this| {
                        this.child(
                            div()
                                .id("command-bar-hints")
                                .flex()
                                .flex_col()
                                .border_t_1()
                                .border_color(colors.border)
                                .children(self.hints.items.into_iter().enumerate().map(
                                    move |(ix, hint)| {
                                        let is_selected = selected_ix == Some(ix);
                                        let insert_text = hint.insert_text.clone();
                                        let on_select = on_select_hint.clone();

                                        div()
                                            .id(("command-hint", ix))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .gap_4()
                                            .px_3()
                                            .py_1p5()
                                            .text_xs()
                                            .when(is_selected, |row| {
                                                row.bg(colors.accent)
                                                    .text_color(colors.accent_foreground)
                                            })
                                            .hover(|style| style.bg(colors.muted))
                                            .when_some(on_select, |row, handler| {
                                                row.on_click(move |_, window, cx| {
                                                    handler(insert_text.clone(), window, cx);
                                                })
                                            })
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .font_bold()
                                                    .text_color(if is_selected {
                                                        colors.accent_foreground
                                                    } else {
                                                        colors.foreground
                                                    })
                                                    .child(hint.label),
                                            )
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_color(if is_selected {
                                                        colors.accent_foreground.alpha(0.85)
                                                    } else {
                                                        colors.muted_foreground
                                                    })
                                                    .child(hint.description),
                                            )
                                    },
                                ))
                                .when_some(self.hints.footer, |panel, footer| {
                                    panel.child(
                                        div()
                                            .px_3()
                                            .py_1()
                                            .border_t_1()
                                            .border_color(colors.border)
                                            .text_xs()
                                            .text_color(colors.muted_foreground)
                                            .child(footer),
                                    )
                                }),
                        )
                    }),
            )
    }
}

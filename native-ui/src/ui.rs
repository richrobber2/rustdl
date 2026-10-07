//! Shared native page chrome. Helpers only style caller-supplied text; privacy
//! decisions stay with each screen.
use super::*;

/// Full-height page shell drawn over the app background.
pub(super) fn page(cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(cx.theme().background.opacity(0.88))
        .text_color(cx.theme().foreground)
}

/// Top bar with a Back button and the page title.
pub(super) fn header(back: Button, title_id: &'static str, title: impl Into<SharedString>) -> Div {
    div().flex().items_center().gap_4().p_4().child(back).child(
        div()
            .flex_1()
            .min_w_0()
            .text_2xl()
            .font_semibold()
            .child(semantic_text(title_id, title)),
    )
}

/// Vertical scroll body shared by list and form screens.
pub(super) fn body(id: &'static str, handle: &ScrollHandle) -> Stateful<Div> {
    semantic_scroll(id, handle)
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .p_4()
        .gap_4()
}

/// Bordered surface used for grouped controls and list rows.
pub(super) fn card(cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted.opacity(0.45))
}

pub(super) fn card_title(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Div {
    div()
        .text_lg()
        .font_semibold()
        .child(semantic_text(id, text))
}

pub(super) fn muted_text(id: impl Into<ElementId>, text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(semantic_text(id, text))
}

pub(super) fn error_text(id: impl Into<ElementId>, text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_color(cx.theme().danger)
        .child(semantic_text(id, text))
}

/// Wrapping row for secondary actions so lists stay compact.
pub(super) fn button_row() -> Div {
    div().flex().flex_wrap().gap_2()
}

/// Two equal-width columns of full-width buttons.
pub(super) fn button_grid(buttons: Vec<Button>) -> Div {
    let mut grid = div().flex().flex_col().gap_3();
    let mut buttons = buttons.into_iter();
    while let Some(first) = buttons.next() {
        let second = buttons.next();
        grid = grid.child(
            div()
                .flex()
                .gap_3()
                .child(div().flex_1().min_w_0().child(first.w_full()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .when_some(second, |cell, button| cell.child(button.w_full())),
                ),
        );
    }
    grid
}

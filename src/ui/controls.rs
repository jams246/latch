use super::theme::current;
use gpui::{prelude::*, *};

pub(super) fn icon(glyph: &'static str) -> Div {
    div()
        .font_family("Segoe MDL2 Assets")
        .text_size(px(15.))
        .child(glyph)
}

pub(super) fn quiet(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    button(id, label)
        .bg(transparent_black())
        .border_color(transparent_black())
}

pub fn button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    let colors = current();
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(colors.control))
        .border_1()
        .border_color(rgb(colors.border))
        .text_color(rgb(colors.text))
        .cursor_pointer()
        .focusable()
        .tab_index(0)
        .focus(move |style| style.border_color(rgb(colors.accent)))
        .hover(move |style| style.bg(rgb(colors.hover)))
        .child(text.into())
}

pub(super) fn dialog_header(title: impl Into<SharedString>) -> Div {
    let colors = current();
    div()
        .w_full()
        .flex_shrink_0()
        .px_4()
        .py_3()
        .rounded_t_lg()
        .bg(rgb(colors.control))
        .border_b_1()
        .border_color(rgb(colors.border))
        .text_lg()
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.into())
}

pub(super) struct Hint(pub(super) String, pub(super) String);

impl Render for Hint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let colors = current();
        div()
            .max_w(px(600.))
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(colors.control))
            .border_1()
            .border_color(rgb(colors.border))
            .text_color(rgb(colors.text))
            .text_sm()
            .child(div().child(self.0.clone()))
            .child(div().text_color(rgb(colors.muted)).child(self.1.clone()))
    }
}

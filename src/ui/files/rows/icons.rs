use gpui::{prelude::*, *};

pub(super) fn generic_icon(is_dir: bool, is_link: bool) -> Div {
    div()
        .relative()
        .size_4()
        .flex_shrink_0()
        .when(is_dir, |icon| {
            icon.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(2.))
                    .w(px(7.))
                    .h(px(4.))
                    .rounded_sm()
                    .bg(rgb(0xe8bd58)),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(5.))
                    .w_4()
                    .h(px(10.))
                    .rounded_sm()
                    .bg(rgb(0xe8bd58)),
            )
        })
        .when(!is_dir, |icon| {
            icon.child(
                div()
                    .absolute()
                    .left(px(2.))
                    .top_0()
                    .w(px(12.))
                    .h_4()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0xb0cada))
                    .bg(rgb(0x365369))
                    .child(div().mx(px(2.)).mt(px(4.)).h(px(1.)).bg(rgb(0xb0cada)))
                    .child(div().mx(px(2.)).mt(px(3.)).h(px(1.)).bg(rgb(0xb0cada))),
            )
        })
        .when(is_link, |icon| {
            icon.child(
                div()
                    .absolute()
                    .left_0()
                    .bottom_0()
                    .size(px(10.))
                    .bg(rgb(0x192a37))
                    .text_xs()
                    .line_height(px(10.))
                    .text_color(rgb(0x9ce6d6))
                    .child("↗"),
            )
        })
}

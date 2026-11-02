use super::Latch;
use gpui::{
    BorderStyle, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, canvas, div, fill, point, prelude::*, px, quad, rgb, size, transparent_black,
};
#[cfg(test)]
use gpui::{TestAppContext, VisualTestContext};

fn thumb_geometry(height: Pixels, maximum: Pixels, offset: Pixels) -> (Pixels, Pixels) {
    let thumb = (height * (height / (height + maximum)))
        .max(px(24.))
        .min(height);
    let top = (-offset / maximum).clamp(0., 1.) * (height - thumb);
    (top, thumb)
}

impl Latch {
    pub(super) fn connection_scrollbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.connection_scroll.clone();
        let entity = cx.entity();
        let scrollbar = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let maximum = handle.max_offset().height;
                if maximum <= px(0.) || bounds.size.height <= px(0.) {
                    return;
                }
                let (top, height) = thumb_geometry(bounds.size.height, maximum, handle.offset().y);
                let thumb = Bounds::new(
                    point(bounds.left() + px(2.), bounds.top() + top),
                    size(bounds.size.width - px(4.), height),
                );
                window.paint_quad(fill(bounds, rgb(super::super::theme::current().control)));
                window.paint_quad(quad(
                    thumb,
                    px(3.),
                    rgb(super::super::theme::current().muted),
                    px(0.),
                    transparent_black(),
                    BorderStyle::default(),
                ));
                window.on_mouse_event({
                    let entity = entity.clone();
                    let handle = handle.clone();
                    move |event: &MouseDownEvent, phase, _, cx| {
                        if !phase.bubble()
                            || event.button != MouseButton::Left
                            || !bounds.contains(&event.position)
                        {
                            return;
                        }
                        entity.update(cx, |this, cx| {
                            let grab = if thumb.contains(&event.position) {
                                event.position.y - thumb.top()
                            } else {
                                height / 2.
                            };
                            this.connection_scroll_drag = Some(grab);
                            let travel = bounds.size.height - height;
                            if travel > px(0.) {
                                let fraction = ((event.position.y - bounds.top() - grab) / travel)
                                    .clamp(0., 1.);
                                handle.set_offset(point(px(0.), -maximum * fraction));
                            }
                            cx.notify();
                        });
                        cx.stop_propagation();
                    }
                });
                window.on_mouse_event({
                    let entity = entity.clone();
                    move |_: &MouseUpEvent, _, _, cx| {
                        entity.update(cx, |this, _| this.connection_scroll_drag = None);
                    }
                });
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if !phase.bubble() || !event.dragging() {
                        return;
                    }
                    entity.update(cx, |this, cx| {
                        if let Some(grab) = this.connection_scroll_drag {
                            let travel = bounds.size.height - height;
                            if travel > px(0.) {
                                let fraction = ((event.position.y - bounds.top() - grab) / travel)
                                    .clamp(0., 1.);
                                handle.set_offset(point(px(0.), -maximum * fraction));
                                cx.notify();
                            }
                        }
                    });
                });
            },
        )
        .size_full();
        div()
            .debug_selector(|| "connection-scrollbar".into())
            .absolute()
            .right_0()
            .top_0()
            .bottom_0()
            .w(px(10.))
            .child(scrollbar)
    }
}

#[cfg(test)]
#[gpui::test]
fn track_click_and_thumb_drag_scroll_connections(cx: &mut TestAppContext) {
    use latch::storage::{AuthMethod, Connection, Store};
    use std::{path::PathBuf, sync::Arc};

    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/connections-scrollbar")
        .join(uuid::Uuid::new_v4().to_string());
    let mut store = Store::load(directory.join("latch.db")).unwrap();
    for index in 0..20 {
        store
            .upsert(Connection::new(
                format!("Synthetic {index:02}"),
                "example.invalid".into(),
                "tester".into(),
                AuthMethod::Password,
            ))
            .unwrap();
    }
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    let mut visual = VisualTestContext::from_window(*handle, cx);
    visual.simulate_resize(size(px(720.), px(480.)));
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx)).clear();
    let track = visual.debug_bounds("connection-scrollbar").unwrap();
    let bottom = point(track.center().x, track.bottom() - px(2.));
    visual.simulate_click(bottom, Default::default());
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx)).clear();
    handle
        .read_with(&visual, |app, _| {
            assert_eq!(
                app.connection_scroll.offset().y,
                -app.connection_scroll.max_offset().height
            );
        })
        .unwrap();
    let gear = visual.debug_bounds("settings-open").unwrap().center();
    visual.simulate_click(gear, Default::default());
    visual.update(|window, cx| window.draw(cx)).clear();
    let back = visual.debug_bounds("settings-back").unwrap().center();
    visual.simulate_click(back, Default::default());
    visual.update(|window, cx| window.draw(cx)).clear();
    handle
        .read_with(&visual, |app, _| {
            assert_eq!(
                app.connection_scroll.offset().y,
                -app.connection_scroll.max_offset().height
            )
        })
        .unwrap();
    visual.simulate_mouse_down(bottom, MouseButton::Left, Default::default());
    visual.simulate_event(MouseMoveEvent {
        position: point(track.center().x, track.top()),
        pressed_button: Some(MouseButton::Left),
        ..Default::default()
    });
    visual.simulate_mouse_up(
        point(track.center().x, track.top()),
        MouseButton::Left,
        Default::default(),
    );
    visual.run_until_parked();
    handle
        .read_with(&visual, |app, _| {
            assert_eq!(app.connection_scroll.offset().y, px(0.));
        })
        .unwrap();
}

#[test]
fn thumb_tracks_scroll_endpoints_and_minimum_size() {
    assert_eq!(
        thumb_geometry(px(100.), px(100.), px(0.)),
        (px(0.), px(50.))
    );
    assert_eq!(
        thumb_geometry(px(100.), px(100.), px(-100.)),
        (px(50.), px(50.))
    );
    assert_eq!(
        thumb_geometry(px(100.), px(900.), px(-900.)),
        (px(76.), px(24.))
    );
}

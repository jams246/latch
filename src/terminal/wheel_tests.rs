use super::*;

#[gpui::test]
fn requested_mouse_tracking_takes_priority_on_both_screens(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    let position = fixture.cell(2., 4.);
    for screen in [b"\x1b[?1049l".as_slice(), b"\x1b[?1049h"] {
        fixture.state.feed(screen);
        for tracking in [1000, 1002, 1003] {
            fixture
                .state
                .feed(format!("\x1b[?{tracking}h\x1b[?1006h").as_bytes());
            fixture.wheel(position, ScrollDelta::Lines(point(0., 2.)));
            fixture.wheel(position, ScrollDelta::Lines(point(0., -1.)));
            assert_eq!(fixture.data(), b"\x1b[<64;5;3M\x1b[<64;5;3M\x1b[<65;5;3M");
            fixture.state.feed(format!("\x1b[?{tracking}l").as_bytes());
        }
    }
}

#[gpui::test]
fn disabling_alternate_scroll_suppresses_arrows_but_not_mouse_reports(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.feed(b"\x1b[?1049h\x1b[?1007l");
    let position = fixture.cell(1., 3.);
    fixture.wheel(position, ScrollDelta::Lines(point(0., 1.)));
    assert!(fixture.data().is_empty());
    fixture.state.feed(b"\x1b[?1000h\x1b[?1006h");
    fixture.wheel(position, ScrollDelta::Lines(point(0., -1.)));
    assert_eq!(fixture.data(), b"\x1b[<65;4;2M");
    fixture.state.feed(b"\x1b[?1000l\x1b[?1007h");
    fixture.wheel(position, ScrollDelta::Lines(point(0., -1.)));
    assert_eq!(fixture.data(), b"\x1b[B");
}

#[gpui::test]
fn shift_and_selection_keep_scrolling_local_with_mouse_tracking(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.feed(b"\x1b[?1000h\x1b[?1006h");
    let position = fixture.cell(0., 0.);
    fixture.visual.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Lines(point(0., 1.)),
        modifiers: Modifiers {
            shift: true,
            ..Default::default()
        },
        ..Default::default()
    });
    assert_eq!(fixture.first_line(), "line 089");
    fixture
        .visual
        .simulate_mouse_down(position, MouseButton::Left, Modifiers::default());
    fixture.wheel(position, ScrollDelta::Lines(point(0., 1.)));
    assert_eq!(fixture.first_line(), "line 088");
    assert!(fixture.data().is_empty());
}

#[gpui::test]
fn mouse_reports_preserve_history_position_and_completed_selection(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.feed(b"\x1b[?1000h\x1b[?1006h");
    let start = fixture.cell(0., 0.);
    let end = fixture.cell(0., 7.);
    fixture
        .visual
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    fixture
        .visual
        .simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    fixture
        .visual
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    fixture.state.scroll(5);
    fixture.wheel(end, ScrollDelta::Lines(point(0., 1.)));
    assert_eq!(fixture.first_line(), "line 085");
    assert_eq!(fixture.state.selected_text().as_deref(), Some("line 090"));
    assert_eq!(fixture.data(), b"\x1b[<64;8;1M");
}

#[gpui::test]
fn fractional_wheel_reports_accumulate_and_include_modifiers(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.feed(b"\x1b[?1000h\x1b[?1006h");
    let position = fixture.cell(2., 4.);
    for _ in 0..4 {
        fixture.visual.simulate_event(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(5.))),
            modifiers: Modifiers {
                alt: true,
                control: true,
                ..Default::default()
            },
            ..Default::default()
        });
    }
    assert_eq!(fixture.data(), b"\x1b[<88;5;3M");
}

#[gpui::test]
fn padding_reports_edge_cells_and_terminal_inset_is_ten_pixels(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.feed(b"\x1b[?1000h\x1b[?1006h");
    fixture
        .handle
        .read_with(&fixture.visual, |view, _| {
            let bounds = view.bounds.unwrap();
            assert_eq!(
                (bounds.left(), bounds.top(), bounds.size.height),
                (px(10.), px(10.), px(200.))
            );
        })
        .unwrap();
    fixture.wheel(point(px(5.), px(5.)), ScrollDelta::Lines(point(0., 1.)));
    assert_eq!(fixture.data(), b"\x1b[<64;1;1M");
    fixture.wheel(
        point(px(395.), px(215.)),
        ScrollDelta::Lines(point(0., -1.)),
    );
    let columns = fixture
        .state
        .snapshot()
        .cells
        .iter()
        .filter(|cell| cell.row == 0)
        .count();
    assert_eq!(fixture.data(), format!("\x1b[<65;{columns};10M").as_bytes());
}

#[test]
fn legacy_and_utf8_reports_respect_coordinate_limits() {
    let modifiers = Modifiers::default();
    assert_eq!(
        input::wheel_report(4, 2, true, modifiers, TermMode::empty()),
        Some(b"\x1b[M`%#".to_vec())
    );
    assert_eq!(
        input::wheel_report(222, 222, false, modifiers, TermMode::empty()),
        Some(vec![27, 91, 77, 97, 255, 255])
    );
    assert_eq!(
        input::wheel_report(223, 0, true, modifiers, TermMode::empty()),
        None
    );
    assert_eq!(
        input::wheel_report(0, 223, true, modifiers, TermMode::empty()),
        None
    );
    assert_eq!(
        input::wheel_report(2014, 2014, false, modifiers, TermMode::UTF8_MOUSE),
        Some("\x1b[Ma\u{7ff}\u{7ff}".as_bytes().to_vec())
    );
    assert_eq!(
        input::wheel_report(2015, 0, true, modifiers, TermMode::UTF8_MOUSE),
        None
    );
    assert_eq!(
        input::wheel_report(0, 2015, true, modifiers, TermMode::UTF8_MOUSE),
        None
    );
    assert_eq!(
        input::wheel_report(3000, 4000, true, modifiers, TermMode::SGR_MOUSE),
        Some(b"\x1b[<64;3001;4001M".to_vec())
    );
}

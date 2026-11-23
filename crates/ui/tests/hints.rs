//! Quick jump end to end in a headless window: the labels go on the buttons and cards that
//! painted, typing a label presses its target, Shift picks a card's play button and Escape
//! presses nothing.

use std::{cell::RefCell, rc::Rc};

use gpui::prelude::FluentBuilder as _;

use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div,
};
use ui::{Button, Card, HintLayer, Theme};

type Log = Rc<RefCell<Vec<&'static str>>>;

/// Two buttons and a card stacked down the window, with the labels over them.
struct Page {
    layer: Entity<HintLayer>,
    log: Log,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let note = |what: &'static str| {
            let log = self.log.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| {
                log.borrow_mut().push(what)
            }
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(Button::new("one").label("One").on_click(note("one")))
            .child(Button::new("two").label("Two").on_click(note("two")))
            .child(
                Card::new("card", "Card")
                    .press(note("card-open"))
                    .play(false, note("card-play")),
            )
            .child(self.layer.clone())
    }
}

fn open_page(cx: &mut TestAppContext) -> (Entity<Page>, &mut VisualTestContext, Log) {
    cx.update(|cx| cx.set_global(Theme::dark()));
    let log: Log = Rc::default();
    let page_log = log.clone();
    let (page, window) = cx.add_window_view(move |window, cx| Page {
        layer: cx.new(|cx| HintLayer::new(window, cx)),
        log: page_log,
    });
    window.run_until_parked();
    (page, window, log)
}

/// Puts the labels up and lets the frame that marks the targets and the one that labels
/// them both draw.
fn show_labels(page: &Entity<Page>, window: &mut VisualTestContext) {
    window.update(|window, cx| {
        let layer = page.read(cx).layer.clone();
        layer.update(cx, |layer, cx| layer.open(window, cx));
    });
    window.run_until_parked();
}

fn layer_open(page: &Entity<Page>, window: &mut VisualTestContext) -> bool {
    window.update(|_, cx| page.read(cx).layer.read(cx).is_open())
}

#[gpui::test]
async fn a_typed_label_presses_its_button(cx: &mut TestAppContext) {
    let (page, window, log) = open_page(cx);
    show_labels(&page, window);
    assert!(layer_open(&page, window));
    // Three targets in reading order take a, s and d.
    window.simulate_keystrokes("s");
    assert_eq!(*log.borrow(), ["two"]);
    assert!(
        !layer_open(&page, window),
        "the labels stay up after a jump"
    );
}

#[gpui::test]
async fn shift_plays_a_card_instead_of_opening_it(cx: &mut TestAppContext) {
    let (page, window, log) = open_page(cx);
    show_labels(&page, window);
    window.simulate_keystrokes("shift-d");
    assert_eq!(*log.borrow(), ["card-play"]);

    show_labels(&page, window);
    window.simulate_keystrokes("d");
    assert_eq!(*log.borrow(), ["card-play", "card-open"]);
}

#[gpui::test]
async fn a_slip_keeps_the_labels_and_escape_presses_nothing(cx: &mut TestAppContext) {
    let (page, window, log) = open_page(cx);
    show_labels(&page, window);
    window.simulate_keystrokes("m");
    assert!(
        layer_open(&page, window),
        "a key no label starts with closed the labels"
    );
    window.simulate_keystrokes("escape");
    assert!(!layer_open(&page, window));
    assert!(log.borrow().is_empty());
}

#[gpui::test]
async fn a_slip_takes_its_shift_away(cx: &mut TestAppContext) {
    let (page, window, log) = open_page(cx);
    show_labels(&page, window);
    // A shifted slip that names no label is dropped with its Shift, so the card opens.
    window.simulate_keystrokes("shift-m d");
    assert_eq!(*log.borrow(), ["card-open"]);
}

/// A page with nothing to press.
struct Blank {
    layer: Entity<HintLayer>,
}

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.layer.clone())
    }
}

#[gpui::test]
async fn labels_on_nothing_close_at_once(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_global(Theme::dark()));
    let (blank, window) = cx.add_window_view(|window, cx| Blank {
        layer: cx.new(|cx| HintLayer::new(window, cx)),
    });
    window.run_until_parked();
    window.update(|window, cx| {
        let layer = blank.read(cx).layer.clone();
        layer.update(cx, |layer, cx| layer.open(window, cx));
    });
    window.run_until_parked();
    assert!(
        !window.update(|_, cx| blank.read(cx).layer.read(cx).is_open()),
        "the labels kept the keyboard with nothing to press"
    );
}

/// A button under a bar drawn over the whole page, with a button of the bar's own.
struct Covered {
    layer: Entity<HintLayer>,
    log: Log,
}

impl Render for Covered {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let note = |what: &'static str| {
            let log = self.log.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| {
                log.borrow_mut().push(what)
            }
        };
        div()
            .relative()
            .size_full()
            .child(Button::new("under").label("Under").on_click(note("under")))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .when(ui::hinting(cx), |bar| bar.child(ui::hint_cover()))
                    .child(Button::new("bar").label("Bar").on_click(note("bar"))),
            )
            .child(self.layer.clone())
    }
}

#[gpui::test]
async fn chrome_drawn_over_the_page_hides_the_labels_under_it(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_global(Theme::dark()));
    let log: Log = Rc::default();
    let page_log = log.clone();
    let (page, window) = cx.add_window_view(move |window, cx| Covered {
        layer: cx.new(|cx| HintLayer::new(window, cx)),
        log: page_log,
    });
    window.run_until_parked();
    window.update(|window, cx| {
        let layer = page.read(cx).layer.clone();
        layer.update(cx, |layer, cx| layer.open(window, cx));
    });
    window.run_until_parked();
    // Only the bar's own button has a label, so it takes the first one.
    window.simulate_keystrokes("a");
    assert_eq!(*log.borrow(), ["bar"]);
}

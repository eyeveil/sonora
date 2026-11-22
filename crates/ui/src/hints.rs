//! Quick jump: a label on everything that can be pressed on screen, and typing a label presses
//! it, the way Rex's fastcopy labels text in a pane. Buttons, cards and table rows mark
//! themselves with [`hint_target`] while hints are up, [`HintLayer`] labels what painted in that
//! frame, and the keys typed into the layer pick one.
//!
//! The digit keys on a numbered list are the other quick jump, and [`dial`] decides when a
//! typed number is final.

use std::rc::Rc;
use std::time::Duration;

use gpui::accesskit::Role;
use gpui::prelude::*;
use gpui::{
    App, Bounds, ClickEvent, Context, FocusHandle, Focusable, FontWeight, Global, KeyDownEvent,
    KeyboardButton, KeyboardClickEvent, Pixels, SharedString, Task, Window, anchored, canvas,
    deferred, div, point, px,
};
use i18n::t;

use crate::metrics::Text;
use crate::theme::ActiveTheme as _;

/// The letters labels are made of, home row first so the labels most pages need are the
/// easiest to type.
pub const ALPHABET: &str = "asdfghjklqwertyuiopzxcvbnm";
/// How long a typed number waits for another digit before it plays, when one could follow.
pub const DIAL_WAIT: Duration = Duration::from_millis(650);
/// How far a label sits in from the corner of what it marks.
const LABEL_INSET: Pixels = px(2.);
/// Two targets whose corners are closer than this share a row when labels are ordered.
const ROW_SLACK: f32 = 4.;
/// The deferred-draw priority of the labels, above every popover and menu.
const LAYER_PRIORITY: usize = 1_000;

/// What a target does when its label is typed.
pub type Jump = Rc<dyn Fn(&mut Window, &mut App)>;
/// A click handler shared between an element's click and its label.
pub(crate) type SharedClick = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// One thing a label can press: where it showed, what a plain label does and what the label
/// typed with Shift does, which falls back to the plain action.
#[derive(Clone)]
pub struct Target {
    pub bounds: Bounds<Pixels>,
    pub primary: Jump,
    pub alternate: Option<Jump>,
}

/// The targets marked while hints are up, gathered anew every frame.
#[derive(Default)]
struct Marks {
    active: bool,
    seen: Vec<Target>,
}

impl Global for Marks {}

/// Whether hints are up, so pressable elements mark themselves. Elements check this before
/// adding a [`hint_target`], so nothing is drawn or kept while hints are down.
pub fn hinting(cx: &App) -> bool {
    cx.try_global::<Marks>().is_some_and(|marks| marks.active)
}

/// An empty element filling its parent that offers the parent to the hint labels. Put it last
/// in a pressable element while [`hinting`]. It marks in paint, so an element that is hidden or
/// scrolled out of its clip never gets a label.
pub fn hint_target(primary: Jump, alternate: Option<Jump>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            let shown = bounds.intersect(&window.content_mask().bounds);
            if shown.size.width <= Pixels::ZERO || shown.size.height <= Pixels::ZERO {
                return;
            }
            let marks = cx.default_global::<Marks>();
            if marks.active {
                marks.seen.push(Target {
                    bounds: shown,
                    primary,
                    alternate,
                });
            }
        },
    )
    .absolute()
    .inset_0()
}

/// A quick jump that runs a click handler as a keyboard press would, for elements whose
/// action is their click.
pub(crate) fn click_jump(handler: SharedClick) -> Jump {
    Rc::new(move |window, cx| {
        let event = ClickEvent::Keyboard(KeyboardClickEvent {
            button: KeyboardButton::Enter,
            bounds: Bounds::default(),
        });
        handler(&event, window, cx);
    })
}

/// The labels for `count` targets, all as long as the longest has to be, so no label is the
/// start of another and each one is final once typed.
pub fn labels(count: usize) -> Vec<String> {
    let alphabet: Vec<char> = ALPHABET.chars().collect();
    let base = alphabet.len();
    let mut width = 1;
    let mut capacity = base;
    while capacity < count {
        width += 1;
        capacity = capacity.saturating_mul(base);
    }
    (0..count)
        .map(|mut index| {
            let mut label = vec![alphabet[0]; width];
            for place in (0..width).rev() {
                label[place] = alphabet[index % base];
                index /= base;
            }
            label.into_iter().collect()
        })
        .collect()
}

/// Puts targets in reading order, top to bottom and then left to right, so labels run the way
/// the page reads.
fn reading_order(targets: &mut [Target]) {
    targets.sort_by(|a, b| {
        let (a, b) = (a.bounds.origin, b.bounds.origin);
        let row = (f32::from(a.y) / ROW_SLACK).round() - (f32::from(b.y) / ROW_SLACK).round();
        row.total_cmp(&0.)
            .then(f32::from(a.x).total_cmp(&f32::from(b.x)))
    });
}

/// What one typed label key leads to.
#[derive(Debug, PartialEq, Eq)]
pub enum Pick {
    /// More keys are needed: the typed letters start at least one label.
    Partial,
    /// The typed letters are this label, by its place in the list.
    Found(usize),
    /// No label starts with the typed letters.
    Miss,
}

/// Matches what is typed against the labels.
pub fn pick(labels: &[String], typed: &str) -> Pick {
    if let Some(at) = labels.iter().position(|label| label == typed) {
        return Pick::Found(at);
    }
    match labels.iter().any(|label| label.starts_with(typed)) {
        true => Pick::Partial,
        false => Pick::Miss,
    }
}

/// What a number typed on a numbered list of `count` rows does next.
#[derive(Debug, PartialEq, Eq)]
pub enum Dialed {
    /// Play row `n`, counted from 1, now: no further digit could name a row.
    Now(usize),
    /// Row `n` is meant unless another digit follows within [`DIAL_WAIT`].
    Wait(usize),
    /// No row has that number.
    Nothing,
}

/// Decides what the digits typed so far mean on a list of `count` rows. On a list of five a 3
/// plays at once, while on a list of forty a 3 waits, since 30 to 39 are rows too.
pub fn dial(typed: &str, count: usize) -> Dialed {
    let Ok(number) = typed.parse::<usize>() else {
        return Dialed::Nothing;
    };
    if number == 0 || number > count {
        return Dialed::Nothing;
    }
    match number.saturating_mul(10) > count {
        true => Dialed::Now(number),
        false => Dialed::Wait(number),
    }
}

/// The digits typed on a numbered list so far, and the wait for one more.
#[derive(Default)]
pub struct Dial {
    typed: String,
    wait: Option<Task<()>>,
}

impl Dial {
    /// The number shown while it waits for another digit.
    pub fn pending(&self) -> Option<&str> {
        self.wait.as_ref().map(|_| self.typed.as_str())
    }

    /// Adds `digit` and returns what the number now means. A digit that names no row starts a
    /// new number with itself, so a slip does not need Escape.
    pub fn push(&mut self, digit: char, count: usize) -> Dialed {
        self.wait = None;
        self.typed.push(digit);
        let dialed = dial(&self.typed, count);
        if dialed == Dialed::Nothing {
            self.typed = digit.to_string();
            return self.settle(dial(&self.typed, count));
        }
        self.settle(dialed)
    }

    /// Keeps the digits only while the number waits.
    fn settle(&mut self, dialed: Dialed) -> Dialed {
        if !matches!(dialed, Dialed::Wait(_)) {
            self.typed.clear();
        }
        dialed
    }

    /// Holds the task that plays the waiting number once [`DIAL_WAIT`] passes.
    pub fn hold(&mut self, wait: Task<()>) {
        self.wait = Some(wait);
    }

    /// Forgets the digits and the wait, after the number played or the list changed.
    pub fn clear(&mut self) {
        self.typed.clear();
        self.wait = None;
    }
}

/// The labels over the window while quick jump is up. It takes the keyboard when it opens and
/// gives it back when a label is typed, Escape is pressed or the pointer goes down anywhere.
pub struct HintLayer {
    focus: FocusHandle,
    /// Where the keyboard was before the labels took it.
    restore: Option<FocusHandle>,
    open: bool,
    /// The targets of the first frame with hints up, each with its label. `None` until that
    /// frame painted.
    labeled: Option<Rc<Vec<(String, Target)>>>,
    typed: String,
    /// Whether any key of the label was typed with Shift, which picks the alternate action.
    shifted: bool,
}

impl HintLayer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        // Labels without the keyboard could not be typed, so losing it takes them down.
        cx.on_blur(&focus, window, |this, window, cx| {
            this.restore = None;
            this.close(window, cx);
        })
        .detach();
        Self {
            focus,
            restore: None,
            open: false,
            labeled: None,
            typed: String::new(),
            shifted: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Puts labels on what can be pressed. Every view repaints, so even cached ones mark their
    /// targets in the next frame.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(window, cx);
            return;
        }
        self.restore = window.focused(cx);
        self.open = true;
        self.labeled = None;
        self.typed.clear();
        self.shifted = false;
        let marks = cx.default_global::<Marks>();
        marks.active = true;
        marks.seen.clear();
        window.focus(&self.focus, cx);
        window.refresh();
        cx.notify();
    }

    /// Takes the labels down and hands the keyboard back.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.labeled = None;
        let marks = cx.default_global::<Marks>();
        marks.active = false;
        marks.seen.clear();
        if let Some(focus) = self.restore.take() {
            window.focus(&focus, cx);
        }
        window.refresh();
        cx.notify();
    }

    /// Labels what the frame marked. It runs after the frame, so the labels show in the next.
    fn label(&mut self, cx: &mut Context<Self>) {
        if !self.open || self.labeled.is_some() {
            return;
        }
        let mut seen = std::mem::take(&mut cx.default_global::<Marks>().seen);
        reading_order(&mut seen);
        let names = labels(seen.len());
        self.labeled = Some(Rc::new(names.into_iter().zip(seen).collect()));
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let keystroke = &event.keystroke;
        match keystroke.key.as_str() {
            "escape" => return self.close(window, cx),
            "backspace" => {
                self.typed.pop();
                cx.notify();
                return;
            }
            _ => {}
        }
        let modifiers = keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
            return;
        }
        let key = keystroke.key.to_lowercase();
        let mut chars = key.chars();
        let (Some(letter), None) = (chars.next(), chars.next()) else {
            return;
        };
        if !ALPHABET.contains(letter) {
            return;
        }
        self.typed.push(letter);
        self.shifted |= modifiers.shift;
        let Some(labeled) = self.labeled.clone() else {
            self.typed.clear();
            return;
        };
        let names: Vec<String> = labeled.iter().map(|(name, _)| name.clone()).collect();
        match pick(&names, &self.typed) {
            Pick::Partial => cx.notify(),
            // A slip drops the key rather than the labels, as fastcopy does.
            Pick::Miss => {
                self.typed.pop();
                cx.notify();
            }
            Pick::Found(at) => {
                let target = labeled[at].1.clone();
                let jump = match self.shifted {
                    true => target.alternate.unwrap_or(target.primary),
                    false => target.primary,
                };
                self.close(window, cx);
                jump(window, cx);
            }
        }
    }

    /// One label, the typed part dimmed, at the top left of what it presses.
    fn badge(&self, name: &str, target: &Target, cx: &App) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let typed = self.typed.len();
        let origin = target.bounds.origin;
        div()
            .absolute()
            .left(origin.x + LABEL_INSET)
            .top(origin.y + LABEL_INSET)
            .flex()
            .px_1()
            .rounded(theme.radius / 2.)
            .bg(theme.primary)
            .text_color(theme.primary_foreground)
            .text_size(theme.text(Text::Small))
            .font_weight(FontWeight::BOLD)
            .shadow_md()
            .child(
                div()
                    .opacity(0.45)
                    .child(SharedString::from(name[..typed].to_uppercase())),
            )
            .child(SharedString::from(name[typed..].to_uppercase()))
    }
}

impl Focusable for HintLayer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for HintLayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let me = cx.entity().downgrade();
        let viewport = window.viewport_size();
        let labeled = self.labeled.clone().unwrap_or_default();
        let shown: Vec<_> = labeled
            .iter()
            .filter(|(name, _)| name.starts_with(&self.typed))
            .map(|(name, target)| self.badge(name, target, cx).into_any_element())
            .collect();
        let label = t!("hints-title");

        let frame = div()
            .id("hint-layer")
            .role(Role::Dialog)
            .aria_label(label)
            .relative()
            .w(viewport.width)
            .h(viewport.height)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .occlude()
            .on_any_mouse_down(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.close(window, cx);
            }))
            .children(shown)
            // The first frame with hints up paints every target before this, since the
            // layer draws last; once it has, the marks get their labels.
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, _, cx| {
                        cx.defer(move |cx| {
                            me.update(cx, |this, cx| this.label(cx)).ok();
                        });
                    },
                )
                .absolute()
                .size_0(),
            );

        deferred(
            anchored()
                .position(point(Pixels::ZERO, Pixels::ZERO))
                .child(frame),
        )
        .with_priority(LAYER_PRIORITY)
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_share_one_width() {
        assert_eq!(labels(3), ["a", "s", "d"]);
        let many = labels(30);
        assert!(many.iter().all(|label| label.len() == 2));
        assert_eq!(many[0], "aa");
        assert_eq!(many[1], "as");
        let unique: std::collections::HashSet<_> = many.iter().collect();
        assert_eq!(unique.len(), 30);
    }

    #[test]
    fn pick_waits_for_a_full_label() {
        let names = labels(30);
        assert_eq!(pick(&names, "a"), Pick::Partial);
        assert_eq!(pick(&names, "as"), Pick::Found(1));
        assert_eq!(pick(&names, "m"), Pick::Miss);
    }

    #[test]
    fn dial_plays_at_once_when_no_digit_can_follow() {
        assert_eq!(dial("3", 5), Dialed::Now(3));
        assert_eq!(dial("1", 10), Dialed::Wait(1));
        assert_eq!(dial("10", 10), Dialed::Now(10));
        assert_eq!(dial("2", 10), Dialed::Now(2));
        assert_eq!(dial("3", 40), Dialed::Wait(3));
        assert_eq!(dial("0", 40), Dialed::Nothing);
        assert_eq!(dial("6", 5), Dialed::Nothing);
    }

    #[test]
    fn a_digit_past_the_list_starts_over() {
        let mut dial = Dial::default();
        assert_eq!(dial.push('1', 15), Dialed::Wait(1));
        dial.hold(Task::ready(()));
        assert_eq!(dial.pending(), Some("1"));
        // 19 is past a list of 15, so the 9 is a number of its own.
        assert_eq!(dial.push('9', 15), Dialed::Now(9));
        assert_eq!(dial.pending(), None);
        assert_eq!(dial.push('1', 15), Dialed::Wait(1));
        assert_eq!(dial.push('2', 15), Dialed::Now(12));
    }

    #[test]
    fn reading_order_runs_by_row_then_column() {
        let jump: Jump = Rc::new(|_, _| {});
        let at = |x: f32, y: f32| Target {
            bounds: Bounds::new(point(px(x), px(y)), gpui::size(px(10.), px(10.))),
            primary: jump.clone(),
            alternate: None,
        };
        let mut targets = vec![at(50., 100.), at(10., 101.), at(10., 10.)];
        reading_order(&mut targets);
        let order: Vec<_> = targets
            .iter()
            .map(|t| (f32::from(t.bounds.origin.x), f32::from(t.bounds.origin.y)))
            .collect();
        assert_eq!(order, [(10., 10.), (10., 101.), (50., 100.)]);
    }
}

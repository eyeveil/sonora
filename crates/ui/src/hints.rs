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
    App, Bounds, ClickEvent, Context, EntityId, FocusHandle, Focusable, FontWeight, Global,
    KeyDownEvent, KeyboardButton, KeyboardClickEvent, Pixels, SharedString, Task, Window, anchored,
    canvas, deferred, div, point, px,
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
    /// Where in the frame's paint order the target was marked.
    painted: usize,
}

/// The targets and covers marked while hints are up, gathered anew every frame.
#[derive(Default)]
struct Marks {
    active: bool,
    seen: Vec<Target>,
    /// What painted over the page, with its place in paint order. A target painted before a
    /// cover that holds its centre is hidden under it and gets no label.
    covers: Vec<(usize, Bounds<Pixels>)>,
    painted: usize,
}

impl Marks {
    fn next(&mut self) -> usize {
        self.painted += 1;
        self.painted
    }

    fn clear(&mut self) {
        self.seen.clear();
        self.covers.clear();
        self.painted = 0;
    }

    /// The targets nothing painted later covers.
    fn uncovered(&mut self) -> Vec<Target> {
        let covers = std::mem::take(&mut self.covers);
        let mut seen = std::mem::take(&mut self.seen);
        seen.retain(|target| {
            let centre = target.bounds.center();
            !covers
                .iter()
                .any(|(painted, bounds)| *painted > target.painted && bounds.contains(&centre))
        });
        self.painted = 0;
        seen
    }
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
                let painted = marks.next();
                marks.seen.push(Target {
                    bounds: shown,
                    primary,
                    alternate,
                    painted,
                });
            }
        },
    )
    .absolute()
    .inset_0()
}

/// An empty element filling its parent that hides the labels of whatever painted under the
/// parent, for chrome drawn over the page such as a see-through player bar. Put it first in the
/// parent while [`hinting`], so the parent's own buttons, painted after it, keep their labels.
pub fn hint_cover() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            let shown = bounds.intersect(&window.content_mask().bounds);
            let marks = cx.default_global::<Marks>();
            if marks.active {
                let painted = marks.next();
                marks.covers.push((painted, shown));
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

/// What a number typed on a numbered list does next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialed {
    /// Play the row shown at this place, counted from 0, now: no longer number starts with it.
    Now(usize),
    /// That row is meant unless another key follows within [`DIAL_WAIT`].
    Wait(usize),
    /// No row has this number yet, but a longer one starts with it, as `2.` does on a disc.
    Partial,
    /// No row has a number that starts with this.
    Nothing,
}

/// Decides what the keys typed so far mean, given the number each row shows in display order.
/// On a list of five a 3 plays at once, while on a list of forty a 3 waits, since 30 to 39 are
/// rows too. Matching what is shown rather than the row's place keeps a local album with a
/// missing track, or a two-disc album numbered `2.3`, on the row the listener reads.
pub fn dial(typed: &str, numbers: &[SharedString]) -> Dialed {
    if typed.is_empty() {
        return Dialed::Nothing;
    }
    let exact = numbers.iter().position(|number| number.as_ref() == typed);
    let longer = numbers
        .iter()
        .any(|number| number.len() > typed.len() && number.starts_with(typed));
    match (exact, longer) {
        (Some(at), false) => Dialed::Now(at),
        (Some(at), true) => Dialed::Wait(at),
        (None, true) => Dialed::Partial,
        (None, false) => Dialed::Nothing,
    }
}

/// The keys typed on a numbered list so far, and the wait for one more.
#[derive(Default)]
pub struct Dial {
    typed: String,
    wait: Option<Task<()>>,
    /// The list the keys were typed on. Keys typed on another list start a new number.
    list: Option<EntityId>,
}

impl Dial {
    /// The keys typed toward the number so far.
    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// The number shown while it waits for another key.
    pub fn pending(&self) -> Option<&str> {
        self.wait.as_ref().map(|_| self.typed.as_str())
    }

    /// Adds `key` and returns what the number now means. A key that leads to no row starts a
    /// new number with itself, so a slip does not need Escape.
    pub fn push(&mut self, key: char, list: EntityId, numbers: &[SharedString]) -> Dialed {
        if self.list != Some(list) {
            self.typed.clear();
            self.list = Some(list);
        }
        self.wait = None;
        self.typed.push(key);
        let dialed = dial(&self.typed, numbers);
        if dialed == Dialed::Nothing {
            self.typed = key.to_string();
            return self.settle(dial(&self.typed, numbers));
        }
        self.settle(dialed)
    }

    /// Keeps the keys only while the number waits.
    fn settle(&mut self, dialed: Dialed) -> Dialed {
        if !matches!(dialed, Dialed::Wait(_) | Dialed::Partial) {
            self.typed.clear();
        }
        dialed
    }

    /// Holds the task that plays the waiting number once [`DIAL_WAIT`] passes.
    pub fn hold(&mut self, wait: Task<()>) {
        self.wait = Some(wait);
    }

    /// Forgets the keys and the wait, after the number played or the list changed.
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
    /// Whether each typed key of the label had Shift. Any one picks the alternate action, and
    /// Backspace takes its key's Shift away with it, as fastcopy does.
    shifted: Vec<bool>,
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
            shifted: Vec::new(),
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
        self.shifted.clear();
        let marks = cx.default_global::<Marks>();
        marks.active = true;
        marks.clear();
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
        marks.clear();
        if let Some(focus) = self.restore.take() {
            window.focus(&focus, cx);
        }
        window.refresh();
        cx.notify();
    }

    /// Labels what the frame marked. It runs after the frame, so the labels show in the next.
    fn label(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || self.labeled.is_some() {
            return;
        }
        // The labels are fixed once given, so later frames stop marking.
        let marks = cx.default_global::<Marks>();
        marks.active = false;
        let mut seen = marks.uncovered();
        // Labels on nothing would only hold the keyboard until Escape.
        if seen.is_empty() {
            return self.close(window, cx);
        }
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
                self.shifted.pop();
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
        let Some(labeled) = self.labeled.clone() else {
            return;
        };
        self.typed.push(letter);
        self.shifted.push(modifiers.shift);
        let names: Vec<String> = labeled.iter().map(|(name, _)| name.clone()).collect();
        match pick(&names, &self.typed) {
            Pick::Partial => cx.notify(),
            // A slip drops the key rather than the labels, as fastcopy does.
            Pick::Miss => {
                self.typed.pop();
                self.shifted.pop();
                cx.notify();
            }
            Pick::Found(at) => {
                let target = labeled[at].1.clone();
                let jump = match self.shifted.contains(&true) {
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
                    move |_, _, window, cx| {
                        window.defer(cx, move |window, cx| {
                            me.update(cx, |this, cx| this.label(window, cx)).ok();
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

    fn numbered(count: usize) -> Vec<SharedString> {
        (1..=count).map(|n| n.to_string().into()).collect()
    }

    #[test]
    fn dial_plays_at_once_when_no_digit_can_follow() {
        assert_eq!(dial("3", &numbered(5)), Dialed::Now(2));
        assert_eq!(dial("1", &numbered(10)), Dialed::Wait(0));
        assert_eq!(dial("10", &numbered(10)), Dialed::Now(9));
        assert_eq!(dial("2", &numbered(10)), Dialed::Now(1));
        assert_eq!(dial("3", &numbered(40)), Dialed::Wait(2));
        assert_eq!(dial("0", &numbered(40)), Dialed::Nothing);
        assert_eq!(dial("6", &numbered(5)), Dialed::Nothing);
    }

    #[test]
    fn dial_follows_the_number_shown_not_the_place() {
        // A local album missing track 2, and a two-disc album.
        let gaps: Vec<SharedString> = ["1", "3", "4"].map(Into::into).to_vec();
        assert_eq!(dial("3", &gaps), Dialed::Now(1));
        assert_eq!(dial("2", &gaps), Dialed::Nothing);
        let discs: Vec<SharedString> = ["1.1", "1.2", "2.1", "2.2"].map(Into::into).to_vec();
        assert_eq!(dial("2", &discs), Dialed::Partial);
        assert_eq!(dial("2.", &discs), Dialed::Partial);
        assert_eq!(dial("2.2", &discs), Dialed::Now(3));
    }

    #[test]
    fn keys_on_another_list_start_a_new_number() {
        let list = numbered(15);
        let mut dial = Dial::default();
        assert_eq!(dial.push('1', EntityId::from(1u64), &list), Dialed::Wait(0));
        assert_eq!(dial.push('2', EntityId::from(2u64), &list), Dialed::Now(1));
    }

    #[test]
    fn a_digit_past_the_list_starts_over() {
        let list = numbered(15);
        let mut dial = Dial::default();
        assert_eq!(dial.push('1', EntityId::from(1u64), &list), Dialed::Wait(0));
        dial.hold(Task::ready(()));
        assert_eq!(dial.pending(), Some("1"));
        // 19 is past a list of 15, so the 9 is a number of its own.
        assert_eq!(dial.push('9', EntityId::from(1u64), &list), Dialed::Now(8));
        assert_eq!(dial.pending(), None);
        assert_eq!(dial.push('1', EntityId::from(1u64), &list), Dialed::Wait(0));
        assert_eq!(dial.push('2', EntityId::from(1u64), &list), Dialed::Now(11));
    }

    #[test]
    fn reading_order_runs_by_row_then_column() {
        let jump: Jump = Rc::new(|_, _| {});
        let at = |x: f32, y: f32| Target {
            bounds: Bounds::new(point(px(x), px(y)), gpui::size(px(10.), px(10.))),
            primary: jump.clone(),
            alternate: None,
            painted: 0,
        };
        let mut targets = vec![at(50., 100.), at(10., 101.), at(10., 10.)];
        reading_order(&mut targets);
        let order: Vec<_> = targets
            .iter()
            .map(|t| (f32::from(t.bounds.origin.x), f32::from(t.bounds.origin.y)))
            .collect();
        assert_eq!(order, [(10., 10.), (10., 101.), (50., 100.)]);
    }

    #[test]
    fn a_cover_painted_later_hides_what_is_under_it() {
        let jump: Jump = Rc::new(|_, _| {});
        let square =
            |x: f32, y: f32| Bounds::new(point(px(x), px(y)), gpui::size(px(10.), px(10.)));
        let mut marks = Marks::default();
        for (x, y) in [(0., 0.), (0., 100.)] {
            let painted = marks.next();
            marks.seen.push(Target {
                bounds: square(x, y),
                primary: jump.clone(),
                alternate: None,
                painted,
            });
        }
        // The bar over the second row paints after it, then its own button paints on top.
        let painted = marks.next();
        marks.covers.push((
            painted,
            Bounds::new(point(px(0.), px(95.)), gpui::size(px(100.), px(20.))),
        ));
        let painted = marks.next();
        marks.seen.push(Target {
            bounds: square(50., 100.),
            primary: jump,
            alternate: None,
            painted,
        });
        let left: Vec<_> = marks
            .uncovered()
            .iter()
            .map(|t| (f32::from(t.bounds.origin.x), f32::from(t.bounds.origin.y)))
            .collect();
        assert_eq!(left, [(0., 0.), (50., 100.)]);
    }
}

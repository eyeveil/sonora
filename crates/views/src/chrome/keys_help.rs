//! The list of every keyboard key the app answers, opened with `?`, the way Vimium and Vim's
//! own `:help` list theirs. Shortcuts the user can rebind show their current keys.

use gpui::accesskit::Role;
use gpui::prelude::*;
use gpui::{Context, FocusHandle, KeyDownEvent, SharedString, Window, div};
use i18n::t;
use state::Sonora;
use ui::{ActiveTheme as _, Dismiss, FORM_CONTEXT, Kbd, Modal, Text};

/// Where a row's keys come from: fixed vim keys, or a shortcut the user may have rebound.
enum Keys {
    /// Each item is one way to press it; a space inside one is a sequence, as in `g g`.
    Fixed(&'static [&'static str]),
    /// The id of a shortcut under Settings, Keybinds.
    Shortcut(&'static str),
}

/// One group of rows: its Fluent heading, then each row's keys and Fluent description.
type Group = (&'static str, &'static [(Keys, &'static str)]);

const GROUPS: &[Group] = &[
    (
        "keys-help-lists",
        &[
            (Keys::Fixed(&["j", "k"]), "keys-help-step"),
            (Keys::Fixed(&["g g", "shift-g"]), "keys-help-ends"),
            (Keys::Fixed(&["ctrl-d", "ctrl-u"]), "keys-help-half-page"),
            (Keys::Fixed(&["enter"]), "keys-help-play"),
            (Keys::Fixed(&["o"]), "keys-help-open"),
            (Keys::Shortcut("open-filter"), "keys-help-filter"),
            (Keys::Fixed(&["n", "shift-n"]), "keys-help-matches"),
            (Keys::Fixed(&["1", "2", "3"]), "keys-help-number"),
            (Keys::Fixed(&["escape"]), "keys-help-deselect"),
        ],
    ),
    (
        "keys-help-moving",
        &[
            (Keys::Shortcut("hint-jump"), "keys-help-jump"),
            (Keys::Fixed(&["shift-h", "shift-l"]), "keys-help-history"),
            (Keys::Fixed(&["shift-j", "shift-k"]), "keys-help-sections"),
            (Keys::Fixed(&["g h"]), "keys-help-home"),
            (Keys::Fixed(&["g s"]), "keys-help-search"),
            (Keys::Fixed(&["g r"]), "keys-help-recents"),
            (Keys::Fixed(&["g l"]), "keys-help-songs"),
            (Keys::Fixed(&["g a"]), "keys-help-albums"),
            (Keys::Fixed(&["g shift-a"]), "keys-help-artists"),
            (Keys::Fixed(&["g p"]), "keys-help-playlists"),
            (Keys::Shortcut("toggle-powerbar"), "keys-help-quick-search"),
        ],
    ),
    (
        "keys-help-playing",
        &[
            (Keys::Shortcut("toggle-playback"), "keys-help-pause"),
            (Keys::Shortcut("toggle-queue"), "keys-help-queue"),
            (Keys::Shortcut("toggle-lyrics"), "keys-help-lyrics"),
            (Keys::Shortcut("toggle-fullscreen"), "keys-help-fullscreen"),
        ],
    ),
    (
        "keys-help-searching",
        &[
            (Keys::Fixed(&["h", "l"]), "keys-help-columns"),
            (Keys::Fixed(&["i", "/"]), "keys-help-type"),
            (Keys::Fixed(&["ctrl-j", "ctrl-p"]), "keys-help-step-field"),
        ],
    ),
];

/// The key list over the window. It takes the keyboard while open, so no key reaches the page
/// under it, and gives it back on Escape, `?`, `q` or a click outside.
pub(crate) struct KeysHelp {
    open: bool,
    focus: FocusHandle,
    restore: Option<FocusHandle>,
}

impl KeysHelp {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            open: false,
            focus: cx.focus_handle(),
            restore: None,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the list, or closes it when it is already up.
    pub(crate) fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return self.close(window, cx);
        }
        self.open = true;
        self.restore = window.focused(cx);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        if let Some(focus) = self.restore.take() {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let typed = keystroke.key_char.as_deref().unwrap_or(&keystroke.key);
        if matches!(typed, "?" | "q") && !keystroke.modifiers.modified() {
            cx.stop_propagation();
            self.close(window, cx);
        }
    }
}

impl Render for KeysHelp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let theme = cx.theme();
        let overrides = Sonora::global(cx).settings.read(cx).keybinds().clone();
        let muted = theme.muted_foreground;
        let small = theme.text(Text::Small);
        let gap = theme.metrics.pad;

        let mut modal = Modal::new("keys-help", t!("keys-help-title"))
            .close_button()
            .on_dismiss(cx.listener(|this, _, window, cx| this.close(window, cx)));
        for (group, (heading, rows)) in GROUPS.iter().enumerate() {
            let rows = rows.iter().enumerate().map(|(index, (keys, what))| {
                let keys: Vec<String> = match keys {
                    Keys::Fixed(keys) => keys.iter().map(|keys| (*keys).to_owned()).collect(),
                    Keys::Shortcut(id) => input::shown(id, &overrides).into_iter().collect(),
                };
                let caps = keys.into_iter().enumerate().map(move |(at, keys)| {
                    div()
                        .flex()
                        .gap_1()
                        .children(keys.split(' ').enumerate().map(move |(part, key)| {
                            Kbd::new(SharedString::from(key.to_owned()))
                                .spoken(("keys-help-key", group * 1_000 + index * 10 + at + part))
                        }))
                });
                div()
                    .id(("keys-help-row", group * 100 + index))
                    .role(Role::Row)
                    .aria_label(i18n::lookup(what, None))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(gap)
                    .child(i18n::lookup(what, None))
                    .child(div().flex().gap(gap / 2.).children(caps))
            });
            modal = modal.child(
                div()
                    .id(("keys-help-group", group))
                    .role(Role::Group)
                    .aria_label(i18n::lookup(heading, None))
                    .flex()
                    .flex_col()
                    .gap(gap / 2.)
                    .pb(gap)
                    .child(
                        div()
                            .text_size(small)
                            .text_color(muted)
                            .child(i18n::lookup(heading, None)),
                    )
                    .children(rows),
            );
        }

        div()
            .absolute()
            .inset_0()
            .key_context(FORM_CONTEXT)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| {
                cx.stop_propagation();
                this.close(window, cx);
            }))
            .child(modal)
            .into_any_element()
    }
}

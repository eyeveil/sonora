use std::time::Duration;

use gpui::accesskit::Role;
use gpui::prelude::*;
use gpui::{AnyView, App, Context, Entity, FocusHandle, FontWeight, Pixels, Render};
use gpui::{StyleRefinement, Window, deferred, div};
use i18n::t;
use input::{TypeDigit, WORKSPACE_CONTEXT};
use state::{AppSettings, Playback, Queue, SideTab, Sonora};
use ui::{
    Activate, ActiveTheme as _, DIAL_WAIT, Deselect, Dial, Dialed, Entrance, Remove, SelectFirst,
    SelectLast, SelectNext, SelectPageDown, SelectPageUp, SelectPrevious, Text, dial, entering,
    shown_listing, veiled,
};
use ui::{hint_cover, hinting};

use crate::chrome::{
    AppNotice, Chrome, PlayerBar, SidebarLeft, SidebarRight, SwipeCue, TitleBarOptions,
};
use crate::shared::menus::CardMenu;
use crate::shells::Shell;

pub(crate) struct Workspace {
    sidebar: Entity<SidebarLeft>,
    player_bar: Entity<PlayerBar>,
    settings: Entity<AppSettings>,
    sidebar_right: Entity<SidebarRight>,
    card_menu: Entity<CardMenu>,
    notice: Entity<AppNotice>,
    swipe_cue: Entity<SwipeCue>,
    content: AnyView,
    /// A screen's own header, floated over the top of the page and outside its transition.
    /// The page pads itself to start beneath it.
    header: Option<AnyView>,
    /// Whether the page is laid out cached. A page that redraws part of itself every frame is
    /// not, since a cached view that redraws builds every view in it again, and it caches its
    /// own parts instead.
    cached: bool,
    transition: Option<Entrance>,
    focus: FocusHandle,
    /// The digits typed toward a row of the page's numbered list.
    dial: Dial,
}

impl Workspace {
    pub fn new(
        playback: Entity<Playback>,
        queue: Entity<Queue>,
        content: AnyView,
        cx: &mut Context<Self>,
    ) -> Self {
        let sidebar = cx.new(SidebarLeft::new);
        let sidebar_right = cx.new(|cx| SidebarRight::new(queue.clone(), playback.clone(), cx));
        let player_bar = cx.new(|cx| PlayerBar::new(playback.clone(), queue, cx));
        let settings = Sonora::global(cx).settings.clone();
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();

        Self {
            sidebar,
            player_bar,
            settings,
            sidebar_right,
            card_menu: CardMenu::entity(cx),
            notice: cx.new(AppNotice::new),
            swipe_cue: cx.new(SwipeCue::new),
            content,
            header: None,
            cached: true,
            transition: None,
            focus: cx.focus_handle(),
            dial: Dial::default(),
        }
    }

    /// A digit typed on the page: the row of the page's list showing that number plays, at
    /// once when no longer number starts with it and after [`DIAL_WAIT`] otherwise, so 1 and
    /// 12 both reach their rows on a list of twenty. A page without a list lets the digit go.
    fn type_digit(&mut self, digit: &TypeDigit, window: &mut Window, cx: &mut Context<Self>) {
        let Some(list) = shown_listing(cx) else {
            self.dial.clear();
            cx.propagate();
            return;
        };
        let numbers = list.numbers(cx);
        match self.dial.push(digit.0, list.entity_id(), &numbers) {
            Dialed::Now(display) => list.play_at(display, window, cx),
            Dialed::Wait(_) | Dialed::Partial => {
                // The number belongs to this list, so a page left meanwhile plays nothing. It
                // is matched again when the wait ends, so a list sorted or filtered meanwhile
                // plays the row that shows the number then.
                let table = list.entity_id();
                let typed = self.dial.typed().to_owned();
                let wait = cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor().timer(DIAL_WAIT).await;
                    this.update_in(cx, |this, window, cx| {
                        this.dial.clear();
                        let shown = shown_listing(cx).filter(|list| list.entity_id() == table);
                        let Some(list) = shown else {
                            return;
                        };
                        if let Dialed::Now(display) | Dialed::Wait(display) =
                            dial(&typed, &list.numbers(cx))
                        {
                            list.play_at(display, window, cx);
                        }
                        cx.notify();
                    })
                    .ok();
                });
                self.dial.hold(wait);
            }
            Dialed::Nothing => {}
        }
        cx.notify();
    }

    /// The number waiting for another digit, shown over the foot of the page.
    fn dial_badge(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        let typed = self.dial.pending()?.to_owned();
        let theme = cx.theme();
        let label = t!("dial-pending", number = typed.as_str());
        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(theme.metrics.player_bar + theme.metrics.pad * 2.)
                .flex()
                .justify_center()
                .child(
                    div()
                        .id("dial-pending")
                        .role(Role::Status)
                        .aria_label(label.clone())
                        .px(theme.metrics.pad * 1.5)
                        .py(theme.metrics.pad / 2.)
                        .rounded(theme.radius)
                        .bg(theme.primary)
                        .text_color(theme.primary_foreground)
                        .text_size(theme.text(Text::Body))
                        .font_weight(FontWeight::BOLD)
                        .shadow_md()
                        .child(label),
                ),
        )
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn toggle_sidebar(&self, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |sidebar, cx| sidebar.toggle(cx));
    }

    pub fn toggle_sidebar_right(&self, cx: &mut Context<Self>) {
        self.sidebar_right.update(cx, |panel, cx| panel.toggle(cx));
    }

    pub fn show_side(&self, tab: SideTab, cx: &mut Context<Self>) {
        self.sidebar_right
            .update(cx, |panel, cx| panel.show(tab, cx));
    }

    /// Opens the right sidebar on `tab` without closing it when it already shows there.
    pub fn reveal_side(&self, tab: SideTab, cx: &mut Context<Self>) {
        self.sidebar_right
            .update(cx, |panel, cx| panel.reveal(tab, cx));
    }

    /// Shows a page, with the header it wants above it or none, and laid out `cached` or not.
    /// The three arrive together so a header can never outlive its screen.
    pub fn set_content(
        &mut self,
        content: AnyView,
        header: Option<AnyView>,
        cached: bool,
        cx: &mut Context<Self>,
    ) {
        self.content = content;
        self.header = header;
        self.cached = cached;
        cx.notify();
    }

    pub fn reveal_content(&mut self, cx: &mut Context<Self>) -> Duration {
        if cx.reduce_motion() {
            self.transition = None;
            return Duration::ZERO;
        }

        let entrance = Entrance::start();
        self.transition = Some(entrance);
        cx.notify();
        entrance.span()
    }

    pub fn finish_transition(&mut self, cx: &mut Context<Self>) {
        if self.transition.take().is_some() {
            cx.notify();
        }
    }

    fn hidden(&mut self, window: &mut Window, cx: &Context<Self>) -> f32 {
        if cx.reduce_motion() {
            self.transition = None;
            return 0.;
        }
        let Some(transition) = self.transition else {
            return 0.;
        };
        if transition.running() {
            window.request_animation_frame();
        }
        transition.hidden()
    }
}

impl Shell for Workspace {
    fn title_bar(&self, content: Option<AnyView>, cx: &App) -> TitleBarOptions {
        TitleBarOptions {
            navigation: true,
            sidebar_open: self.sidebar.read(cx).is_open(),
            sidebar_right: Some(self.sidebar_right.read(cx).is_open()),
            border: true,
            content,
            transparent: false,
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hinted = hinting(cx);
        let right = self.sidebar_right.read(cx).occupied_width(window);
        let reserved = self.sidebar_right.read(cx).reserved_width();
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.adapt(reserved, window, cx));
        let left = self.sidebar.read(cx).occupied_width();
        let overlay_width = self.sidebar.read(cx).overlay_width();
        Chrome::publish(left, right, reserved, cx);
        let covered = self.sidebar_right.read(cx).covers_content(window);
        let overlay = self.sidebar.read(cx).overlays();
        let bar_height = PlayerBar::height(window, cx);
        let underlap = self.settings.read(cx).player_bar_translucent();
        let bottom = match underlap {
            true => bar_height,
            false => Pixels::ZERO,
        };
        Chrome::set_bottom(bottom, cx);
        // A cached view is laid out from the style given here and its own root
        // style is never consulted, so it can only be cached while it is in the
        // flow at a width this knows: an overlaid sidebar places itself, and a
        // closed one hides itself and takes no space at all.
        let sidebar_width = self.sidebar.read(cx).occupied_width();
        let sidebar = match overlay || sidebar_width == gpui::Pixels::ZERO {
            true => self.sidebar.clone().into_any_element(),
            false => self
                .sidebar
                .clone()
                .cached(StyleRefinement::default().w(sidebar_width).h_full())
                .into_any_element(),
        };
        // The header is cached at the height it last measured, so a page that redraws every
        // frame leaves it be. Until it has measured itself it is laid out as it is.
        let top = crate::screens::settings::Layout::of(window, cx);
        let header = self.header.clone().map(|header| match top.measured {
            true => header
                .cached(StyleRefinement::default().w_full().h(top.top))
                .into_any_element(),
            false => header.into_any_element(),
        });
        let hidden = self.hidden(window, cx);
        // The scrim is what fades the outgoing page: it is the page's own colour at
        // full strength, so covering the content with it costs no layout and the
        // content view keeps its cache. A see-through window has no such colour. The
        // scrim would be one more translucent layer over the one the root already
        // paints, and the content area would sit visibly darker than the chrome
        // around it for the length of the transition — a quad can only add coverage,
        // never replace it. There the content carries the fade itself, which is the
        // same curve at the price of leaving the cached layout path while it runs.
        let dissolving = cx.theme().transparent;
        let scrim = match dissolving {
            true => 0.,
            false => hidden,
        };
        let backdrop = cx.theme().background;
        // That price has to be paid for real, not merely accepted: opacity is baked
        // into a primitive as it is painted, and a cached view replays its primitives
        // until something notifies it, so a fading page that stayed cached would be
        // recorded on the first frame, nearly clear, and replayed that way long after
        // the transition had ended. While it carries its own fade the content is
        // rendered uncached. Every screen sizes its root `size_full`, so the layout
        // is the one the cache would have used, and only the frames of the
        // transition pay for the render.
        let content = match (dissolving && hidden > 0.) || !self.cached {
            true => self.content.clone().into_any_element(),
            false => self
                .content
                .clone()
                .cached(StyleRefinement::default().size_full())
                .into_any_element(),
        };

        div()
            .id("workspace")
            .role(Role::Main)
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .min_h_0()
            .key_context(super::shell_context(WORKSPACE_CONTEXT))
            .track_focus(&self.focus)
            .on_action(|_: &SelectNext, window, cx| {
                if let Some(table) = shown_listing(cx) {
                    table.select_next(window, cx);
                    cx.stop_propagation();
                }
            })
            .on_action(|_: &SelectPrevious, window, cx| {
                if let Some(table) = shown_listing(cx) {
                    table.select_previous(window, cx);
                    cx.stop_propagation();
                }
            })
            .on_action(|_: &SelectFirst, window, cx| {
                if let Some(table) = shown_listing(cx) {
                    table.select_first(window, cx);
                    cx.stop_propagation();
                }
            })
            .on_action(|_: &SelectLast, window, cx| {
                if let Some(table) = shown_listing(cx) {
                    table.select_last(window, cx);
                    cx.stop_propagation();
                }
            })
            .on_action(|_: &SelectPageDown, window, cx| {
                if let Some(table) = shown_listing(cx) {
                    table.select_page(false, window, cx);
                    cx.stop_propagation();
                }
            })
            .on_action(|_: &SelectPageUp, window, cx| {
                if let Some(table) = shown_listing(cx) {
                    table.select_page(true, window, cx);
                    cx.stop_propagation();
                }
            })
            .on_action(cx.listener(Self::type_digit))
            .on_action(|_: &Deselect, _, cx| {
                if let Some(table) = shown_listing(cx) {
                    table.deselect(cx);
                    cx.stop_propagation();
                }
            })
            .on_action(|_: &Activate, _, cx| {
                if let Some(table) = shown_listing(cx) {
                    table.activate(cx);
                    cx.stop_propagation();
                }
            })
            .on_action(|_: &Remove, _, cx| {
                if let Some(table) = shown_listing(cx) {
                    table.remove(cx);
                    cx.stop_propagation();
                }
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(!overlay, |this| this.child(sidebar))
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .ml(overlay_width)
                            .when(overlay, |this| this.overflow_hidden())
                            .when(covered, |this| this.hidden())
                            // A two-finger swipe across the page steps through its history,
                            // as in a browser.
                            .on_scroll_wheel({
                                let cue = self.swipe_cue.clone();
                                move |event, window, cx| {
                                    cue.update(cx, |cue, cx| cue.follow(event, window, cx));
                                }
                            })
                            .child(
                                div()
                                    .relative()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_h_0()
                                    .when(hidden > 0., |this| this.overflow_hidden())
                                    .child(
                                        div()
                                            .absolute()
                                            .left(-overlay_width)
                                            .right_0()
                                            .top_0()
                                            .bottom_0()
                                            .flex()
                                            .flex_col()
                                            .map(|this| match dissolving {
                                                true => entering(this, hidden),
                                                false => veiled(this, hidden),
                                            })
                                            .child(content),
                                    )
                                    .when(scrim > 0., |this| {
                                        this.child(
                                            div()
                                                .absolute()
                                                .left_0()
                                                .right_0()
                                                .top_0()
                                                .bottom_0()
                                                .bg(backdrop)
                                                .opacity(scrim),
                                        )
                                    })
                                    // The header comes last, so it paints over the page and
                                    // the scrim, and spans the same width the page does
                                    // beneath an overlaid sidebar.
                                    .when_some(header, |this, header| {
                                        this.child(
                                            div()
                                                .absolute()
                                                .left(-overlay_width)
                                                .right_0()
                                                .top_0()
                                                .when(hinted, |this| this.child(hint_cover()))
                                                .child(header),
                                        )
                                    }),
                            )
                            .child(self.swipe_cue.clone()),
                    )
                    .child(self.sidebar_right.clone())
                    .when(overlay, |this| this.child(self.sidebar.clone())),
            )
            .child(
                div()
                    .relative()
                    .when(underlap, |bar| bar.absolute().left_0().right_0().bottom_0())
                    // The bar covers the page under it, so rows there get no label.
                    .when(hinted, |bar| bar.child(hint_cover()))
                    .child(
                        self.player_bar
                            .clone()
                            .cached(StyleRefinement::default().w_full().h(bar_height)),
                    ),
            )
            // Deferred so the cards sit above the sidebars' resize grips. The root draws the
            // dialogs after the shell, so those still cover them.
            .child(deferred(self.notice.clone()))
            .child(self.card_menu.clone())
            .children(self.dial_badge(cx))
    }
}

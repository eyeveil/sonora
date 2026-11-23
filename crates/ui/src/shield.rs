use gpui::prelude::*;
use gpui::{App, Div, ElementId, Interactivity, Stateful, StyleRefinement, Window, div};

use crate::hints::{hint_cover, hinting};

#[derive(IntoElement)]
pub struct Shield {
    base: Stateful<Div>,
}

impl Shield {
    #[track_caller]
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { base: div().id(id) }
    }
}

impl Styled for Shield {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for Shield {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for Shield {}

impl RenderOnce for Shield {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut base = self.base;
        let overrides = std::mem::take(base.style());

        // What a shield blocks the pointer from, quick jump gives no label either.
        let mut shield = base
            .occlude()
            .when(hinting(cx), |this| this.child(hint_cover()));
        shield.style().refine(&overrides);
        shield
    }
}

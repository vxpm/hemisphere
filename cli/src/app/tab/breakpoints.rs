use crate::app::{border_style, tab::Context};
use ratatui::{
    layout::Rect,
    widgets::{Block, HighlightSpacing, List, ListDirection, ListState},
};

pub struct BreakpointsPane {
    list_state: ListState,
}

impl Default for BreakpointsPane {
    fn default() -> Self {
        Self {
            list_state: ListState::default(),
        }
    }
}

impl BreakpointsPane {
    pub fn scroll_up(&mut self) {
        self.list_state.select_previous();
    }

    pub fn scroll_down(&mut self) {
        self.list_state.select_next();
    }

    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let items = ctx.state.breakpoints().iter().map(|bp| bp.to_string());
        let list = List::new(items)
            .block(
                Block::bordered()
                    .title("Breakpoints")
                    .border_style(border_style(focused)),
            )
            .direction(ListDirection::TopToBottom)
            .highlight_symbol(" > ")
            .highlight_spacing(HighlightSpacing::Always);

        ctx.frame
            .render_stateful_widget(list, area, &mut self.list_state);
    }
}

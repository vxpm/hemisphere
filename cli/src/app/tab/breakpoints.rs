use crate::app::{border_style, tab::Context};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    symbols,
    text::Text,
    widgets::{Block, Borders, Row, Table, TableState, Tabs},
};

pub struct BreakpointsPane {
    table_state: TableState,
}

impl Default for BreakpointsPane {
    fn default() -> Self {
        Self {
            table_state: TableState::new().with_selected(Some(0)),
        }
    }
}

impl BreakpointsPane {
    pub fn scroll_up(&mut self) {
        self.table_state.scroll_up_by(1);
    }

    pub fn scroll_down(&mut self) {
        self.table_state.scroll_down_by(1);
    }

    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let block = Block::new()
            .title("Breakpoints")
            .borders(Borders::ALL)
            .border_style(border_style(focused));
        let inner = block.inner(area);
        ctx.frame.render_widget(block, area);
    }
}

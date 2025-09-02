use crate::app::{border_style, tab::Context};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Style, Stylize},
    widgets::{Block, Borders, Row, Table, TableState},
};

pub struct RegistersPane {
    table_state: TableState,
}

impl Default for RegistersPane {
    fn default() -> Self {
        Self {
            table_state: TableState::new().with_selected(Some(0)),
        }
    }
}

impl RegistersPane {
    pub fn scroll_up(&mut self) {
        self.table_state.scroll_up_by(1);
    }

    pub fn scroll_down(&mut self) {
        self.table_state.scroll_down_by(1);
    }

    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let cpu = &ctx.state.hemisphere().system.cpu;

        let header = Row::new(vec!["GPR", "Value"]).light_magenta();
        let widths = [Constraint::Length(3), Constraint::Min(1)];
        let mut rows = Vec::new();
        for gpr in 0..32 {
            rows.push(Row::new(vec![
                format!("R{gpr:02}"),
                format!("{:08X}", cpu.user.gpr[gpr]),
            ]));
        }

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .block(
                Block::new()
                    .title("Registers")
                    .borders(Borders::ALL)
                    .border_style(border_style(focused)),
            )
            .row_highlight_style(Style::new().light_blue())
            .highlight_symbol("> ");

        ctx.frame
            .render_stateful_widget(table, area, &mut self.table_state);
    }
}

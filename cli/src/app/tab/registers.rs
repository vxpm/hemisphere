use crate::app::{border_style, tab::Context};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    symbols,
    widgets::{Block, Borders, Row, Table, TableState, Tabs},
};

pub struct RegistersPane {
    current: usize,
    table_state: TableState,
}

impl Default for RegistersPane {
    fn default() -> Self {
        Self {
            current: 0,
            table_state: TableState::new().with_selected(Some(0)),
        }
    }
}

impl RegistersPane {
    pub fn next(&mut self) {
        self.current = (self.current + 1) % 2;
        self.table_state.select(Some(0));
    }

    pub fn previous(&mut self) {
        self.current = self.current.checked_sub(1).unwrap_or(1);
        self.table_state.select(Some(0));
    }

    pub fn scroll_up(&mut self) {
        self.table_state.scroll_up_by(1);
    }

    pub fn scroll_down(&mut self) {
        self.table_state.scroll_down_by(1);
    }

    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let block = Block::new()
            .title("Registers")
            .borders(Borders::ALL)
            .border_style(border_style(focused));
        let inner = block.inner(area);
        ctx.frame.render_widget(block, area);

        let [tabs_area, table_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);

        let tabs = Tabs::new(vec!["GPR", "FPR"])
            .block(
                Block::new()
                    .borders(Borders::BOTTOM)
                    .border_style(border_style(focused)),
            )
            .style(Style::new().magenta().dim())
            .highlight_style(Style::new().light_magenta().not_dim())
            .select(self.current)
            .divider(symbols::DOT)
            .padding(" ", " ");

        ctx.frame.render_widget(tabs, tabs_area);

        let cpu = &ctx.state.hemisphere().system.cpu;
        let header = Row::new(vec!["Reg", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(5), Constraint::Min(1)];
        let mut rows = Vec::new();

        match self.current {
            0 => {
                for i in 0..32 {
                    rows.push(Row::new(vec![
                        format!("R{i:02}"),
                        format!("{:08X}", cpu.user.gpr[i]),
                    ]));
                }
            }
            1 => {
                for i in 0..32 {
                    rows.push(Row::new(vec![
                        format!("F{i:02}"),
                        format!("{:?}", cpu.user.fpr[i]),
                    ]));
                }
            }
            _ => unreachable!(),
        }

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue().dim())
            .row_highlight_style(Style::new().light_blue().not_dim())
            .highlight_symbol("> ");

        ctx.frame
            .render_stateful_widget(table, table_area, &mut self.table_state);
    }
}

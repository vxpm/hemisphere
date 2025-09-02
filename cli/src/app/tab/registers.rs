use crate::app::{Action, border_style, tab::Context};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    symbols,
    text::Text,
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
    fn render_int_formats(&mut self, ctx: &mut Context, area: Rect, value: u32) {
        let header = Row::new(vec!["Format", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(8), Constraint::Min(1)];
        let rows = vec![
            Row::new(vec![Text::raw("Unsigned"), format!("{value}").into()]),
            Row::new(vec![
                Text::raw("Signed"),
                format!("{}", value as i32).into(),
            ]),
            Row::new(vec![
                Text::raw("Float"),
                format!("{:?}", value as f32).into(),
            ]),
            Row::new(vec![Text::raw("Hex"), format!("{value:08X}").into()]),
            Row::new(vec![Text::raw("Binary"), format!("{:032b}", value).into()]),
        ];

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue());

        ctx.frame.render_widget(table, area);
    }

    fn render_float_formats(&mut self, ctx: &mut Context, area: Rect, value: f64) {
        let header = Row::new(vec!["Format", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(8), Constraint::Min(1)];
        let rows = vec![
            Row::new(vec![
                Text::raw("Unsigned"),
                format!("{}", value as u64).into(),
            ]),
            Row::new(vec![
                Text::raw("Signed"),
                format!("{}", value as i64).into(),
            ]),
            Row::new(vec![Text::raw("Float"), format!("{value:?}").into()]),
            Row::new(vec![
                Text::raw("Hex"),
                format!("{:016X}", value as u64).into(),
            ]),
            Row::new(vec![
                Text::raw("Binary"),
                format!("{:064b}", value as u64).into(),
            ]),
        ];

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue());

        ctx.frame.render_widget(table, area);
    }

    fn render_gpr(&mut self, ctx: &mut Context, area: Rect) {
        let [regs, formats] =
            Layout::horizontal([Constraint::Percentage(25), Constraint::Percentage(75)])
                .areas(area);

        let header = Row::new(vec!["Reg", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(5), Constraint::Min(1)];
        let mut rows = Vec::new();

        for i in 0..32 {
            let cpu = &ctx.state.hemisphere().system.cpu;
            rows.push(Row::new(vec![
                format!("R{i:02}"),
                format!("{:?}", cpu.user.gpr[i]),
            ]));

            if self
                .table_state
                .selected()
                .is_some_and(|selected| selected == i)
            {
                self.render_int_formats(ctx, formats, cpu.user.gpr[i]);
            }
        }

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue().dim())
            .row_highlight_style(Style::new().light_blue().not_dim())
            .highlight_symbol("> ");

        ctx.frame
            .render_stateful_widget(table, regs, &mut self.table_state);
    }

    fn render_fpr(&mut self, ctx: &mut Context, area: Rect) {
        let [regs, formats] =
            Layout::horizontal([Constraint::Percentage(25), Constraint::Percentage(75)])
                .areas(area);

        let header = Row::new(vec!["Reg", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(5), Constraint::Min(1)];
        let mut rows = Vec::new();

        for i in 0..32 {
            let cpu = &ctx.state.hemisphere().system.cpu;
            rows.push(Row::new(vec![
                format!("F{i:02}"),
                format!("{:?}", cpu.user.fpr[i]),
            ]));

            if self
                .table_state
                .selected()
                .is_some_and(|selected| selected == i)
            {
                self.render_float_formats(ctx, formats, cpu.user.fpr[i]);
            }
        }

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue().dim())
            .row_highlight_style(Style::new().light_blue().not_dim())
            .highlight_symbol("> ");

        ctx.frame
            .render_stateful_widget(table, regs, &mut self.table_state);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.current = self.current.checked_sub(1).unwrap_or(1);
                self.table_state.select(Some(0));
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.current = (self.current + 1) % 2;
                self.table_state.select(Some(0));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.table_state.select_next();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.table_state.select_previous();
            }
            _ => (),
        }

        None
    }

    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let block = Block::new()
            .title("Registers")
            .borders(Borders::ALL)
            .border_style(border_style(focused));
        let inner = block.inner(area);
        ctx.frame.render_widget(block, area);

        let [tabs_area, bottom] =
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

        match self.current {
            0 => self.render_gpr(ctx, bottom),
            1 => self.render_fpr(ctx, bottom),
            _ => unreachable!(),
        }
    }
}

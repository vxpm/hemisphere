use crate::app::{Action, border_style, tab::Context};
use bytesize::ByteSize;
use hemisphere::core::arch::{Bat, CondReg, MachineState, XerReg};
use ratatui::{
    Frame,
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

macro_rules! row {
    ($name:expr; $fmt:literal, $($args:expr),*) => {
        Row::new(vec![
            Text::raw(format!($name)),
            format!($fmt, $($args),*).into(),
        ])
    };
}

impl RegistersPane {
    fn render_int_formats(&mut self, frame: &mut Frame, area: Rect, value: u32) {
        let header = Row::new(vec!["Format", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(8), Constraint::Min(1)];
        let rows = vec![
            row!("Unsigned"; "{}", value),
            row!("Signed"; "{}", value as i32),
            row!("Float"; "{:?}", value as f32),
            row!("Hex"; "{:08X}", value),
            row!("Binary"; "{:032b}", value),
        ];

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue());

        frame.render_widget(table, area);
    }

    fn render_float_formats(&mut self, frame: &mut Frame, area: Rect, value: f64) {
        let header = Row::new(vec!["Format", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(8), Constraint::Min(1)];
        let rows = vec![
            row!("Unsigned"; "{}", value as u64),
            row!("Signed"; "{}", value as i64),
            row!("Float"; "{:?}", value),
            row!("Hex"; "{:016X}", value as u64),
            row!("Binary"; "{:064b}", value as u64),
        ];

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue());

        frame.render_widget(table, area);
    }

    fn render_msr(&self, frame: &mut Frame, area: Rect, msr: MachineState) {
        let header = Row::new(vec!["Field", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(24), Constraint::Min(1)];
        let rows = vec![
            row!("Little Endian"; "{}", msr.little_endian()),
            row!("Exception Recoverable"; "{}", msr.recoverable_exception()),
            row!("Data Addr. Translation"; "{}", msr.data_addr_translation()),
            row!("Instr. Addr. Translation"; "{}", msr.instr_addr_translation()),
            row!("High Exceptions Vectors"; "{}", msr.exception_prefix()),
            row!("Machine Check Exceptions"; "{}", msr.machine_check()),
            row!("Float Available"; "{}", msr.float_available()),
            row!("User Mode"; "{}", msr.user_mode()),
            row!("External Interrupts"; "{}", msr.external_interrupts()),
            row!("Little Endian Exception"; "{}", msr.exception_little_endian()),
        ];

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue());

        frame.render_widget(table, area);
    }

    fn render_cr(&self, frame: &mut Frame, area: Rect, cr: CondReg) {
        let header = Row::new(vec!["Field", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(24), Constraint::Min(1)];
        let bit = |b| if b { "1" } else { "0" };
        let mut rows = Vec::new();

        for (i, cond) in cr.fields().iter().rev().enumerate() {
            rows.push(row!(
                "CR{i}"; "ov: {} eq: {} gt: {} lt: {}",
                bit(cond.ov()),
                bit(cond.eq()),
                bit(cond.gt()),
                bit(cond.lt())
            ));
        }

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue());

        frame.render_widget(table, area);
    }

    fn render_xer(&self, frame: &mut Frame, area: Rect, xer: XerReg) {
        let header = Row::new(vec!["Field", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(24), Constraint::Min(1)];
        let rows = vec![
            row!("Byte Count"; "{}", xer.byte_count().value()),
            row!("Carry"; "{}", xer.carry()),
            row!("Overflow"; "{}", xer.overflow()),
            row!("Overflow Fuse"; "{}", xer.overflow_fuse()),
        ];

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue());

        frame.render_widget(table, area);
    }

    fn render_bats(&self, frame: &mut Frame, area: Rect, bats: [Bat; 4]) {
        let header = Row::new(vec!["Field", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(24), Constraint::Min(1)];
        let mut rows = Vec::new();

        for (i, bat) in bats.iter().enumerate() {
            rows.extend([
                row!("BAT{i} Virtual Range"; "{}..={}", bat.start(), bat.end()),
                row!("BAT{i} Physical Range"; "{}..={}", bat.physical_start(), bat.physical_end()),
                row!("BAT{i} Length"; "{}", ByteSize(bat.block_length() as u64)),
            ]);
        }

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue());

        frame.render_widget(table, area);
    }

    fn render_core(&mut self, ctx: &mut Context, area: Rect) {
        let [regs, formats] =
            Layout::horizontal([Constraint::Percentage(20), Constraint::Percentage(80)])
                .areas(area);

        let header = Row::new(vec!["Reg", "Value"]).light_magenta().not_dim();
        let widths = [Constraint::Length(5), Constraint::Min(1)];
        let mut rows = Vec::new();

        let hex = |value: u32| -> String { format!("{:08X}", value) };
        let mut current = 0;
        macro_rules! reg {
            ($name:expr, $value:expr, $basic:expr, $detailed:ident) => {
                rows.push(Row::new(vec![$name.to_string(), ($basic)($value)]));

                if self
                    .table_state
                    .selected()
                    .is_some_and(|selected| selected == current)
                {
                    self.$detailed(&mut ctx.frame, formats, $value);
                }

                #[allow(unused_assignments)]
                {
                    current += 1;
                }
            };

            ($name:expr, $value:expr) => {
                reg!($name, $value, |value| hex(value), render_int_formats);
            };
        }

        let cpu = &ctx.state.hemisphere().system.cpu;
        reg!(
            "MSR",
            cpu.supervisor.config.msr.clone(),
            |msr: MachineState| hex(msr.to_bits()),
            render_msr
        );
        reg!(
            "CR",
            cpu.user.cr.clone(),
            |cr: CondReg| hex(cr.to_bits()),
            render_cr
        );
        reg!(
            "XER",
            cpu.user.xer.clone(),
            |xer: XerReg| hex(xer.to_bits()),
            render_xer
        );
        reg!("CTR", cpu.user.ctr);
        reg!("SRR0", cpu.supervisor.exception.srr[0]);
        reg!("SRR1", cpu.supervisor.exception.srr[1]);
        reg!(
            "IBAT",
            cpu.supervisor.memory.ibat.clone(),
            |_| "[...]".to_owned(),
            render_bats
        );
        reg!(
            "DBAT",
            cpu.supervisor.memory.dbat.clone(),
            |_| "[...]".to_owned(),
            render_bats
        );

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .style(Style::new().blue().dim())
            .row_highlight_style(Style::new().light_blue().not_dim())
            .highlight_symbol("> ");

        ctx.frame
            .render_stateful_widget(table, regs, &mut self.table_state);
    }

    fn render_gpr(&mut self, ctx: &mut Context, area: Rect) {
        let [regs, formats] =
            Layout::horizontal([Constraint::Percentage(20), Constraint::Percentage(80)])
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
                self.render_int_formats(&mut ctx.frame, formats, cpu.user.gpr[i]);
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
            Layout::horizontal([Constraint::Percentage(20), Constraint::Percentage(80)])
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
                self.render_float_formats(&mut ctx.frame, formats, cpu.user.fpr[i][0]);
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
                self.current = self.current.checked_sub(1).unwrap_or(2);
                self.table_state.select(Some(0));
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.current = (self.current + 1) % 3;
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

        let tabs = Tabs::new(vec!["Core", "GPR", "FPR"])
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
            0 => self.render_core(ctx, bottom),
            1 => self.render_gpr(ctx, bottom),
            2 => self.render_fpr(ctx, bottom),
            _ => unreachable!(),
        }
    }
}

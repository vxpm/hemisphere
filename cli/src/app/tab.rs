use crate::app::{Action, block_style, center};
use eyre_pretty::eyre::Result;
use hemisphere::{
    FREQUENCY,
    core::{
        Address,
        arch::{
            GPR, Reg, SPR,
            powerpc::{Extensions, Ins, Opcode, ParsedIns},
        },
    },
    runner::State,
};
use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Constraint, Flex, Layout, Rect, Size},
    style::{Style, Stylize},
    symbols,
    text::Text,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Row, Table},
};
use std::collections::VecDeque;
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

pub struct Control {
    pub running: bool,
}

pub struct Context<'ctx, 'frame> {
    pub control: &'ctx mut Control,
    pub state: &'ctx mut State,
    pub frame: &'ctx mut Frame<'frame>,
    pub area: Rect,
    pub focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Main,
    Memory,
    Blocks,
}

impl Tab {
    pub fn next(self) -> Tab {
        match self {
            Self::Main => Self::Memory,
            Self::Memory => Self::Blocks,
            Self::Blocks => Self::Main,
        }
    }

    pub fn previous(self) -> Tab {
        match self {
            Self::Main => Self::Blocks,
            Self::Memory => Self::Main,
            Self::Blocks => Self::Memory,
        }
    }
}

pub struct Main {
    focused_pane: usize,
    average_ips: VecDeque<f32>,
    simplified_asm: bool,
    regs_scroll_state: ScrollViewState,
}

impl Default for Main {
    fn default() -> Self {
        Self {
            focused_pane: Default::default(),
            average_ips: Default::default(),
            simplified_asm: true,
            regs_scroll_state: ScrollViewState::new(),
        }
    }
}

impl Main {
    fn render_disasm(&mut self, ctx: &mut Context, area: Rect) {
        let header = Row::new(vec!["Address", "Instruction"]).light_magenta();
        let widths = [Constraint::Length(11), Constraint::Min(1)];

        let mut rows = Vec::new();
        let mut parsed = ParsedIns::new();
        for i in 0..area.height {
            let system = &ctx.state.hemisphere().system;
            let pc = system.cpu.pc.value();
            let offset = i as i32 - area.height as i32 / 2;
            let target = Address(pc.wrapping_add_signed(offset * 4));
            let translated = system.cpu.supervisor.translate_instr_addr(target);
            let instruction = Ins::new(system.bus.read(translated), Extensions::gekko_broadway());

            if self.simplified_asm {
                instruction.parse_simplified(&mut parsed);
            } else {
                instruction.parse_basic(&mut parsed);
            }

            let addr_style = if offset == 0 {
                Style::new().light_blue()
            } else {
                Style::new().dim().blue()
            };

            let instruction_style = if instruction.op == Opcode::Illegal {
                Style::new().red().dim()
            } else {
                Style::new().white()
            };

            rows.push(Row::new([
                Text::styled(target.to_string(), addr_style),
                Text::styled(parsed.to_string(), instruction_style),
            ]));
        }

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .block(
                Block::new()
                    .title("Disassembly")
                    .borders(Borders::ALL)
                    .style(block_style(ctx.focused && self.focused_pane == 0)),
            );

        ctx.frame.render_widget(table, area);
    }

    fn render_control(&mut self, ctx: &mut Context, area: Rect) {
        let block = Block::bordered()
            .title("Control")
            .style(block_style(ctx.focused && self.focused_pane == 1));
        let inner = block.inner(area);
        ctx.frame.render_widget(block, area);

        let [status_area, ips_area, chart_area] = Layout::horizontal([
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Min(1),
        ])
        .areas(inner);

        let status = if ctx.control.running {
            Text::styled("⏵ Running", Style::new().green())
        } else {
            Text::styled("⏸ Paused", Style::new().red())
        };

        let avg_ips = (ctx.state.stats().ips.iter().sum::<f32>()
            / ctx.state.stats().ips.len().max(1) as f32)
            .abs();
        let avg_mips = avg_ips / 1_000_000.0;
        let ratio = avg_ips / FREQUENCY as f32;
        let ips = Text::styled(
            format!("{:.02} MIPS ({:.02}x)", avg_mips, ratio),
            Style::new().light_blue(),
        );

        if ctx.control.running {
            if self.average_ips.len() >= 32 {
                self.average_ips.pop_front();
            }
            self.average_ips.push_back(avg_mips);
        }

        let ips_data = self
            .average_ips
            .iter()
            .enumerate()
            .map(|(i, x)| (i as f64, *x as f64))
            .collect::<Vec<_>>();

        let ips_dataset = Dataset::default()
            .name("data1")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().magenta())
            .data(&ips_data);

        let x_axis = Axis::default().bounds([0.0, 31.0]);
        let y_axis = Axis::default().bounds([0.0, 486.0]);
        let chart = Chart::new(vec![ips_dataset]).x_axis(x_axis).y_axis(y_axis);

        ctx.frame.render_widget(status, status_area);
        ctx.frame.render_widget(ips, ips_area);
        ctx.frame.render_widget(chart, chart_area);
    }

    fn render_regs(&mut self, ctx: &mut Context, area: Rect) {
        // let block = Block::bordered()
        //     .title("Registers")
        //     .style(block_style(ctx.focused && self.focused_pane == 2));
        // let inner = block.inner(area);
        // ctx.frame.render_widget(block, area);

        let mut scroll_view = ScrollView::new(Size::new(area.width, 35))
            .horizontal_scrollbar_visibility(ScrollbarVisibility::Never);

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
                    .style(block_style(ctx.focused && self.focused_pane == 2)),
            );

        scroll_view.render_widget(table, scroll_view.area());

        ctx.frame
            .render_stateful_widget(scroll_view, area, &mut self.regs_scroll_state);
    }

    fn render_help(&mut self, ctx: &mut Context, area: Rect) {
        const HELP: [&[&'static str]; 3] = [
            // disasm
            &["[a] toggle simplified asm", "[s] step"],
            // control
            &["[r] toggle running", "[s] step"],
            // regs
            &[],
        ];

        let block = Block::bordered().title("Help").style(Style::new().dim());
        let inner = block.inner(area);
        ctx.frame.render_widget(block, area);

        if !ctx.focused {
            return;
        }

        let current_help = HELP[self.focused_pane];
        let percent = 100 / (current_help.len().max(1));
        let chunks = Layout::horizontal(
            (0..current_help.len()).map(|_| Constraint::Percentage(percent as u16)),
        )
        .flex(Flex::SpaceBetween)
        .split(inner);

        for (t, chunk) in current_help.iter().zip(chunks.iter()) {
            ctx.frame.render_widget(
                t,
                center(
                    *chunk,
                    Constraint::Length(t.len() as u16),
                    Constraint::Length(1),
                ),
            );
        }
    }

    pub fn render(&mut self, mut ctx: Context) {
        let [content, help] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).areas(ctx.area);

        let [disasm, right] =
            Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
                .areas(content);

        let [control, regs] =
            Layout::vertical([Constraint::Length(5), Constraint::Min(10)]).areas(right);

        self.render_disasm(&mut ctx, disasm);
        self.render_control(&mut ctx, control);
        self.render_regs(&mut ctx, regs);
        self.render_help(&mut ctx, help);
    }

    pub fn handle_event(&mut self, event: Event) -> Result<Option<Action>> {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Esc => return Ok(Some(Action::Unfocus)),
                KeyCode::Tab => self.focused_pane = (self.focused_pane + 1) % 3,
                other => match self.focused_pane {
                    0 => match other {
                        KeyCode::Char('s') => return Ok(Some(Action::RunStep)),
                        KeyCode::Char('a') => self.simplified_asm = !self.simplified_asm,
                        _ => (),
                    },
                    1 => match other {
                        KeyCode::Char('s') => return Ok(Some(Action::RunStep)),
                        KeyCode::Char('r') => return Ok(Some(Action::RunToggle)),
                        _ => (),
                    },
                    2 => (),
                    _ => unreachable!(),
                },
            },
            _ => (),
        }

        Ok(None)
    }
}

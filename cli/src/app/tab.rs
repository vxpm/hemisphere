use crate::app::Action;
use eyre_pretty::eyre::Result;
use hemisphere::{
    core::{
        Address,
        arch::powerpc::{Extensions, Ins, Opcode, ParsedIns},
    },
    runner::State,
};
use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Text, ToSpan},
    widgets::{Block, Borders, Paragraph, Row, Table},
};

pub struct Context<'ctx, 'frame> {
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

#[derive(Default)]
pub struct Main {}

impl Main {
    fn render_disasm(&mut self, ctx: &mut Context, area: Rect) {
        let header = Row::new(vec!["Address", "Instruction"]).light_magenta();
        let widths = [Constraint::Length(11), Constraint::Min(1)];

        let mut rows = Vec::new();
        for i in 0..area.height {
            let system = &ctx.state.hemisphere.system;
            let pc = system.cpu.pc.value();
            let offset = i as i32 - area.height as i32 / 2;
            let target = Address(pc.wrapping_add_signed(offset * 4));
            let translated = system.cpu.supervisor.translate_instr_addr(target);
            let instruction = Ins::new(
                ctx.state.hemisphere.system.bus.read(translated),
                Extensions::gekko_broadway(),
            );

            let mut parsed = ParsedIns::new();
            instruction.parse_simplified(&mut parsed);

            let addr_style = if offset == 0 {
                Style::new().blue()
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
            .block(Block::new().title("Disassembly").borders(Borders::ALL));

        ctx.frame.render_widget(table, area);
    }

    pub fn render(&mut self, mut ctx: Context) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(ctx.area);

        self.render_disasm(&mut ctx, chunks[0]);
        ctx.frame.render_widget("main help", chunks[1]);
    }

    pub fn handle_event(&mut self, event: Event) -> Result<Option<Action>> {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Esc => return Ok(Some(Action::Unfocus)),
                _ => (),
            },
            _ => (),
        }

        Ok(None)
    }
}

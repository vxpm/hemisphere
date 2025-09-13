use std::fmt::Write;

use crate::app::{Action, border_style, tab::Context};
use hemisphere::{
    Address,
    arch::disasm::{Extensions, Ins, Opcode, ParsedIns},
};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Paragraph, Row, Table},
};

pub struct DisasmPane {
    pub target: Address,
    pub follow_pc: bool,
    pub simplified_asm: bool,
}

impl Default for DisasmPane {
    fn default() -> Self {
        Self {
            target: Address::default(),
            follow_pc: true,
            simplified_asm: true,
        }
    }
}

impl DisasmPane {
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.follow_pc = false;
                self.target += 4;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.follow_pc = false;
                self.target -= 4;
            }
            KeyCode::Char('r') => return Some(Action::RunToggle),
            KeyCode::Char('s') => return Some(Action::RunStep),
            KeyCode::Char('a') => self.simplified_asm = !self.simplified_asm,
            KeyCode::Char('f') => self.follow_pc = true,
            KeyCode::Char('b') => return Some(Action::AddBreakpoint(self.target)),

            _ => (),
        }

        None
    }

    fn render_instrs(&mut self, ctx: &mut Context, area: Rect) {
        let system = &ctx.state.hemisphere().system;
        let header = Row::new(vec!["Address", "Instruction"]).light_magenta();
        let widths = [Constraint::Length(11), Constraint::Min(1)];

        let mut rows = Vec::new();
        let mut parsed = ParsedIns::new();
        for i in 0..area.height {
            let offset = i as i32 - area.height as i32 / 2;
            let current = Address(self.target.value().wrapping_add_signed(offset * 4));
            let translated = system.translate_instr_addr(current);
            let instruction = Ins::new(
                system.bus.read_pure(translated).unwrap_or(0),
                Extensions::gekko_broadway(),
            );

            if self.simplified_asm {
                instruction.parse_simplified(&mut parsed);
            } else {
                instruction.parse_basic(&mut parsed);
            }

            let addr_style = if current == system.cpu.pc {
                Style::new().green()
            } else if current == self.target {
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
                Text::styled(current.to_string(), addr_style),
                Text::styled(parsed.to_string(), instruction_style),
            ]));
        }

        let table = Table::new(rows, widths).column_spacing(2).header(header);
        ctx.frame.render_widget(table, area);
    }

    fn render_line(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let text = if let Some(addr2line) = ctx.addr2line
            && let Ok(Some(loc)) = addr2line.find_location(self.target.value() as u64)
        {
            let mut text = loc.file.unwrap_or("unknown").to_string();
            if let Some(line) = loc.line {
                write!(&mut text, ":{line}").unwrap();
            }

            text
        } else {
            String::new()
        };

        let scroll = text.chars().count().saturating_sub(area.width as usize) as u16;
        let paragraph = Paragraph::new(text)
            .block(
                Block::new()
                    .borders(Borders::TOP)
                    .border_style(border_style(focused)),
            )
            .scroll((0, scroll));

        ctx.frame.render_widget(paragraph, area);
    }

    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        if self.follow_pc {
            self.target = ctx.state.hemisphere().system.cpu.pc;
        }

        let block = Block::new()
            .title("Disassembly")
            .borders(Borders::ALL)
            .border_style(border_style(focused));
        let inner = block.inner(area);

        ctx.frame.render_widget(block, area);

        let [instrs, line] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(inner);

        self.render_instrs(ctx, instrs);
        self.render_line(ctx, line, focused);
    }
}

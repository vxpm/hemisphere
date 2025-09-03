use crate::app::{Action, border_style, tab::Context};
use hemisphere::core::{
    Address,
    arch::powerpc::{Extensions, Ins, Opcode, ParsedIns},
};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Rect},
    style::{Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Row, Table},
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

    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let system = &ctx.state.hemisphere().system;
        let header = Row::new(vec!["Address", "Instruction"]).light_magenta();
        let widths = [Constraint::Length(11), Constraint::Min(1)];

        if self.follow_pc {
            self.target = system.cpu.pc;
        }

        let mut rows = Vec::new();
        let mut parsed = ParsedIns::new();
        for i in 0..area.height {
            let offset = i as i32 - area.height as i32 / 2;
            let current = Address(self.target.value().wrapping_add_signed(offset * 4));
            let translated = system.cpu.supervisor.translate_instr_addr(current);
            let instruction = Ins::new(system.bus.read(translated), Extensions::gekko_broadway());

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

        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(header)
            .block(
                Block::new()
                    .title("Disassembly")
                    .borders(Borders::ALL)
                    .border_style(border_style(focused)),
            );

        ctx.frame.render_widget(table, area);
    }
}

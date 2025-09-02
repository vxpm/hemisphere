use crate::app::{border_style, tab::Context};
use hemisphere::core::{
    Address,
    arch::powerpc::{Extensions, Ins, Opcode, ParsedIns},
};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Row, Table},
};

pub struct DisasmPane {
    pub simplified_asm: bool,
}

impl Default for DisasmPane {
    fn default() -> Self {
        Self {
            simplified_asm: true,
        }
    }
}

impl DisasmPane {
    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
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
                    .border_style(border_style(focused)),
            );

        ctx.frame.render_widget(table, area);
    }
}

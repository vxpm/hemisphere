mod breakpoints;
mod disasm;
mod registers;
mod status;

use crate::app::{
    Action, center,
    tab::{
        breakpoints::BreakpointsPane, disasm::DisasmPane, registers::RegistersPane,
        status::StatusPane,
    },
};
use hemisphere::runner::State;
use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Flex, Layout, Rect},
    widgets::Block,
};

pub struct Context<'ctx, 'frame> {
    pub running: bool,
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
pub struct Main {
    focused_pane: usize,
    disasm: DisasmPane,
    status: StatusPane,
    registers: RegistersPane,
    breakpoints: BreakpointsPane,
}

impl Main {
    fn render_cmd(&mut self, ctx: &mut Context, area: Rect) {
        let help: [&[_]; 4] = [
            // disasm
            &[
                "[k] move up",
                "[j] move down",
                "[r] toggle running",
                "[s] step",
                "[f] follow pc",
                "[b] breakpoint",
                if self.disasm.simplified_asm {
                    "[a] use basic asm"
                } else {
                    "[a] use simplified asm"
                },
            ],
            // control
            &["[r] toggle running", "[s] step"],
            // regs
            &[
                "[k] move up",
                "[j] move down",
                "[h] previous",
                "[l] next",
                "[space] edit",
            ],
            // breakpoints
            if let Some(input) = self.breakpoints.input() {
                &[input, "[enter] confirm"]
            } else {
                &["[k] move up", "[j] move down", "[a] add", "[d] delete"]
            },
        ];

        let block = Block::bordered().title("Command");
        let inner = block.inner(area);
        ctx.frame.render_widget(block, area);

        if !ctx.focused {
            return;
        }

        let current_help = help[self.focused_pane];
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
        let [content, cmd] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).areas(ctx.area);

        let [disasm, right] =
            Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
                .areas(content);

        let [status, registers, breakpoints] = Layout::vertical([
            Constraint::Length(5),
            Constraint::Min(9),
            Constraint::Percentage(40),
        ])
        .areas(right);

        let focused = {
            let focused = ctx.focused;
            let pane = self.focused_pane;
            move |n| focused && pane == n
        };

        self.disasm.render(&mut ctx, disasm, focused(0));
        self.status.render(&mut ctx, status, focused(1));
        self.registers.render(&mut ctx, registers, focused(2));
        self.breakpoints.render(&mut ctx, breakpoints, focused(3));
        self.render_cmd(&mut ctx, cmd);
    }

    pub fn handle_event(&mut self, event: Event) -> Option<Action> {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Esc => return Some(Action::Unfocus),
                KeyCode::Char('[') | KeyCode::Char('\'') => {
                    self.focused_pane = self.focused_pane.checked_sub(1).unwrap_or(3)
                }
                KeyCode::Char(']') | KeyCode::Tab => {
                    self.focused_pane = (self.focused_pane + 1) % 4
                }
                _ => match self.focused_pane {
                    0 => return self.disasm.handle_key(key),
                    1 => return self.status.handle_key(key),
                    2 => return self.registers.handle_key(key),
                    3 => return self.breakpoints.handle_key(key),
                    _ => unreachable!(),
                },
            }
        }

        None
    }
}

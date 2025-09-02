mod disasm;
mod registers;
mod status;

use crate::app::{
    Action, center,
    tab::{disasm::DisasmPane, registers::RegistersPane, status::StatusPane},
};
use eyre_pretty::eyre::Result;
use hemisphere::runner::State;
use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    widgets::Block,
};

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
    disasm: DisasmPane,
    status: StatusPane,
    registers: RegistersPane,
}

impl Default for Main {
    fn default() -> Self {
        Self {
            focused_pane: Default::default(),
            disasm: Default::default(),
            status: Default::default(),
            registers: Default::default(),
        }
    }
}

impl Main {
    fn render_help(&mut self, ctx: &mut Context, area: Rect) {
        let help: [&[_]; 3] = [
            // disasm
            &[
                if self.disasm.simplified_asm {
                    "[a] use basic asm"
                } else {
                    "[a] use simplified asm"
                },
                "[s] step",
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
        ];

        let block = Block::bordered().title("Help").style(Style::new().gray());
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
        let [content, help] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).areas(ctx.area);

        let [disasm, right] =
            Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
                .areas(content);

        let [status, registers] =
            Layout::vertical([Constraint::Length(5), Constraint::Min(10)]).areas(right);

        let focused = {
            let focused = ctx.focused;
            let pane = self.focused_pane;
            move |n| focused && pane == n
        };

        self.disasm.render(&mut ctx, disasm, focused(0));
        self.status.render(&mut ctx, status, focused(1));
        self.registers.render(&mut ctx, registers, focused(2));
        self.render_help(&mut ctx, help);
    }

    pub fn handle_event(&mut self, event: Event) -> Result<Option<Action>> {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Esc => return Ok(Some(Action::Unfocus)),
                KeyCode::Tab => self.focused_pane = (self.focused_pane + 1) % 3,
                code => match self.focused_pane {
                    0 => match code {
                        KeyCode::Char('s') => return Ok(Some(Action::RunStep)),
                        KeyCode::Char('a') => {
                            self.disasm.simplified_asm = !self.disasm.simplified_asm
                        }
                        _ => (),
                    },
                    1 => match code {
                        KeyCode::Char('s') => return Ok(Some(Action::RunStep)),
                        KeyCode::Char('r') => return Ok(Some(Action::RunToggle)),
                        _ => (),
                    },
                    2 => match code {
                        KeyCode::Left | KeyCode::Char('h') => {
                            self.registers.previous();
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            self.registers.next();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            self.registers.scroll_down();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            self.registers.scroll_up();
                        }
                        _ => (),
                    },
                    _ => unreachable!(),
                },
            },
            _ => (),
        }

        Ok(None)
    }
}

use crate::app::{Action, border_style, tab::Context};
use hemisphere::core::Address;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    widgets::{Block, HighlightSpacing, List, ListDirection, ListState},
};

pub struct BreakpointsPane {
    input: Option<String>,
    list_state: ListState,
}

impl Default for BreakpointsPane {
    fn default() -> Self {
        Self {
            input: None,
            list_state: ListState::default(),
        }
    }
}

impl BreakpointsPane {
    pub fn input(&self) -> Option<&String> {
        self.input.as_ref()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if let Some(input) = &mut self.input {
            match key.code {
                KeyCode::Char(c) => input.push(c),
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Enter => {
                    let input = self.input.take().unwrap();
                    let input = input.replace("_", "");
                    let addr = u32::from_str_radix(input.strip_prefix("0x").unwrap_or(&input), 16);

                    if let Ok(addr) = addr {
                        return Some(Action::AddBreakpoint(Address(addr)));
                    }
                }
                _ => (),
            }
        } else {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.list_state.select_next();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.list_state.select_previous();
                }
                KeyCode::Char('a') => {
                    self.input = Some(String::new());
                }
                _ => (),
            }
        }

        None
    }

    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let items = ctx.state.breakpoints().iter().map(|bp| bp.to_string());
        let list = List::new(items)
            .block(
                Block::bordered()
                    .title("Breakpoints")
                    .border_style(border_style(focused)),
            )
            .direction(ListDirection::TopToBottom)
            .highlight_symbol(" > ")
            .highlight_spacing(HighlightSpacing::Always);

        ctx.frame
            .render_stateful_widget(list, area, &mut self.list_state);
    }
}

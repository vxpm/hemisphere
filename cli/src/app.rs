mod tab;

use crate::app::tab::Tab;
use eyre_pretty::eyre::Result;
use hemisphere::runner::Runner;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyModifiers},
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders},
};
use std::time::Duration;

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);

    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);

    area
}

/// Actions a tab might request the app to do.
enum Action {
    Unfocus,
    Quit,
}

pub struct App {
    runner: Runner,
    current_tab: Tab,
    tab_focused: bool,

    main_tab: tab::Main,
}

fn render_tabs(frame: &mut Frame, area: Rect, focused: bool, current: usize) {
    let titles = vec!["Main", "Memory", "Blocks"]
        .into_iter()
        .map(|t| t.white())
        .collect::<Vec<_>>();

    let block = Block::default()
        .title("Tabs")
        .borders(Borders::ALL)
        .style(Style::default().fg(if focused { Color::Green } else { Color::White }));

    frame.render_widget(block, area);

    let chunks = Layout::horizontal([Constraint::Min(1); 3]).split(area);
    for (index, (chunk, title)) in chunks.iter().zip(titles).enumerate() {
        let style = if index == current {
            Style::default().blue().underlined()
        } else {
            Style::default().white()
        };

        let len = title.content.len() as u16;
        frame.render_widget(
            title.style(style),
            center(*chunk, Constraint::Length(len), Constraint::Length(1)),
        );
    }
}

impl App {
    pub fn new(runner: Runner) -> Self {
        Self {
            runner,
            current_tab: Tab::Main,
            tab_focused: false,

            main_tab: Default::default(),
        }
    }

    pub fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(1)])
                    .split(frame.area());

                render_tabs(
                    frame,
                    chunks[0],
                    !self.tab_focused,
                    self.current_tab as usize,
                );

                self.runner.with_state(|state| {
                    let ctx = tab::Context {
                        state,
                        frame,
                        area: chunks[1],
                        focused: self.tab_focused,
                    };

                    match self.current_tab {
                        Tab::Main => self.main_tab.render(ctx),
                        Tab::Memory => (),
                        Tab::Blocks => (),
                    }
                });
            })?;

            if self.handle_events()? {
                break;
            }
        }

        Ok(())
    }

    fn handle_event_tabs(&mut self, event: Event) -> Option<Action> {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    self.current_tab = self.current_tab.previous();
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.current_tab = self.current_tab.next();
                }
                KeyCode::Char('q') => return Some(Action::Quit),
                _ => (),
            },
            _ => (),
        }

        None
    }

    pub fn handle_events(&mut self) -> Result<bool> {
        while event::poll(Duration::from_millis(10))? {
            let event = event::read()?;
            if let Event::Key(key) = event
                && key.modifiers.contains(KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('c')
            {
                return Ok(true);
            }

            let action = if self.tab_focused {
                match self.current_tab {
                    Tab::Main => self.main_tab.handle_event(event),
                    Tab::Memory => Ok(None),
                    Tab::Blocks => Ok(None),
                }?
            } else {
                self.handle_event_tabs(event)
            };

            let Some(action) = action else { continue };
            match action {
                Action::Unfocus => self.tab_focused = false,
                Action::Quit => return Ok(true),
            }
        }

        Ok(false)
    }
}

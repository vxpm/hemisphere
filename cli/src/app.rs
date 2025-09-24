mod tab;

use crate::app::tab::Tab;
use eyre_pretty::eyre::Result;
use hemisphere::{Address, runner::Runner};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyModifiers},
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::Block,
};
use std::time::Duration;
use tracing::debug;

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);

    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);

    area
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::LightGreen)
    } else {
        Style::default().fg(Color::White).dim()
    }
}

/// Actions a tab might request the app to do.
enum Action {
    AddBreakpoint(Address),
    RemoveBreakpoint(usize),
    RunStep,
    RunToggle,
    Unfocus,
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

    let block = Block::bordered()
        .title("Tabs")
        .border_style(border_style(focused));
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

                let running = self.runner.running();
                self.runner.with_state(|state| {
                    let ctx = tab::Context {
                        running,
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
                KeyCode::Down | KeyCode::Char('j') => {
                    self.tab_focused = true;
                }
                _ => (),
            },
            _ => (),
        }

        None
    }

    pub fn handle_events(&mut self) -> Result<bool> {
        let mut timeout = Duration::from_millis(20);
        while event::poll(timeout)? {
            timeout = Duration::from_millis(5);

            let event = event::read()?;
            match event {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(true);
                    }
                    _ => (),
                },
                _ => (),
            }

            let action = if self.tab_focused {
                match self.current_tab {
                    Tab::Main => self.main_tab.handle_event(event),
                    Tab::Memory => None,
                    Tab::Blocks => None,
                }
            } else {
                self.handle_event_tabs(event)
            };

            let Some(action) = action else { continue };
            match action {
                Action::AddBreakpoint(addr) => {
                    self.runner.with_state(|s| {
                        let breakpoints = s.breakpoints_mut();
                        if !breakpoints.contains(&addr) {
                            breakpoints.push(addr)
                        }
                    });
                }
                Action::RemoveBreakpoint(index) => {
                    self.runner.with_state(|s| {
                        let breakpoints = s.breakpoints_mut();
                        breakpoints.remove(index);
                    });
                }
                Action::RunStep => {
                    if !self.runner.running() {
                        self.runner.with_state(|s| {
                            debug!("stepping at {}", s.hemisphere().system.cpu.pc);
                            s.hemisphere_mut().step();
                        });
                    }
                }
                Action::RunToggle => {
                    let running = self.runner.running();
                    self.runner.set_run(!running);
                }
                Action::Unfocus => self.tab_focused = false,
            }
        }

        Ok(false)
    }
}

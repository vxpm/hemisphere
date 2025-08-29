use crossterm::event::{Event, KeyCode};
use eyre_pretty::eyre::Result;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::Action;

pub enum Tab {
    Main,
}

impl Tab {
    fn render_main(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(frame.area());

        frame.render_widget("hi", chunks[0]);
        frame.render_widget("hi", chunks[1]);
        frame.render_widget("hi", chunks[2]);
    }

    pub fn render(&mut self, frame: &mut Frame) {
        match self {
            Self::Main => self.render_main(frame),
        }
    }

    fn handle_event_main(&mut self, event: Event) -> Result<Action> {
        match event {
            _ => (),
        }

        Ok(Action::None)
    }

    pub fn handle_event(&mut self, event: Event) -> Result<Action> {
        if let Event::Key(key) = event
            && key.code == KeyCode::Esc
        {
            return Ok(Action::Quit);
        }

        match self {
            Self::Main => self.handle_event_main(event),
        }
    }
}

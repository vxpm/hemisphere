use crate::app::{Action, border_style, tab::Context};
use hemisphere::FREQUENCY;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    symbols,
    text::Text,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
};
use std::collections::VecDeque;

#[derive(Default)]
pub struct StatusPane {
    average_cps: VecDeque<f32>,
}

impl StatusPane {
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('s') => Some(Action::RunStep),
            KeyCode::Char('r') => Some(Action::RunToggle),
            _ => None,
        }
    }

    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let block = Block::bordered()
            .title("Status")
            .border_style(border_style(focused));
        let inner = block.inner(area);
        ctx.frame.render_widget(block, area);

        let [status_area, cps_area, chart_area] = Layout::horizontal([
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Min(1),
        ])
        .areas(inner);

        let status = if ctx.running {
            Text::styled("⏵ Running", Style::new().green())
        } else {
            Text::styled("⏸ Paused", Style::new().red())
        };

        let avg_cps = (ctx.state.stats().cps.iter().sum::<f32>()
            / ctx.state.stats().cps.len().max(1) as f32)
            .abs();
        let avg_mcps = avg_cps / 1_000_000.0;
        let ratio = avg_cps / FREQUENCY as f32;
        let cps = Text::styled(
            format!("{:.02} MCPS ({:.02}x)", avg_mcps, ratio),
            Style::new().light_blue(),
        );

        if ctx.running {
            if self.average_cps.len() >= 128 {
                self.average_cps.pop_front();
            }
            self.average_cps.push_back(avg_mcps);
        }

        let cps_data = self
            .average_cps
            .iter()
            .enumerate()
            .map(|(i, x)| (i as f64, *x as f64))
            .collect::<Vec<_>>();

        let cps_dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().magenta())
            .data(&cps_data);

        let x_axis = Axis::default().bounds([0.0, 127.0]);
        let y_axis = Axis::default().bounds([0.0, 486.0]);
        let chart = Chart::new(vec![cps_dataset]).x_axis(x_axis).y_axis(y_axis);

        ctx.frame.render_widget(status, status_area);
        ctx.frame.render_widget(cps, cps_area);
        ctx.frame.render_widget(chart, chart_area);
    }
}

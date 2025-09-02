use crate::app::{border_style, tab::Context};
use hemisphere::FREQUENCY;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    symbols,
    text::Text,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
};
use std::collections::VecDeque;

#[derive(Default)]
pub struct StatusPane {
    average_ips: VecDeque<f32>,
}

impl StatusPane {
    pub fn render(&mut self, ctx: &mut Context, area: Rect, focused: bool) {
        let block = Block::bordered()
            .title("Status")
            .border_style(border_style(focused));
        let inner = block.inner(area);
        ctx.frame.render_widget(block, area);

        let [status_area, ips_area, chart_area] = Layout::horizontal([
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Min(1),
        ])
        .areas(inner);

        let status = if ctx.control.running {
            Text::styled("⏵ Running", Style::new().green())
        } else {
            Text::styled("⏸ Paused", Style::new().red())
        };

        let avg_ips = (ctx.state.stats().ips.iter().sum::<f32>()
            / ctx.state.stats().ips.len().max(1) as f32)
            .abs();
        let avg_mips = avg_ips / 1_000_000.0;
        let ratio = avg_ips / FREQUENCY as f32;
        let ips = Text::styled(
            format!("{:.02} MIPS ({:.02}x)", avg_mips, ratio),
            Style::new().light_blue(),
        );

        if ctx.control.running {
            if self.average_ips.len() >= 128 {
                self.average_ips.pop_front();
            }
            self.average_ips.push_back(avg_mips);
        }

        let ips_data = self
            .average_ips
            .iter()
            .enumerate()
            .map(|(i, x)| (i as f64, *x as f64))
            .collect::<Vec<_>>();

        let ips_dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().magenta())
            .data(&ips_data);

        let x_axis = Axis::default().bounds([0.0, 127.0]);
        let y_axis = Axis::default().bounds([0.0, 486.0]);
        let chart = Chart::new(vec![ips_dataset]).x_axis(x_axis).y_axis(y_axis);

        ctx.frame.render_widget(status, status_area);
        ctx.frame.render_widget(ips, ips_area);
        ctx.frame.render_widget(chart, chart_area);
    }
}

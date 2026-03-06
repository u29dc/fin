use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Span,
    widgets::{BarChart, Block, Borders, Cell, LineGauge, Paragraph, Row, Table},
};

use crate::{
    app::App,
    fetch::CategoriesDashboardPayload,
    palette::{ACCENT_1, ACCENT_2, ACCENT_3},
    ui::widgets::{format_minor_compact, sparkline_text, truncate_text},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, payload: &CategoriesDashboardPayload, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(4)])
        .split(area);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(sections[0]);

    let bars = payload
        .pareto
        .iter()
        .map(|point| {
            (
                truncate_text(&point.category, 18),
                (point.total_minor.abs() / 100).max(0) as u64,
            )
        })
        .collect::<Vec<_>>();
    let bar_refs = bars
        .iter()
        .map(|(label, value)| (label.as_str(), *value))
        .collect::<Vec<_>>();
    let pareto_max = bars.iter().map(|(_, value)| *value).max().unwrap_or(1);
    let pareto_chart = BarChart::default()
        .data(&bar_refs)
        .bar_width(7)
        .bar_gap(1)
        .bar_style(app.theme.body.fg(ACCENT_2))
        .value_style(app.theme.body.fg(ACCENT_1))
        .label_style(app.theme.footer_meta)
        .max(pareto_max)
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            format!(" Pareto ({}, {}m) ", payload.group_id, payload.months.len()),
            app.theme.section_heading,
        )));
    frame.render_widget(pareto_chart, top[0]);

    let stability_header = Row::new(vec![
        Cell::from("category").style(app.theme.section_heading),
        Cell::from("trend").style(app.theme.section_heading),
        Cell::from("total").style(app.theme.section_heading),
    ]);
    let stability_rows = payload.stability.iter().map(|row| {
        Row::new(vec![
            Cell::from(truncate_text(&row.category, 14)),
            Cell::from(sparkline_text(&row.month_values_minor)),
            Cell::from(format_minor_compact(row.total_minor)),
        ])
    });
    let stability_table = Table::new(
        stability_rows,
        [
            Constraint::Length(15),
            Constraint::Length(8),
            Constraint::Length(12),
        ],
    )
    .header(stability_header)
    .column_spacing(1)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        " Category Stability (6 Full Months) ",
        app.theme.section_heading,
    )));
    frame.render_widget(stability_table, top[1]);

    let leakage_ratio = (payload.leakage.leakage_pct / 100.0).clamp(0.0, 1.0);
    let top = payload.pareto.first();
    let leakage = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(sections[1]);
    frame.render_widget(
        Paragraph::new(format!(
            "Uncategorized leakage: {} ({:.2}%) count={} total_expense={} | top={} ({:.1}%, {} txns)",
            format_minor_compact(payload.leakage.uncategorized_minor),
            payload.leakage.leakage_pct,
            payload.leakage.uncategorized_count,
            format_minor_compact(payload.leakage.total_expense_minor),
            top.map(|point| point.category.as_str()).unwrap_or("n/a"),
            top.map(|point| point.share_pct).unwrap_or(0.0),
            top.map(|point| point.transaction_count).unwrap_or(0),
        ))
        .style(app.theme.footer_meta),
        leakage[0],
    );
    frame.render_widget(
        LineGauge::default()
            .ratio(leakage_ratio)
            .filled_style(app.theme.body.fg(ACCENT_3))
            .unfilled_style(app.theme.footer_meta)
            .line_set(ratatui::symbols::line::THICK)
            .label(Span::styled(
                format!("{:.2}% uncategorized", payload.leakage.leakage_pct),
                app.theme.footer_meta,
            )),
        leakage[1],
    );
}

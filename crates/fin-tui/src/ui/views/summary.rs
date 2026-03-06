use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, LineGauge, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::App,
    fetch::SummaryDashboardPayload,
    palette::ACCENT_2,
    ui::widgets::{format_minor_compact, render_empty_state},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, payload: &SummaryDashboardPayload, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(8)])
        .split(area);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(sections[0]);

    let kpi_header = Row::new(vec![
        Cell::from("group").style(app.theme.section_heading),
        Cell::from("net worth").style(app.theme.section_heading),
        Cell::from("last net").style(app.theme.section_heading),
        Cell::from("avg6 net").style(app.theme.section_heading),
        Cell::from("runway").style(app.theme.section_heading),
        Cell::from("available").style(app.theme.section_heading),
    ]);
    let kpi_rows = payload.group_rows.iter().map(|row| {
        Row::new(vec![
            Cell::from(row.group_id.clone()),
            Cell::from(format_minor_compact(row.net_worth_minor)),
            Cell::from(
                row.last_full_month_net_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
            ),
            Cell::from(
                row.avg_six_month_net_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
            ),
            Cell::from(
                row.runway_months
                    .map(|value| format!("{value:.1}m"))
                    .unwrap_or_else(|| "n/a".to_owned()),
            ),
            Cell::from(
                row.available_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
            ),
        ])
    });
    let kpi_table = Table::new(
        kpi_rows,
        [
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(9),
            Constraint::Length(12),
        ],
    )
    .header(kpi_header)
    .column_spacing(1)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        " Group Scoreboard ",
        app.theme.section_heading,
    )));
    frame.render_widget(kpi_table, top[0]);

    let summary_lines = vec![
        Line::from(vec![
            Span::styled("Consolidated ", app.theme.section_heading),
            Span::styled(
                format_minor_compact(payload.consolidated_net_worth_minor),
                app.theme.body,
            ),
        ]),
        Line::from(vec![
            Span::styled("Generated ", app.theme.section_heading),
            Span::styled(payload.generated_at.clone(), app.theme.footer_meta),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Coverage ", app.theme.section_heading),
            Span::styled(
                format!("{} groups", payload.group_rows.len()),
                app.theme.footer_meta,
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(summary_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(" Snapshot ", app.theme.section_heading)),
            )
            .wrap(Wrap { trim: false }),
        top[1],
    );

    if payload.reserve_rows.is_empty() {
        render_empty_state(frame, sections[1], "No reserve rows.", app.theme);
        return;
    }

    let reserve_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(3); payload.reserve_rows.len()].as_slice())
        .split(sections[1]);
    for (index, row) in payload.reserve_rows.iter().enumerate() {
        if index >= reserve_sections.len() {
            break;
        }
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(reserve_sections[index]);
        let title = format!(
            "{} | runway {} | available {} | reserve target {}",
            row.group_id,
            row.runway_months
                .map(|value| format!("{value:.1}m"))
                .unwrap_or_else(|| "n/a".to_owned()),
            format_minor_compact(row.available_minor),
            format_minor_compact(row.target_minor),
        );
        frame.render_widget(Paragraph::new(title).style(app.theme.footer_meta), inner[0]);

        let ratio = if row.target_minor <= 0 {
            if row.available_minor > 0 { 1.0 } else { 0.0 }
        } else {
            (row.available_minor as f64 / row.target_minor as f64).clamp(0.0, 1.0)
        };
        let gauge = LineGauge::default()
            .ratio(ratio)
            .filled_style(app.theme.body.fg(ACCENT_2))
            .unfilled_style(app.theme.footer_meta)
            .line_set(ratatui::symbols::line::THICK)
            .label(Span::styled(
                format!(
                    "expense {} + tax {}",
                    format_minor_compact(row.expense_reserve_minor),
                    format_minor_compact(row.tax_reserve_minor)
                ),
                app.theme.footer_meta,
            ));
        frame.render_widget(gauge, inner[1]);
    }
}

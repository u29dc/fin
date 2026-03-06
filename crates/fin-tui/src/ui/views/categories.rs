use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::App,
    fetch::CategoriesDashboardPayload,
    palette::ACCENT_3,
    ui::widgets::{format_minor_compact, render_empty_state, sparkline_text, truncate_text},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, payload: &CategoriesDashboardPayload, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(area);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(sections[0]);
    let bottom = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(4)])
        .split(sections[1]);

    render_pareto(frame, top[0], payload, app);
    render_hierarchy(frame, top[1], payload, app);
    render_flow(frame, bottom[0], payload, app);
    render_leakage(frame, bottom[1], payload, app);
}

fn render_pareto(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &CategoriesDashboardPayload,
    app: &App,
) {
    if payload.pareto.is_empty() {
        render_empty_state(frame, area, "No pareto categories.", app.theme);
        return;
    }

    let header = Row::new(vec![
        Cell::from("category").style(app.theme.section_heading),
        Cell::from("trend").style(app.theme.section_heading),
        Cell::from("txns").style(app.theme.section_heading),
        Cell::from("share").style(app.theme.section_heading),
        Cell::from("total").style(app.theme.section_heading),
    ]);
    let rows = payload.pareto.iter().map(|point| {
        let stability = payload
            .stability
            .iter()
            .find(|row| row.category == point.category);
        let trend = stability
            .map(|row| sparkline_text(&row.month_values_minor))
            .unwrap_or_else(|| "-".to_owned());
        let total_minor = stability
            .map(|row| row.total_minor)
            .unwrap_or(point.total_minor);
        Row::new(vec![
            Cell::from(truncate_text(&point.category, 18)),
            Cell::from(trend),
            Cell::from(point.transaction_count.to_string()),
            Cell::from(format!("{:.1}%", point.share_pct)),
            Cell::from(format_minor_compact(total_minor)),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Min(14),
            Constraint::Length(8),
            Constraint::Length(5),
            Constraint::Length(7),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        format!(" Pareto ({}, {}m) ", payload.group_id, payload.months.len()),
        app.theme.section_heading,
    )));
    frame.render_widget(table, area);
}

fn render_hierarchy(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &CategoriesDashboardPayload,
    app: &App,
) {
    if payload.hierarchy.is_empty() {
        render_empty_state(frame, area, "No hierarchy rows.", app.theme);
        return;
    }

    let header = Row::new(vec![
        Cell::from("tree").style(app.theme.section_heading),
        Cell::from("root %").style(app.theme.section_heading),
        Cell::from("amount").style(app.theme.section_heading),
    ]);
    let rows = payload.hierarchy.iter().map(|row| {
        let indent = "  ".repeat(row.depth.min(4));
        let label = format!(
            "{indent}{}",
            truncate_text(&row.label, 22usize.saturating_sub(row.depth * 2))
        );
        Row::new(vec![
            Cell::from(label),
            Cell::from(format!("{:.1}%", row.share_of_root_pct)),
            Cell::from(format_minor_compact(row.total_minor)),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Min(18),
            Constraint::Length(8),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Expense Tree ", app.theme.section_heading)),
    );
    frame.render_widget(table, area);
}

fn render_flow(frame: &mut Frame<'_>, area: Rect, payload: &CategoriesDashboardPayload, app: &App) {
    if payload.flow.is_empty() {
        render_empty_state(frame, area, "No flow edges.", app.theme);
        return;
    }

    let header = Row::new(vec![
        Cell::from("from").style(app.theme.section_heading),
        Cell::from("to").style(app.theme.section_heading),
        Cell::from("amount").style(app.theme.section_heading),
        Cell::from("share").style(app.theme.section_heading),
    ]);
    let rows = payload.flow.iter().map(|row| {
        Row::new(vec![
            Cell::from(truncate_text(&row.source_label, 14)),
            Cell::from(truncate_text(&row.target_label, 14)),
            Cell::from(format_minor_compact(row.amount_minor)),
            Cell::from(format!(
                "{:.0}%/{:.0}%",
                row.share_of_total_pct, row.share_of_source_pct
            )),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(15),
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Length(9),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Flow Matrix ", app.theme.section_heading)),
    );
    frame.render_widget(table, area);
}

fn render_leakage(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &CategoriesDashboardPayload,
    app: &App,
) {
    let top = payload.pareto.first();
    let lines = vec![
        Line::from(vec![
            Span::styled("uncategorized ", app.theme.section_heading),
            Span::styled(
                format!(
                    "{} ({:.2}%) count {}",
                    format_minor_compact(payload.leakage.uncategorized_minor),
                    payload.leakage.leakage_pct,
                    payload.leakage.uncategorized_count,
                ),
                if payload.leakage.leakage_pct >= 5.0 {
                    app.theme.body.fg(ACCENT_3)
                } else {
                    app.theme.body
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("expense base ", app.theme.section_heading),
            Span::styled(
                format_minor_compact(payload.leakage.total_expense_minor),
                app.theme.footer_meta,
            ),
            Span::styled(" | top category ", app.theme.section_heading),
            Span::styled(
                top.map(|point| format!("{} {:.1}%", point.category, point.share_pct))
                    .unwrap_or_else(|| "n/a".to_owned()),
                app.theme.footer_meta,
            ),
        ]),
        Line::from(vec![
            Span::styled("read order ", app.theme.section_heading),
            Span::styled(
                "pareto -> tree -> flow to trace concentration, structure, then movement",
                app.theme.footer_meta,
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(" Leakage + Guide ", app.theme.section_heading)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::App,
    fetch::ReportsDashboardPayload,
    palette::{ACCENT_1, ACCENT_3},
    ui::widgets::{format_minor_compact, render_empty_state},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, payload: &ReportsDashboardPayload, app: &App) {
    if payload.recent_months.is_empty()
        && payload.runway_snapshots.is_empty()
        && payload.reserve_snapshots.is_empty()
    {
        render_empty_state(
            frame,
            area,
            "No report snapshots for this group.",
            app.theme,
        );
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(10)])
        .split(area);

    frame.render_widget(summary_panel(payload, app), sections[0]);

    let body = if sections[1].width >= 132 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
            .split(sections[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(11), Constraint::Min(12)])
            .split(sections[1])
    };

    render_recent_months(frame, body[0], payload, app);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(7)])
        .split(body[1]);
    render_runway_panel(frame, right[0], payload, app);
    render_reserve_panel(frame, right[1], payload, app);
}

fn summary_panel(payload: &ReportsDashboardPayload, app: &App) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                format!("reports {} ", payload.group_id),
                app.theme.section_heading,
            ),
            Span::styled(
                format!(
                    "totals income {} | expense {} | net {}",
                    format_minor_compact(payload.totals_income_minor),
                    format_minor_compact(payload.totals_expense_minor),
                    signed_minor(payload.totals_net_minor),
                ),
                app.theme.body,
            ),
        ]),
        Line::from(vec![
            Span::styled("latest ", app.theme.section_heading),
            Span::styled(
                payload
                    .latest_full_month_net_minor
                    .map(signed_minor)
                    .unwrap_or_else(|| "n/a".to_owned()),
                signed_style(payload.latest_full_month_net_minor.unwrap_or_default(), app),
            ),
            Span::styled(" | avg6 ", app.theme.section_heading),
            Span::styled(
                payload
                    .avg_six_month_net_minor
                    .map(signed_minor)
                    .unwrap_or_else(|| "n/a".to_owned()),
                signed_style(payload.avg_six_month_net_minor.unwrap_or_default(), app),
            ),
            Span::styled(" | runway ", app.theme.section_heading),
            Span::styled(
                payload
                    .latest_runway_months
                    .map(|value| format!("{value:.1}m"))
                    .unwrap_or_else(|| "n/a".to_owned()),
                app.theme.body,
            ),
            Span::styled(" | available ", app.theme.section_heading),
            Span::styled(
                payload
                    .latest_available_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
                app.theme.body,
            ),
        ]),
        Line::from(vec![
            Span::styled("burn ", app.theme.section_heading),
            Span::styled(
                payload
                    .burn_rate_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
                app.theme.footer_meta,
            ),
            Span::styled(" | median expense ", app.theme.section_heading),
            Span::styled(
                payload
                    .median_expense_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
                app.theme.footer_meta,
            ),
            Span::styled(" | tax reserve ", app.theme.section_heading),
            Span::styled(
                payload
                    .latest_tax_reserve_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
                app.theme.footer_meta,
            ),
            Span::styled(" | expense reserve ", app.theme.section_heading),
            Span::styled(
                payload
                    .latest_expense_reserve_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
                app.theme.footer_meta,
            ),
        ]),
    ])
    .wrap(Wrap { trim: false })
}

fn render_recent_months(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &ReportsDashboardPayload,
    app: &App,
) {
    if payload.recent_months.is_empty() {
        render_empty_state(frame, area, "No completed months yet.", app.theme);
        return;
    }

    let header = Row::new(vec![
        Cell::from("month").style(app.theme.section_heading),
        Cell::from("income").style(app.theme.section_heading),
        Cell::from("expense").style(app.theme.section_heading),
        Cell::from("net").style(app.theme.section_heading),
        Cell::from("save").style(app.theme.section_heading),
        Cell::from("dev").style(app.theme.section_heading),
    ]);
    let rows = payload.recent_months.iter().map(|point| {
        let deviation = point
            .expense_deviation_ratio
            .map(|value| format!("{:+.0}%", (value - 1.0) * 100.0))
            .unwrap_or_else(|| "n/a".to_owned());
        Row::new(vec![
            Cell::from(point.month.clone()),
            Cell::from(format_minor_compact(point.income_minor)),
            Cell::from(format_minor_compact(point.expense_minor)),
            Cell::from(Span::styled(
                signed_minor(point.net_minor),
                signed_style(point.net_minor, app),
            )),
            Cell::from(
                point
                    .savings_rate_pct
                    .map(|value| format!("{value:.0}%"))
                    .unwrap_or_else(|| "n/a".to_owned()),
            ),
            Cell::from(Span::styled(
                deviation,
                if point.is_anomaly {
                    app.theme.body.fg(ACCENT_3)
                } else {
                    app.theme.footer_meta
                },
            )),
        ])
    });

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(7),
                Constraint::Length(8),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            " Recent Full Months ",
            app.theme.section_heading,
        ))),
        area,
    );
}

fn render_runway_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &ReportsDashboardPayload,
    app: &App,
) {
    if payload.runway_snapshots.is_empty() {
        render_empty_state(frame, area, "No runway data.", app.theme);
        return;
    }

    let header = Row::new(vec![
        Cell::from("date").style(app.theme.section_heading),
        Cell::from("runway").style(app.theme.section_heading),
        Cell::from("balance").style(app.theme.section_heading),
        Cell::from("burn").style(app.theme.section_heading),
    ]);
    let rows = payload.runway_snapshots.iter().map(|row| {
        Row::new(vec![
            Cell::from(row.date.clone()),
            Cell::from(Span::styled(
                format!("{:.1}m", row.runway_months),
                if row.runway_months < 6.0 {
                    app.theme.body.fg(ACCENT_3)
                } else {
                    app.theme.body.fg(ACCENT_1)
                },
            )),
            Cell::from(format_minor_compact(row.balance_minor)),
            Cell::from(format_minor_compact(row.burn_rate_minor)),
        ])
    });

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Runway Ladder ", app.theme.section_heading)),
        ),
        area,
    );
}

fn render_reserve_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &ReportsDashboardPayload,
    app: &App,
) {
    if payload.reserve_snapshots.is_empty() {
        render_empty_state(frame, area, "No reserve data.", app.theme);
        return;
    }

    let header = Row::new(vec![
        Cell::from("date").style(app.theme.section_heading),
        Cell::from("tax").style(app.theme.section_heading),
        Cell::from("expense").style(app.theme.section_heading),
        Cell::from("available").style(app.theme.section_heading),
    ]);
    let rows = payload.reserve_snapshots.iter().map(|row| {
        Row::new(vec![
            Cell::from(row.date.clone()),
            Cell::from(format_minor_compact(row.tax_reserve_minor)),
            Cell::from(format_minor_compact(row.expense_reserve_minor)),
            Cell::from(Span::styled(
                signed_minor(row.available_minor),
                signed_style(row.available_minor, app),
            )),
        ])
    });

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            " Reserve Snapshots ",
            app.theme.section_heading,
        ))),
        area,
    );
}

fn signed_minor(value: i64) -> String {
    if value > 0 {
        return format!("+{}", format_minor_compact(value));
    }
    format_minor_compact(value)
}

fn signed_style(value: i64, app: &App) -> ratatui::style::Style {
    if value >= 0 {
        app.theme.body.fg(ACCENT_1)
    } else {
        app.theme.body.fg(ACCENT_3)
    }
}

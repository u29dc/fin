use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::App,
    fetch::{AccountFreshnessRow, OverviewDashboardPayload},
    palette::ACCENT_3,
    ui::widgets::{format_minor_compact, render_empty_state, sparkline_text, truncate_text},
};

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    selected_row: usize,
    payload: &OverviewDashboardPayload,
    app: &App,
) {
    if payload.accounts.is_empty() {
        render_empty_state(frame, area, "No overview accounts.", app.theme);
        return;
    }

    let selected_row = selected_row.min(payload.accounts.len().saturating_sub(1));
    let selected = &payload.accounts[selected_row];
    let sections = if area.width >= 112 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(10), Constraint::Min(10)])
            .split(area)
    };

    render_accounts_table(frame, sections[0], selected_row, payload, app);
    render_detail(frame, sections[1], selected, payload, app);
}

fn render_accounts_table(
    frame: &mut Frame<'_>,
    area: Rect,
    selected_row: usize,
    payload: &OverviewDashboardPayload,
    app: &App,
) {
    let header = Row::new(vec![
        Cell::from("account").style(app.theme.section_heading),
        Cell::from("balance").style(app.theme.section_heading),
        Cell::from("days").style(app.theme.section_heading),
        Cell::from("updated").style(app.theme.section_heading),
    ]);
    let rows = payload.accounts.iter().enumerate().map(|(index, row)| {
        let stale = row
            .stale_days
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_owned());
        let updated = row
            .updated_at
            .as_deref()
            .map(|value| truncate_text(value, 10))
            .unwrap_or_else(|| "n/a".to_owned());
        let style = if index == selected_row {
            app.theme.selected
        } else if row.stale_days.unwrap_or(0) > 90 {
            app.theme.body.fg(ACCENT_3)
        } else {
            app.theme.body
        };
        Row::new(vec![
            Cell::from(truncate_text(&row.label, 18)),
            Cell::from(format_minor_compact(row.balance_minor)),
            Cell::from(stale),
            Cell::from(updated),
        ])
        .style(style)
    });

    let table = Table::new(
        rows,
        [
            Constraint::Min(16),
            Constraint::Length(11),
            Constraint::Length(6),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        format!(
            " Accounts ({}) total {} ",
            payload.scope_label,
            format_minor_compact(payload.total_balance_minor)
        ),
        app.theme.section_heading,
    )));
    frame.render_widget(table, area);
}

fn render_detail(
    frame: &mut Frame<'_>,
    area: Rect,
    selected: &AccountFreshnessRow,
    payload: &OverviewDashboardPayload,
    app: &App,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(6)])
        .split(area);

    let account_hist = history_summary(&selected.history);
    let scope_hist = history_summary(&payload.scope_history);
    let contribution_line = if selected.contributions.is_empty() {
        if selected.is_investment {
            "contrib n/a".to_owned()
        } else {
            "contrib non-investment account".to_owned()
        }
    } else {
        let values = selected
            .contributions
            .iter()
            .map(|point| point.contributions_minor)
            .collect::<Vec<_>>();
        format!(
            "contrib {} total {}",
            sparkline_text(&values),
            selected
                .contributions
                .last()
                .map(|point| format_minor_compact(point.contributions_minor))
                .unwrap_or_else(|| "n/a".to_owned()),
        )
    };

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("selected ", app.theme.section_heading),
                Span::styled(selected.label.clone(), app.theme.body),
                Span::styled(" | balance ", app.theme.section_heading),
                Span::styled(format_minor_compact(selected.balance_minor), app.theme.body),
                Span::styled(" | kind ", app.theme.section_heading),
                Span::styled(
                    if selected.is_investment {
                        "investment"
                    } else {
                        "cash"
                    },
                    app.theme.footer_meta,
                ),
            ]),
            Line::from(vec![
                Span::styled("updated ", app.theme.section_heading),
                Span::styled(
                    selected.updated_at.as_deref().unwrap_or("n/a").to_owned(),
                    app.theme.footer_meta,
                ),
                Span::styled(" | stale ", app.theme.section_heading),
                Span::styled(
                    selected
                        .stale_days
                        .map(|value| format!("{value}d"))
                        .unwrap_or_else(|| "n/a".to_owned()),
                    app.theme.footer_meta,
                ),
            ]),
            Line::from(Span::styled(account_hist, app.theme.body)),
            Line::from(Span::styled(scope_hist, app.theme.footer_meta)),
            Line::from(Span::styled(contribution_line, app.theme.footer_meta)),
        ])
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            " Selected Account ",
            app.theme.section_heading,
        )))
        .wrap(Wrap { trim: false }),
        sections[0],
    );

    render_projection(frame, sections[1], payload, app);
}

fn render_projection(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &OverviewDashboardPayload,
    app: &App,
) {
    let Some(projection) = payload.projection.as_ref() else {
        render_empty_state(
            frame,
            area,
            "No runway projection for this scope.",
            app.theme,
        );
        return;
    };

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(4)])
        .split(area);

    let summary = vec![
        Line::from(vec![
            Span::styled("liquid ", app.theme.section_heading),
            Span::styled(
                format_minor_compact(projection.liquid_balance_minor),
                app.theme.body,
            ),
            Span::styled(" | median expense ", app.theme.section_heading),
            Span::styled(
                format_minor_compact(projection.median_monthly_expense_minor),
                app.theme.body,
            ),
        ]),
        Line::from(vec![
            Span::styled("as of ", app.theme.section_heading),
            Span::styled(
                projection.assumptions.as_of_date.clone(),
                app.theme.footer_meta,
            ),
            Span::styled(" | months ", app.theme.section_heading),
            Span::styled(
                projection.assumptions.projection_months.to_string(),
                app.theme.footer_meta,
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(summary), inner[0]);

    let header = Row::new(vec![
        Cell::from("scenario").style(app.theme.section_heading),
        Cell::from("burn").style(app.theme.section_heading),
        Cell::from("breach").style(app.theme.section_heading),
        Cell::from("path").style(app.theme.section_heading),
    ]);
    let rows = projection.scenarios.iter().map(|scenario| {
        let balances = scenario
            .points
            .iter()
            .map(|point| point.balance_minor)
            .collect::<Vec<_>>();
        let breach = scenario
            .zero_balance_crossing
            .as_ref()
            .map(|crossing| crossing.date.clone())
            .unwrap_or_else(|| "none".to_owned());
        Row::new(vec![
            Cell::from(scenario.label.clone()),
            Cell::from(format_minor_compact(scenario.burn_rate_minor)),
            Cell::from(truncate_text(&breach, 10)),
            Cell::from(sparkline_text(&balances)),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(13),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Min(12),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        format!(" Projection ({}) ", payload.scope_label),
        app.theme.section_heading,
    )));
    frame.render_widget(table, inner[1]);
}

fn history_summary(points: &[fin_sdk::DailyBalancePoint]) -> String {
    if points.is_empty() {
        return "history n/a".to_owned();
    }
    let values = points
        .iter()
        .map(|point| point.balance_minor)
        .collect::<Vec<_>>();
    let latest = points
        .last()
        .map(|point| point.balance_minor)
        .unwrap_or_default();
    format!(
        "hist {} latest {}",
        sparkline_text(&values),
        format_minor_compact(latest),
    )
}

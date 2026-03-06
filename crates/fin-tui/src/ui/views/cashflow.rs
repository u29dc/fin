use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::App,
    fetch::CashflowDashboardPayload,
    palette::{ACCENT_1, ACCENT_2, ACCENT_3},
    ui::widgets::{format_minor_compact, scaled_bar},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, payload: &CashflowDashboardPayload, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Line::from(vec![
        Span::styled(format!("{} ", payload.group_id), app.theme.section_heading),
        Span::styled(
            format!(
                "latest {} | 6m avg net {} | runway {}",
                payload
                    .latest_full_month
                    .as_ref()
                    .map(|point| format_minor_compact(point.net_minor))
                    .unwrap_or_else(|| "n/a".to_owned()),
                payload
                    .avg_six_month_net_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
                payload
                    .runway_months
                    .map(|value| format!("{value:.1}m"))
                    .unwrap_or_else(|| "n/a".to_owned())
            ),
            app.theme.footer_meta,
        ),
    ]);
    let averages = Line::from(vec![
        Span::styled("6m avg income ", app.theme.section_heading),
        Span::styled(
            payload
                .avg_six_month_income_minor
                .map(format_minor_compact)
                .unwrap_or_else(|| "n/a".to_owned()),
            app.theme.body,
        ),
        Span::styled(" | 6m avg expense ", app.theme.section_heading),
        Span::styled(
            payload
                .avg_six_month_expense_minor
                .map(format_minor_compact)
                .unwrap_or_else(|| "n/a".to_owned()),
            app.theme.body,
        ),
    ]);
    frame.render_widget(Paragraph::new(vec![header, averages]), sections[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(sections[1]);

    render_monthly_net_bars(frame, body[0], payload, app);
    render_income_expense_bars(frame, body[1], payload, app);

    let reserve_line = Line::from(vec![
        Span::styled("available ", app.theme.section_heading),
        Span::styled(
            payload
                .available_minor
                .map(format_minor_compact)
                .unwrap_or_else(|| "n/a".to_owned()),
            app.theme.body,
        ),
        Span::styled(" | expense reserve ", app.theme.section_heading),
        Span::styled(
            payload
                .expense_reserve_minor
                .map(format_minor_compact)
                .unwrap_or_else(|| "n/a".to_owned()),
            app.theme.body,
        ),
        Span::styled(" | tax reserve ", app.theme.section_heading),
        Span::styled(
            payload
                .tax_reserve_minor
                .map(format_minor_compact)
                .unwrap_or_else(|| "n/a".to_owned()),
            app.theme.body,
        ),
    ]);
    frame.render_widget(
        Paragraph::new(reserve_line)
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                " Runway + Reserve ",
                app.theme.section_heading,
            )))
            .wrap(Wrap { trim: false }),
        sections[2],
    );
}

fn render_monthly_net_bars(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &CashflowDashboardPayload,
    app: &App,
) {
    let max_abs = payload
        .points
        .iter()
        .map(|point| point.net_minor.abs())
        .max()
        .unwrap_or(1)
        .max(1);
    let bar_width = area.width.saturating_sub(22) as usize;
    let header = Row::new(vec![
        Cell::from("month").style(app.theme.section_heading),
        Cell::from("net").style(app.theme.section_heading),
        Cell::from("bar").style(app.theme.section_heading),
    ]);
    let rows = payload.points.iter().map(|point| {
        let bar = scaled_bar(point.net_minor.abs(), max_abs, bar_width.max(2));
        let sign = if point.net_minor >= 0 { "+" } else { "-" };
        let mut row = Row::new(vec![
            Cell::from(point.month.clone()),
            Cell::from(format_minor_compact(point.net_minor)),
            Cell::from(format!("{sign}{bar}")),
        ]);
        row = if point.net_minor >= 0 {
            row.style(app.theme.body.fg(ACCENT_2))
        } else {
            row.style(app.theme.body.fg(ACCENT_3))
        };
        row
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        " Monthly Net (Diverging Bars) ",
        app.theme.section_heading,
    )));
    frame.render_widget(table, area);
}

fn render_income_expense_bars(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &CashflowDashboardPayload,
    app: &App,
) {
    let max_income = payload
        .points
        .iter()
        .map(|point| point.income_minor)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_expense = payload
        .points
        .iter()
        .map(|point| point.expense_minor)
        .max()
        .unwrap_or(1)
        .max(1);
    let bar_width = area.width.saturating_sub(34) as usize;

    let header = Row::new(vec![
        Cell::from("month").style(app.theme.section_heading),
        Cell::from("income").style(app.theme.section_heading),
        Cell::from("expense").style(app.theme.section_heading),
    ]);
    let rows = payload.points.iter().map(|point| {
        Row::new(vec![
            Cell::from(point.month.clone()),
            Cell::from(format!(
                "{} {}",
                scaled_bar(point.income_minor, max_income, bar_width.max(2)),
                format_minor_compact(point.income_minor)
            ))
            .style(app.theme.body.fg(ACCENT_1)),
            Cell::from(format!(
                "{} {}",
                scaled_bar(point.expense_minor, max_expense, bar_width.max(2)),
                format_minor_compact(point.expense_minor)
            ))
            .style(app.theme.body.fg(ACCENT_3)),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        " Income vs Expense (12 Full Months) ",
        app.theme.section_heading,
    )));
    frame.render_widget(table, area);
}

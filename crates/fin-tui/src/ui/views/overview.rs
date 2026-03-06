use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Span,
    widgets::{BarChart, Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::{
    app::App,
    fetch::OverviewDashboardPayload,
    palette::{ACCENT_2, ACCENT_3},
    ui::widgets::{format_minor_compact, label_value_line, truncate_text},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, payload: &OverviewDashboardPayload, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let bars = payload
        .accounts
        .iter()
        .map(|row| {
            (
                truncate_text(&row.label, 18),
                ((row.balance_minor.abs() / 100).max(0) as u64).max(1),
            )
        })
        .collect::<Vec<_>>();
    let bar_refs = bars
        .iter()
        .map(|(label, value)| (label.as_str(), *value))
        .collect::<Vec<_>>();
    let max_value = bars.iter().map(|(_, value)| *value).max().unwrap_or(1);
    let account_chart = BarChart::default()
        .data(&bar_refs)
        .bar_width(7)
        .bar_gap(1)
        .bar_style(app.theme.body.fg(ACCENT_2))
        .value_style(app.theme.body.fg(ACCENT_2))
        .label_style(app.theme.footer_meta)
        .max(max_value)
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            format!(
                " Account Balances ({}) total {} ",
                payload.scope_label,
                format_minor_compact(payload.total_balance_minor)
            ),
            app.theme.section_heading,
        )));
    frame.render_widget(account_chart, sections[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(5)])
        .split(sections[1]);
    frame.render_widget(
        Paragraph::new(vec![
            label_value_line("scope ", payload.scope_label.clone(), app.theme),
            label_value_line("accounts ", payload.accounts.len().to_string(), app.theme),
            label_value_line(
                "total ",
                format_minor_compact(payload.total_balance_minor),
                app.theme,
            ),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Scope Snapshot ", app.theme.section_heading)),
        ),
        right[0],
    );

    let freshness_header = Row::new(vec![
        Cell::from("account").style(app.theme.section_heading),
        Cell::from("days").style(app.theme.section_heading),
        Cell::from("updated").style(app.theme.section_heading),
    ]);
    let freshness_rows = payload.accounts.iter().map(|row| {
        let days = row
            .stale_days
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_owned());
        let updated = row
            .updated_at
            .as_deref()
            .map(|value| truncate_text(value, 10))
            .unwrap_or_else(|| "n/a".to_owned());
        let mut rendered = Row::new(vec![
            Cell::from(truncate_text(&row.label, 16)),
            Cell::from(days.clone()),
            Cell::from(updated),
        ]);
        if let Ok(parsed) = days.parse::<i64>()
            && parsed > 90
        {
            rendered = rendered.style(app.theme.body.fg(ACCENT_3));
        }
        rendered
    });
    let freshness_table = Table::new(
        freshness_rows,
        [
            Constraint::Length(17),
            Constraint::Length(6),
            Constraint::Length(11),
        ],
    )
    .header(freshness_header)
    .column_spacing(1)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        " Data Freshness (Account Updates) ",
        app.theme.section_heading,
    )));
    frame.render_widget(freshness_table, right[1]);
}

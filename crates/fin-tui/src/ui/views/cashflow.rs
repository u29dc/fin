use fin_sdk::ShortTermTrend;
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
            Constraint::Length(5),
        ])
        .split(area);

    let trend = trend_label(payload.short_term_trend);
    let recent_anomalies = if payload.recent_anomaly_months.is_empty() {
        "stable".to_owned()
    } else {
        payload.recent_anomaly_months.join(", ")
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(format!("{} ", payload.group_id), app.theme.section_heading),
                Span::styled(
                    format!(
                        "latest {} | 6m avg net {} | runway {}",
                        payload
                            .latest_full_month
                            .as_ref()
                            .map(|point| signed_minor(point.net_minor))
                            .unwrap_or_else(|| "n/a".to_owned()),
                        payload
                            .avg_six_month_net_minor
                            .map(signed_minor)
                            .unwrap_or_else(|| "n/a".to_owned()),
                        payload
                            .runway_months
                            .map(|value| format!("{value:.1}m"))
                            .unwrap_or_else(|| "n/a".to_owned()),
                    ),
                    app.theme.footer_meta,
                ),
            ]),
            Line::from(vec![
                Span::styled("median spend ", app.theme.section_heading),
                Span::styled(
                    payload
                        .median_spend_minor
                        .map(format_minor_compact)
                        .unwrap_or_else(|| "n/a".to_owned()),
                    app.theme.body,
                ),
                Span::styled(" | trend ", app.theme.section_heading),
                Span::styled(trend.0, trend.1),
                Span::styled(" | anomalies ", app.theme.section_heading),
                Span::styled(
                    format!(
                        "{} ({recent_anomalies})",
                        payload.anomaly_count_last_12_months
                    ),
                    if payload.anomaly_count_last_12_months > 0 {
                        app.theme.body.fg(ACCENT_3)
                    } else {
                        app.theme.footer_meta
                    },
                ),
            ]),
        ]),
        sections[0],
    );

    render_grouped_monthly_bars(frame, sections[1], payload, app);
    render_footer(frame, sections[2], payload, app);
}

fn render_grouped_monthly_bars(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &CashflowDashboardPayload,
    app: &App,
) {
    let max_flow = payload
        .points
        .iter()
        .flat_map(|point| [point.income_minor, point.expense_minor])
        .max()
        .unwrap_or(1)
        .max(1);
    let max_net = payload
        .points
        .iter()
        .map(|point| point.net_minor.abs())
        .max()
        .unwrap_or(1)
        .max(1);
    let flow_col_width = usize::from(area.width.saturating_sub(46) / 2).max(12);
    let flow_bar_width = flow_col_width.saturating_sub(7).max(2);
    let net_bar_width = 8usize;

    let header = Row::new(vec![
        Cell::from("month").style(app.theme.section_heading),
        Cell::from("income").style(app.theme.section_heading),
        Cell::from("expense").style(app.theme.section_heading),
        Cell::from("net").style(app.theme.section_heading),
        Cell::from("save").style(app.theme.section_heading),
        Cell::from("dev").style(app.theme.section_heading),
    ]);
    let rows = payload.points.iter().map(|point| {
        let income_cell = format!(
            "{} {}",
            scaled_bar(point.income_minor, max_flow, flow_bar_width),
            format_minor_compact(point.income_minor)
        );
        let expense_cell = format!(
            "{} {}",
            scaled_bar(point.expense_minor, max_flow, flow_bar_width),
            format_minor_compact(point.expense_minor)
        );
        let net_cell = format!(
            "{} {}",
            scaled_bar(point.net_minor.abs(), max_net, net_bar_width),
            signed_minor(point.net_minor)
        );
        let savings = point
            .savings_rate_pct
            .map(|value| format!("{value:.0}%"))
            .unwrap_or_else(|| "n/a".to_owned());
        let deviation = deviation_label(point.expense_deviation_ratio, point.is_anomaly);
        Row::new(vec![
            Cell::from(point.month.clone()),
            Cell::from(Span::styled(income_cell, app.theme.body.fg(ACCENT_2))),
            Cell::from(Span::styled(expense_cell, app.theme.body.fg(ACCENT_3))),
            Cell::from(Span::styled(
                net_cell,
                if point.net_minor >= 0 {
                    app.theme.body.fg(ACCENT_1)
                } else {
                    app.theme.body.fg(ACCENT_3)
                },
            )),
            Cell::from(Span::styled(savings, app.theme.body)),
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

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(flow_col_width as u16),
            Constraint::Min(flow_col_width as u16),
            Constraint::Length(14),
            Constraint::Length(7),
            Constraint::Length(9),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        " Grouped Monthly Bars (Income, Expense, Net, Deviation) ",
        app.theme.section_heading,
    )));
    frame.render_widget(table, area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, payload: &CashflowDashboardPayload, app: &App) {
    let latest = payload.latest_full_month.as_ref();
    let recent_anomalies = if payload.recent_anomaly_months.is_empty() {
        "stable last 12m".to_owned()
    } else {
        payload.recent_anomaly_months.join(", ")
    };

    let latest_line = match latest {
        Some(point) => format!(
            "latest {} | income {} | expense {} | net {} | rolling median {}",
            point.month,
            format_minor_compact(point.income_minor),
            format_minor_compact(point.expense_minor),
            signed_minor(point.net_minor),
            point
                .rolling_median_expense_minor
                .map(format_minor_compact)
                .unwrap_or_else(|| "n/a".to_owned()),
        ),
        None => "latest n/a".to_owned(),
    };
    let averages_line = format!(
        "avg6 income {} | avg6 expense {} | avg6 net {} | available {}",
        payload
            .avg_six_month_income_minor
            .map(format_minor_compact)
            .unwrap_or_else(|| "n/a".to_owned()),
        payload
            .avg_six_month_expense_minor
            .map(format_minor_compact)
            .unwrap_or_else(|| "n/a".to_owned()),
        payload
            .avg_six_month_net_minor
            .map(signed_minor)
            .unwrap_or_else(|| "n/a".to_owned()),
        payload
            .available_minor
            .map(format_minor_compact)
            .unwrap_or_else(|| "n/a".to_owned()),
    );
    let watch_line = format!(
        "expense reserve {} | tax reserve {} | anomaly watch {} | income light / expense dark",
        payload
            .expense_reserve_minor
            .map(format_minor_compact)
            .unwrap_or_else(|| "n/a".to_owned()),
        payload
            .tax_reserve_minor
            .map(format_minor_compact)
            .unwrap_or_else(|| "n/a".to_owned()),
        recent_anomalies,
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(latest_line, app.theme.body)),
            Line::from(Span::styled(averages_line, app.theme.footer_meta)),
            Line::from(Span::styled(watch_line, app.theme.footer_meta)),
        ])
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            " Reserve + Watchlist ",
            app.theme.section_heading,
        )))
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn trend_label(trend: Option<ShortTermTrend>) -> (String, ratatui::style::Style) {
    match trend {
        Some(ShortTermTrend::Positive) => (
            "up".to_owned(),
            ratatui::style::Style::default().fg(ACCENT_2),
        ),
        Some(ShortTermTrend::Negative) => (
            "down".to_owned(),
            ratatui::style::Style::default().fg(ACCENT_3),
        ),
        Some(ShortTermTrend::Flat) => (
            "flat".to_owned(),
            ratatui::style::Style::default().fg(ACCENT_1),
        ),
        None => (
            "n/a".to_owned(),
            ratatui::style::Style::default().fg(ACCENT_1),
        ),
    }
}

fn signed_minor(value: i64) -> String {
    if value > 0 {
        return format!("+{}", format_minor_compact(value));
    }
    format_minor_compact(value)
}

fn deviation_label(ratio: Option<f64>, anomaly: bool) -> String {
    match ratio {
        Some(ratio) => {
            let delta_pct = (ratio - 1.0) * 100.0;
            if anomaly {
                format!("{delta_pct:+.0}% !")
            } else {
                format!("{delta_pct:+.0}%")
            }
        }
        None => "n/a".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::deviation_label;

    #[test]
    fn deviation_label_marks_anomalies_explicitly() {
        assert_eq!(deviation_label(Some(1.24), true), "+24% !");
        assert_eq!(deviation_label(Some(0.82), false), "-18%");
        assert_eq!(deviation_label(None, false), "n/a");
    }
}

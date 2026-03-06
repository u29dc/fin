use fin_sdk::{AllocationBucket, ShortTermTrend};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    app::App,
    fetch::{SummaryAllocation, SummaryDashboardPayload, SummaryGroupPanel, SummaryMonthSnapshot},
    palette::{ACCENT_1, ACCENT_2, ACCENT_3},
    ui::widgets::{format_minor_compact, proportional_widths, render_empty_state, truncate_text},
};

const KPI_CARD_HEIGHT: u16 = 6;

pub fn render(frame: &mut Frame<'_>, area: Rect, payload: &SummaryDashboardPayload, app: &App) {
    if payload.groups.is_empty() {
        render_empty_state(frame, area, "No group summaries.", app.theme);
        return;
    }

    let kpi_columns = choose_kpi_columns(area.width, payload.groups.len());
    let kpi_rows = payload.groups.len().div_ceil(kpi_columns);
    let kpi_height = (KPI_CARD_HEIGHT * u16::try_from(kpi_rows).unwrap_or(1))
        .min(area.height.saturating_sub(8))
        .max(KPI_CARD_HEIGHT.min(area.height));

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(kpi_height),
            Constraint::Min(8),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("summary ", app.theme.section_heading),
                Span::styled(
                    format!(
                        "consolidated {} | groups {}",
                        format_minor_compact(payload.consolidated_net_worth_minor),
                        payload.groups.len()
                    ),
                    app.theme.body,
                ),
            ]),
            Line::from(vec![
                Span::styled("focus ", app.theme.section_heading),
                Span::styled(
                    "runway | last net | avg6 | median spend | allocation | watchlist",
                    app.theme.footer_meta,
                ),
                Span::styled(" | generated ", app.theme.section_heading),
                Span::styled(
                    truncate_text(&payload.generated_at, 36),
                    app.theme.footer_meta,
                ),
            ]),
        ]),
        sections[0],
    );

    render_group_grid(
        frame,
        sections[1],
        &payload.groups,
        kpi_columns,
        render_kpi_card,
        app,
    );

    let detail_columns = choose_detail_columns(sections[2].width, payload.groups.len());
    render_group_grid(
        frame,
        sections[2],
        &payload.groups,
        detail_columns,
        render_detail_card,
        app,
    );
}

fn choose_kpi_columns(width: u16, group_count: usize) -> usize {
    if group_count <= 1 {
        return 1;
    }
    if width >= 132 {
        return 3.min(group_count);
    }
    if width >= 88 {
        return 2.min(group_count);
    }
    1
}

fn choose_detail_columns(width: u16, group_count: usize) -> usize {
    if group_count <= 1 {
        return 1;
    }
    if width >= 156 {
        return 3.min(group_count);
    }
    if width >= 104 {
        return 2.min(group_count);
    }
    1
}

fn render_group_grid(
    frame: &mut Frame<'_>,
    area: Rect,
    groups: &[SummaryGroupPanel],
    columns: usize,
    renderer: fn(&mut Frame<'_>, Rect, &SummaryGroupPanel, &App),
    app: &App,
) {
    let columns = columns.max(1);
    let row_count = groups.len().div_ceil(columns);
    let row_constraints = vec![Constraint::Ratio(1, row_count as u32); row_count];
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);

    for (row_index, row_area) in rows.iter().enumerate() {
        let start = row_index * columns;
        let end = (start + columns).min(groups.len());
        let visible = end.saturating_sub(start).max(1);
        let column_constraints = vec![Constraint::Ratio(1, visible as u32); visible];
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(column_constraints)
            .split(*row_area);

        for (col_index, rect) in cols.iter().enumerate() {
            let group_index = start + col_index;
            if let Some(group) = groups.get(group_index) {
                renderer(frame, *rect, group, app);
            }
        }
    }
}

fn render_kpi_card(frame: &mut Frame<'_>, area: Rect, group: &SummaryGroupPanel, app: &App) {
    let trend = trend_badge(group.short_term_trend);
    let reserve_status = if group.allocation.under_reserved {
        format!(
            "short {}",
            format_minor_compact(group.allocation.shortfall_minor)
        )
    } else {
        "reserve ok".to_owned()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("runway ", app.theme.section_heading),
            Span::styled(format_runway(group.runway_months), app.theme.body),
            Span::styled(" | trend ", app.theme.section_heading),
            Span::styled(trend.0, trend.1),
            Span::styled(" | anomalies ", app.theme.section_heading),
            Span::styled(
                group.anomaly_count_last_12_months.to_string(),
                if group.anomaly_count_last_12_months > 0 {
                    app.theme.body.fg(ACCENT_3)
                } else {
                    app.theme.body
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("worth ", app.theme.section_heading),
            Span::styled(format_minor_compact(group.net_worth_minor), app.theme.body),
            Span::styled(" | avail ", app.theme.section_heading),
            Span::styled(
                group
                    .available_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
                app.theme.body,
            ),
        ]),
        Line::from(vec![
            Span::styled("last ", app.theme.section_heading),
            Span::styled(
                signed_minor(group.last_full_month_net_minor),
                signed_value_style(app, group.last_full_month_net_minor),
            ),
            Span::styled(" | avg6 ", app.theme.section_heading),
            Span::styled(
                signed_minor(group.avg_six_month_net_minor),
                signed_value_style(app, group.avg_six_month_net_minor),
            ),
        ]),
        Line::from(vec![
            Span::styled("median spend ", app.theme.section_heading),
            Span::styled(
                group
                    .median_spend_minor
                    .map(format_minor_compact)
                    .unwrap_or_else(|| "n/a".to_owned()),
                app.theme.body,
            ),
            Span::styled(" | ", app.theme.section_heading),
            Span::styled(reserve_status, app.theme.footer_meta),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                format!(" {} [{}] ", group.label, group.group_id),
                app.theme.section_heading,
            )))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_detail_card(frame: &mut Frame<'_>, area: Rect, group: &SummaryGroupPanel, app: &App) {
    let compact = area.width < 56;
    let allocation_line = allocation_bar_line(&group.allocation, area.width, app);
    let mix_line = allocation_mix_line(group, compact, app);
    let month_line = last_month_line(group.last_month.as_ref(), compact, app);
    let watch_line = watch_line(group, compact, app);

    let lines = vec![allocation_line, mix_line, month_line, watch_line];
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                format!(" {} allocation + watch ", group.label),
                app.theme.section_heading,
            )))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn allocation_bar_line(allocation: &SummaryAllocation, width: u16, app: &App) -> Line<'static> {
    let bar_width = usize::from(width.saturating_sub(20)).max(8);
    let values = allocation
        .segments
        .iter()
        .map(|segment| segment.amount_minor)
        .collect::<Vec<_>>();
    let widths = proportional_widths(&values, bar_width);

    let mut spans = vec![Span::styled("alloc ", app.theme.section_heading)];
    if allocation.segments.is_empty() {
        spans.push(Span::styled("n/a", app.theme.footer_meta));
    } else {
        for (segment, segment_width) in allocation.segments.iter().zip(widths) {
            if segment_width == 0 {
                continue;
            }
            spans.push(Span::styled(
                "█".repeat(segment_width),
                app.theme.body.fg(bucket_color(segment.bucket)),
            ));
        }
    }
    spans.push(Span::styled(
        format!(" {}", format_minor_compact(allocation.display_total_minor)),
        app.theme.footer_meta,
    ));
    Line::from(spans)
}

fn allocation_mix_line(group: &SummaryGroupPanel, compact: bool, app: &App) -> Line<'static> {
    let allocation = &group.allocation;
    let line = if compact {
        format!(
            "basis {} | bal {} | exp {} | shown {} | tax {}",
            allocation.basis_label,
            format_minor_compact(allocation.balance_basis_minor),
            format_minor_compact(allocation.expense_reserve_minor),
            format_minor_compact(allocation.expense_reserve_display_minor),
            format_minor_compact(allocation.tax_reserve_minor),
        )
    } else {
        let savings_or_emergency = if allocation.emergency_fund_minor > 0 {
            format!(
                "emerg {}",
                format_minor_compact(allocation.emergency_fund_minor)
            )
        } else if allocation.savings_minor > 0 {
            format!("savings {}", format_minor_compact(allocation.savings_minor))
        } else {
            "savings n/a".to_owned()
        };
        format!(
            "basis {} | basis bal {} | avail {} | exp {} | shown {} | tax {} | {} | invest {}",
            allocation.basis_label,
            format_minor_compact(allocation.balance_basis_minor),
            format_minor_compact(allocation.available_minor),
            format_minor_compact(allocation.expense_reserve_minor),
            format_minor_compact(allocation.expense_reserve_display_minor),
            format_minor_compact(allocation.tax_reserve_minor),
            savings_or_emergency,
            format_minor_compact(allocation.investment_minor),
        )
    };

    Line::from(vec![Span::styled(line, app.theme.footer_meta)])
}

fn last_month_line(
    month: Option<&SummaryMonthSnapshot>,
    compact: bool,
    app: &App,
) -> Line<'static> {
    let Some(month) = month else {
        return Line::from(vec![Span::styled("last month n/a", app.theme.footer_meta)]);
    };

    let line = if compact {
        format!(
            "{} inc {} exp {} net {}",
            month.month,
            format_minor_compact(month.income_minor),
            format_minor_compact(month.expense_minor),
            signed_minor(Some(month.net_minor)),
        )
    } else {
        format!(
            "{} inc {} ({}) | exp {} ({}) | net {} ({}) | save {}",
            month.month,
            format_minor_compact(month.income_minor),
            pct_label(month.income_change_pct),
            format_minor_compact(month.expense_minor),
            pct_label(month.expense_change_pct),
            signed_minor(Some(month.net_minor)),
            pct_label(month.net_change_pct),
            month
                .savings_rate_pct
                .map(|value| format!("{value:.1}%"))
                .unwrap_or_else(|| "n/a".to_owned()),
        )
    };

    Line::from(vec![Span::styled(line, app.theme.body)])
}

fn watch_line(group: &SummaryGroupPanel, compact: bool, app: &App) -> Line<'static> {
    let recent = if group.recent_anomaly_months.is_empty() {
        "stable".to_owned()
    } else {
        group.recent_anomaly_months.join(",")
    };
    let reserve_hint = if group.allocation.under_reserved {
        format!(
            "reserve short {}",
            format_minor_compact(group.allocation.shortfall_minor)
        )
    } else {
        "reserve covered".to_owned()
    };
    let segment_labels = group
        .allocation
        .segments
        .iter()
        .map(|segment| {
            format!(
                "{} {:.0}%",
                truncate_text(&segment.label.to_ascii_lowercase(), 4),
                segment.share_pct
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let line = if compact {
        format!(
            "watch {} | recent {} | {}",
            group.anomaly_count_last_12_months, recent, reserve_hint
        )
    } else {
        format!(
            "watch {} anomalies | recent {} | {} | mix {}",
            group.anomaly_count_last_12_months, recent, reserve_hint, segment_labels
        )
    };
    Line::from(vec![Span::styled(line, app.theme.footer_meta)])
}

fn bucket_color(bucket: AllocationBucket) -> ratatui::style::Color {
    match bucket {
        AllocationBucket::AvailableCash => ACCENT_2,
        AllocationBucket::ExpenseReserve => ACCENT_3,
        AllocationBucket::TaxReserve => ACCENT_1,
        AllocationBucket::EmergencyFund => ACCENT_1,
        AllocationBucket::Savings => ACCENT_2,
        AllocationBucket::Investment => ACCENT_3,
        AllocationBucket::Other => ACCENT_1,
    }
}

fn trend_badge(trend: Option<ShortTermTrend>) -> (String, ratatui::style::Style) {
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

fn format_runway(value: Option<f64>) -> String {
    value
        .map(|months| format!("{months:.1}m"))
        .unwrap_or_else(|| "n/a".to_owned())
}

fn signed_minor(value: Option<i64>) -> String {
    match value {
        Some(amount) if amount > 0 => format!("+{}", format_minor_compact(amount)),
        Some(amount) => format_minor_compact(amount),
        None => "n/a".to_owned(),
    }
}

fn signed_value_style(app: &App, value: Option<i64>) -> ratatui::style::Style {
    match value {
        Some(amount) if amount > 0 => app.theme.body.fg(ACCENT_2),
        Some(amount) if amount < 0 => app.theme.body.fg(ACCENT_3),
        Some(_) => app.theme.body,
        None => app.theme.footer_meta,
    }
}

fn pct_label(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:+.1}%"))
        .unwrap_or_else(|| "n/a".to_owned())
}

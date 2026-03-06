use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{
        BarChart, Block, Borders, Cell, Clear, LineGauge, List, ListItem, Paragraph, Row, Table,
        Tabs, Wrap,
    },
};

use crate::{
    app::App,
    fetch::{
        CashflowDashboardPayload, CategoriesDashboardPayload, OverviewDashboardPayload,
        RoutePayload, SummaryDashboardPayload, TransactionsPayload, transaction_matches_query,
    },
    palette::PaletteRow,
    palette::{ACCENT_1, ACCENT_2, ACCENT_3},
    routes::Route,
};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let root = Block::default().style(app.theme.root);
    frame.render_widget(root, frame.area());

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_header(frame, app, sections[0]);
    render_body(frame, app, sections[1]);
    render_footer(frame, app, sections[2]);
    if app.palette.open {
        render_palette(frame, app);
    }
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24),
            Constraint::Min(10),
            Constraint::Length(36),
        ])
        .split(area);

    let brand = Paragraph::new(Line::from(Span::styled(app.header_text(), app.theme.brand)));
    frame.render_widget(brand, chunks[0]);

    let center = Paragraph::new(Line::from(Span::styled(
        app.route_context(),
        app.theme.header_meta,
    )))
    .alignment(Alignment::Center);
    frame.render_widget(center, chunks[1]);

    let right = Paragraph::new(Line::from(Span::styled(
        "cmd+p | ctrl+p command palette",
        app.theme.header_meta,
    )))
    .alignment(Alignment::Right);
    frame.render_widget(right, chunks[2]);
}

fn render_body(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    render_navigation(frame, app, sections[0]);
    render_main(frame, app, sections[1]);
}

fn render_navigation(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let border_style = if app.is_navigation_focused() {
        app.theme.brand
    } else {
        app.theme.border
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Navigation ", app.theme.header_meta))
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let titles = Route::ALL
        .iter()
        .map(|route| Line::from(Span::raw(route.label())))
        .collect::<Vec<_>>();
    let selected = Route::ALL
        .iter()
        .position(|route| *route == app.route)
        .unwrap_or(0);

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(app.theme.tabs)
        .highlight_style(app.theme.tabs_active)
        .divider(" | ");
    frame.render_widget(tabs, inner);
}

fn render_main(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let border_style = if app.is_navigation_focused() {
        app.theme.border
    } else {
        app.theme.brand
    };
    let title = if app.is_pending_refresh() {
        format!(" {} [loading] ", app.route.label())
    } else {
        format!(" {} ", app.route.label())
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, app.theme.header_meta))
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(payload) = app.route_payload() else {
        let placeholder = if app.is_pending_refresh() {
            "Loading route data..."
        } else {
            "No data loaded for this route."
        };
        frame.render_widget(
            Paragraph::new(placeholder).style(app.theme.footer_meta),
            inner,
        );
        return;
    };

    match payload {
        RoutePayload::Text(text) => {
            frame.render_widget(
                Paragraph::new(text.clone())
                    .style(app.theme.body)
                    .wrap(Wrap { trim: false }),
                inner,
            );
        }
        RoutePayload::Transactions(payload) => {
            render_transactions_table(frame, inner, app.selected_row(), payload, app);
        }
        RoutePayload::SummaryDashboard(payload) => {
            render_summary_dashboard(frame, inner, payload, app)
        }
        RoutePayload::CashflowDashboard(payload) => {
            render_cashflow_dashboard(frame, inner, payload, app)
        }
        RoutePayload::OverviewDashboard(payload) => {
            render_overview_dashboard(frame, inner, payload, app)
        }
        RoutePayload::CategoriesDashboard(payload) => {
            render_categories_dashboard(frame, inner, payload, app)
        }
    }
}

fn render_transactions_table(
    frame: &mut Frame<'_>,
    area: Rect,
    selected_row: usize,
    payload: &TransactionsPayload,
    app: &App,
) {
    if payload.rows.is_empty() {
        frame.render_widget(
            Paragraph::new("No rows.").style(app.theme.footer_meta),
            area,
        );
        return;
    }

    let search_query = app.transactions_search_query();
    let show_search = app.transactions_search_visible();
    let filtered = payload
        .rows
        .iter()
        .filter(|row| transaction_matches_query(row, search_query))
        .collect::<Vec<_>>();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if show_search {
            vec![
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ]
        } else {
            vec![Constraint::Length(1), Constraint::Min(1)]
        })
        .split(area);

    let table_area = if show_search {
        sections[2]
    } else {
        sections[1]
    };
    let visible_rows = table_area.height.saturating_sub(2) as usize;
    let selected = selected_row.min(filtered.len().saturating_sub(1));
    let offset = if visible_rows == 0 {
        0
    } else if selected >= visible_rows {
        selected + 1 - visible_rows
    } else {
        0
    };
    let end = if visible_rows == 0 {
        filtered.len()
    } else {
        (offset + visible_rows).min(filtered.len())
    };

    let range_start = if filtered.is_empty() { 0 } else { offset + 1 };
    let summary = format!(
        "rows={} total={} has_more={} preview_limit={} range {}-{}",
        filtered.len(),
        payload.rows.len(),
        payload.has_more,
        payload.limit,
        range_start,
        end
    );
    frame.render_widget(
        Paragraph::new(Span::styled(summary, app.theme.footer_meta)),
        sections[0],
    );

    if show_search {
        let search_line = if search_query.is_empty() {
            Line::from(vec![
                Span::styled("find: ", app.theme.section_heading),
                Span::styled(
                    "type to filter, enter close, esc clear",
                    app.theme.footer_meta,
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("find: ", app.theme.section_heading),
                Span::styled(search_query.to_owned(), app.theme.body),
            ])
        };
        frame.render_widget(Paragraph::new(search_line), sections[1]);
    }

    if filtered.is_empty() {
        frame.render_widget(
            Paragraph::new("No matching rows for current filter.").style(app.theme.footer_meta),
            table_area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from("date").style(app.theme.section_heading),
        Cell::from("from").style(app.theme.section_heading),
        Cell::from("to").style(app.theme.section_heading),
        Cell::from("amount").style(app.theme.section_heading),
        Cell::from("description").style(app.theme.section_heading),
        Cell::from("counterparty").style(app.theme.section_heading),
    ]);

    let widths = [
        Constraint::Length(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Length(12),
        Constraint::Percentage(24),
        Constraint::Percentage(16),
    ];

    let rows = filtered[offset..end]
        .iter()
        .enumerate()
        .map(|(local, row)| {
            let row_index = offset + local;
            let mut rendered = Row::new(vec![
                Cell::from(truncate_text(&row.posted_at, 19)),
                Cell::from(truncate_text(&row.from_account, 30)),
                Cell::from(truncate_text(&row.to_account, 30)),
                Cell::from(row.amount_minor.to_string()),
                Cell::from(truncate_text(&row.description, 38)),
                Cell::from(truncate_text(&row.counterparty, 24)),
            ]);
            if row_index == selected {
                rendered = rendered.style(app.theme.selected);
            }
            rendered
        });

    let table = Table::new(rows, widths).header(header).column_spacing(1);
    frame.render_widget(table, table_area);
}

fn render_summary_dashboard(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &SummaryDashboardPayload,
    app: &App,
) {
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
        frame.render_widget(
            Paragraph::new("No reserve rows.")
                .style(app.theme.footer_meta)
                .block(Block::default().borders(Borders::ALL).title(Span::styled(
                    " Runway + Reserves ",
                    app.theme.section_heading,
                ))),
            sections[1],
        );
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

fn render_cashflow_dashboard(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &CashflowDashboardPayload,
    app: &App,
) {
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

fn render_overview_dashboard(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &OverviewDashboardPayload,
    app: &App,
) {
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
        .value_style(app.theme.body.fg(ACCENT_1))
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
            Line::from(vec![
                Span::styled("scope ", app.theme.section_heading),
                Span::styled(payload.scope_label.clone(), app.theme.body),
            ]),
            Line::from(vec![
                Span::styled("accounts ", app.theme.section_heading),
                Span::styled(payload.accounts.len().to_string(), app.theme.body),
            ]),
            Line::from(vec![
                Span::styled("total ", app.theme.section_heading),
                Span::styled(
                    format_minor_compact(payload.total_balance_minor),
                    app.theme.body,
                ),
            ]),
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

fn render_categories_dashboard(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &CategoriesDashboardPayload,
    app: &App,
) {
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

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(88)])
        .split(area);

    let hints = Paragraph::new(Line::from(vec![
        Span::styled("tab ", app.theme.section_heading),
        Span::styled("focus ", app.theme.footer_meta),
        Span::styled("left/right ", app.theme.section_heading),
        Span::styled("tabs ", app.theme.footer_meta),
        Span::styled("1-6 ", app.theme.section_heading),
        Span::styled("routes ", app.theme.footer_meta),
        Span::styled("up/down ", app.theme.section_heading),
        Span::styled("rows ", app.theme.footer_meta),
        Span::styled("cmd/ctrl+f ", app.theme.section_heading),
        Span::styled("find ", app.theme.footer_meta),
        Span::styled("cmd/ctrl+p ", app.theme.section_heading),
        Span::styled("palette ", app.theme.footer_meta),
        Span::styled("r ", app.theme.section_heading),
        Span::styled("refresh ", app.theme.footer_meta),
        Span::styled("q ", app.theme.section_heading),
        Span::styled("quit", app.theme.footer_meta),
    ]));
    frame.render_widget(hints, chunks[0]);

    let (route_index, route_total) = app.route_position();
    let focus = if app.is_navigation_focused() {
        "nav"
    } else {
        "main"
    };
    let fetch_state = if app.is_pending_refresh() {
        "busy"
    } else {
        "idle"
    };
    let status = format!(
        "focus:{focus} | fetch:{fetch_state} | route:{route_index}/{route_total} | {}",
        app.status
    );
    let right = Paragraph::new(Line::from(Span::styled(status, app.theme.footer_status)))
        .alignment(Alignment::Right);
    frame.render_widget(right, chunks[1]);
}

fn render_palette(frame: &mut Frame<'_>, app: &App) {
    let area = centered_rect_with_min(50, 50, 56, 18, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.brand)
        .title(Span::styled(" Command Palette ", app.theme.section_heading));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let input = Paragraph::new(Line::from(vec![
        Span::styled("> ", app.theme.section_heading),
        Span::styled(app.palette.query.clone(), app.theme.body),
    ]));
    frame.render_widget(input, sections[0]);

    let selected_source = app.palette_filtered.get(app.palette.selected).copied();
    let rows = app.palette_rows();
    let selected_visual_index = selected_source.and_then(|selected| {
        rows.iter().position(|row| match row {
            PaletteRow::Action(index) => *index == selected,
            PaletteRow::Section(_) | PaletteRow::Separator => false,
        })
    });

    let visible_rows = sections[1].height.max(1) as usize;
    let offset = if let Some(selected_index) = selected_visual_index {
        if selected_index >= visible_rows {
            selected_index + 1 - visible_rows
        } else {
            0
        }
    } else {
        0
    };
    let end = (offset + visible_rows).min(rows.len());
    let visible_rows = &rows[offset..end];

    let mut items = Vec::with_capacity(visible_rows.len());
    for row in visible_rows {
        match row {
            PaletteRow::Section(section) => {
                let line = Line::from(Span::styled(
                    format!("{}:", section.label()),
                    app.theme.section_heading,
                ));
                items.push(ListItem::new(line));
            }
            PaletteRow::Separator => {
                let divider = "─".repeat(sections[1].width.saturating_sub(1) as usize);
                let line = Line::from(Span::styled(divider, app.theme.footer_meta));
                items.push(ListItem::new(line));
            }
            PaletteRow::Action(index) => {
                let Some(action) = app.palette_actions.get(*index) else {
                    continue;
                };

                let style = if Some(*index) == selected_source {
                    app.theme.selected
                } else {
                    app.theme.body
                };
                let context_style = if Some(*index) == selected_source {
                    app.theme.selected
                } else {
                    app.theme.footer_meta
                };

                let line = if action.context.is_empty() {
                    Line::from(Span::styled(action.title.clone(), style))
                } else {
                    palette_two_column_line(
                        &action.title,
                        &action.context,
                        style,
                        context_style,
                        sections[1].width.saturating_sub(1) as usize,
                    )
                };
                items.push(ListItem::new(line));
            }
        }
    }

    frame.render_widget(List::new(items), sections[1]);
}

fn centered_rect_with_min(
    percent_x: u16,
    percent_y: u16,
    min_width: u16,
    min_height: u16,
    area: Rect,
) -> Rect {
    let desired_width = ((area.width as u32 * percent_x as u32) / 100) as u16;
    let desired_height = ((area.height as u32 * percent_y as u32) / 100) as u16;

    let width = desired_width.max(min_width.min(area.width)).min(area.width);
    let height = desired_height
        .max(min_height.min(area.height))
        .min(area.height);

    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;

    Rect {
        x,
        y,
        width,
        height,
    }
}

fn palette_two_column_line(
    left: &str,
    right: &str,
    left_style: ratatui::style::Style,
    right_style: ratatui::style::Style,
    width: usize,
) -> Line<'static> {
    if width < 8 {
        return Line::from(Span::styled(truncate_text(left, width), left_style));
    }

    let right_width = right.chars().count().min(width / 3).max(8).min(width);
    let left_width = width.saturating_sub(right_width + 1);
    let left_text = truncate_text(left, left_width);
    let right_text = truncate_text(right, right_width);
    let padding = width
        .saturating_sub(left_text.chars().count() + right_text.chars().count())
        .max(1);

    Line::from(vec![
        Span::styled(left_text, left_style),
        Span::styled(" ".repeat(padding), left_style),
        Span::styled(right_text, right_style),
    ])
}

fn scaled_bar(value: i64, max: i64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if max <= 0 || value <= 0 {
        return " ".repeat(width);
    }
    let filled = ((value as f64 / max as f64) * width as f64)
        .round()
        .clamp(1.0, width as f64) as usize;
    let mut output = String::new();
    output.push_str(&"█".repeat(filled));
    output.push_str(&" ".repeat(width.saturating_sub(filled)));
    output
}

fn sparkline_text(values: &[i64]) -> String {
    if values.is_empty() {
        return "-".to_owned();
    }
    const LEVELS: &[char; 8] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let min = values.iter().copied().min().unwrap_or(0);
    let max = values.iter().copied().max().unwrap_or(0);
    if min == max {
        return "▅".repeat(values.len());
    }
    values
        .iter()
        .map(|value| {
            let ratio = (*value - min) as f64 / (max - min) as f64;
            let index = (ratio * (LEVELS.len() - 1) as f64).round() as usize;
            LEVELS[index.min(LEVELS.len() - 1)]
        })
        .collect::<String>()
}

fn format_minor_compact(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let abs = value.abs() as f64 / 100.0;
    if abs >= 1_000_000.0 {
        format!("{sign}{:.1}m", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{sign}{:.1}k", abs / 1_000.0)
    } else {
        format!("{sign}{abs:.0}")
    }
}

fn truncate_text(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_owned();
    }
    if max_len <= 3 {
        return value.chars().take(max_len).collect();
    }
    let mut output = value.chars().take(max_len - 3).collect::<String>();
    output.push_str("...");
    output
}

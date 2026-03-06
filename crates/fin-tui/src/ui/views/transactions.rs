use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::App,
    fetch::{
        TransactionDetailPanel, TransactionTableRow, TransactionsPayload, transaction_matches_query,
    },
    ui::widgets::{format_minor_compact, render_empty_state, truncate_text},
};

const WIDE_BREAKPOINT: u16 = 132;
const DETAIL_HEIGHT: u16 = 12;

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    selected_row: usize,
    payload: &TransactionsPayload,
    app: &App,
) {
    if payload.rows.is_empty() {
        render_empty_state(frame, area, "No transactions.", app.theme);
        return;
    }

    let search_query = app.transactions_search_query();
    let filtered = payload
        .rows
        .iter()
        .filter(|row| transaction_matches_query(row, search_query))
        .collect::<Vec<_>>();
    let selected_index = selected_row.min(filtered.len().saturating_sub(1));
    let selected = filtered.get(selected_index).copied();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(8),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(transaction_summary_line(payload, filtered.len())),
        sections[0],
    );
    frame.render_widget(
        Paragraph::new(search_line(
            search_query,
            filtered.len(),
            payload.rows.len(),
            app.transactions_search_visible(),
            app,
        )),
        sections[1],
    );

    let content = if sections[2].width >= WIDE_BREAKPOINT {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(sections[2])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(DETAIL_HEIGHT)])
            .split(sections[2])
    };

    render_transactions_table(frame, content[0], selected_index, &filtered, payload, app);
    render_transaction_detail(frame, content[1], selected, payload, app);
}

fn transaction_summary_line(payload: &TransactionsPayload, filtered_count: usize) -> Line<'static> {
    let range_start = if payload.rows.is_empty() {
        0
    } else {
        payload.page_index * payload.limit + 1
    };
    let range_end = if payload.rows.is_empty() {
        0
    } else {
        payload.page_index * payload.limit + payload.rows.len()
    };
    let group = payload.group_id.as_deref().unwrap_or("all");
    let page = payload.page_index + 1;
    let total_pages = if payload.total_count == 0 {
        1
    } else {
        payload.total_count.div_ceil(payload.limit.max(1))
    };
    Line::from(vec![Span::raw(format!(
        "group {group} | sort {} {} | page {page}/{total_pages} | rows {range_start}-{range_end} of {} | visible {filtered_count}/{} | more {}",
        transaction_sort_label(payload.sort_field),
        sort_direction_label(payload.sort_direction),
        payload.total_count,
        payload.rows.len(),
        yes_no(payload.has_more),
    ))])
}

fn search_line(
    search_query: &str,
    filtered_count: usize,
    page_count: usize,
    search_visible: bool,
    app: &App,
) -> Line<'static> {
    if search_query.is_empty() {
        return Line::from(vec![
            Span::styled("find ", app.theme.section_heading),
            Span::styled(
                if search_visible {
                    "type to filter page rows | enter close | esc clear | pgup/pgdn change page"
                } else {
                    "type to filter page rows | ctrl/cmd+f focus | pgup/pgdn change page | home/end jump"
                },
                app.theme.footer_meta,
            ),
        ]);
    }

    Line::from(vec![
        Span::styled("find ", app.theme.section_heading),
        Span::styled(search_query.to_owned(), app.theme.body),
        Span::styled(" | ", app.theme.section_heading),
        Span::styled(
            format!("matched {filtered_count} of {page_count} rows on this page"),
            app.theme.footer_meta,
        ),
    ])
}

fn render_transactions_table(
    frame: &mut Frame<'_>,
    area: Rect,
    selected_row: usize,
    filtered: &[&TransactionTableRow],
    payload: &TransactionsPayload,
    app: &App,
) {
    if filtered.is_empty() {
        render_empty_state(
            frame,
            area,
            "No matching transactions on this page.",
            app.theme,
        );
        return;
    }

    let visible_rows = area.height.saturating_sub(3) as usize;
    let offset = if visible_rows == 0 {
        0
    } else if selected_row >= visible_rows {
        selected_row + 1 - visible_rows
    } else {
        0
    };
    let end = if visible_rows == 0 {
        filtered.len()
    } else {
        (offset + visible_rows).min(filtered.len())
    };

    let header = Row::new(vec![
        Cell::from(Span::styled("date", app.theme.section_heading)),
        Cell::from(Span::styled("from", app.theme.section_heading)),
        Cell::from(Span::styled("to", app.theme.section_heading)),
        Cell::from(Span::styled("amount", app.theme.section_heading)),
        Cell::from(Span::styled("detail", app.theme.section_heading)),
    ]);
    let rows = filtered[offset..end]
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let visual_index = offset + index;
            let style = if visual_index == selected_row {
                app.theme.selected
            } else {
                app.theme.body
            };
            let detail = if row.counterparty.is_empty() {
                truncate_text(&row.description, 28)
            } else {
                truncate_text(&format!("{} | {}", row.description, row.counterparty), 28)
            };
            Row::new(vec![
                Cell::from(truncate_text(&row.posted_at, 10)),
                Cell::from(truncate_text(&row.from_account, 16)),
                Cell::from(truncate_text(&row.to_account, 16)),
                Cell::from(signed_minor(row.amount_minor)),
                Cell::from(detail),
            ])
            .style(style)
        });

    let title = format!(
        " Transactions {} / {} ",
        filtered.len().min(payload.rows.len()),
        payload.rows.len(),
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(17),
            Constraint::Length(17),
            Constraint::Length(12),
            Constraint::Min(24),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(title, app.theme.section_heading)),
    );
    frame.render_widget(table, area);
}

fn render_transaction_detail(
    frame: &mut Frame<'_>,
    area: Rect,
    selected: Option<&TransactionTableRow>,
    payload: &TransactionsPayload,
    app: &App,
) {
    let Some(selected) = selected else {
        render_empty_state(frame, area, "No transaction selected.", app.theme);
        return;
    };
    let detail = payload.detail_by_posting_id.get(&selected.posting_id);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("posted ", app.theme.section_heading),
            Span::styled(selected.posted_at.clone(), app.theme.body),
        ]),
        Line::from(vec![
            Span::styled("amount ", app.theme.section_heading),
            Span::styled(
                signed_minor(selected.amount_minor),
                signed_minor_style(selected.amount_minor, app),
            ),
            Span::styled(" | counterparty ", app.theme.section_heading),
            Span::styled(
                fallback_text(selected.counterparty.as_str(), "n/a"),
                app.theme.body,
            ),
        ]),
        Line::from(vec![
            Span::styled("flow ", app.theme.section_heading),
            Span::styled(
                format!("{} -> {}", selected.from_account, selected.to_account),
                app.theme.body,
            ),
        ]),
        Line::from(vec![
            Span::styled("description ", app.theme.section_heading),
            Span::styled(selected.description.clone(), app.theme.body),
        ]),
    ];

    if let Some(detail) = detail {
        lines.extend(detail_lines(detail, app));
    } else {
        lines.push(Line::from(vec![
            Span::styled("detail ", app.theme.section_heading),
            Span::styled("not loaded", app.theme.footer_meta),
        ]));
        if !selected.pair_accounts.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("pairs ", app.theme.section_heading),
                Span::styled(selected.pair_accounts.join(", "), app.theme.footer_meta),
            ]));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(" Selected Detail ", app.theme.section_heading)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn detail_lines(detail: &TransactionDetailPanel, app: &App) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("detail ts ", app.theme.section_heading),
            Span::styled(detail.posted_at.clone(), app.theme.footer_meta),
        ]),
        Line::from(vec![
            Span::styled("date ", app.theme.section_heading),
            Span::styled(detail.posted_date.clone(), app.theme.body),
            Span::styled(" | currency ", app.theme.section_heading),
            Span::styled(detail.currency.clone(), app.theme.body),
            Span::styled(" | detail amt ", app.theme.section_heading),
            Span::styled(
                signed_minor(detail.amount_minor),
                signed_minor_style(detail.amount_minor, app),
            ),
        ]),
        Line::from(vec![
            Span::styled("transfer ", app.theme.section_heading),
            Span::styled(yes_no(detail.is_transfer), app.theme.body),
        ]),
        Line::from(vec![
            Span::styled("posting ", app.theme.section_heading),
            Span::styled(detail.posting_id.clone(), app.theme.footer_meta),
        ]),
        Line::from(vec![
            Span::styled("journal ", app.theme.section_heading),
            Span::styled(detail.journal_entry_id.clone(), app.theme.footer_meta),
        ]),
        Line::from(vec![
            Span::styled("source ", app.theme.section_heading),
            Span::styled(
                detail.source_file.as_deref().unwrap_or("n/a").to_owned(),
                app.theme.footer_meta,
            ),
        ]),
    ];

    if let Some(raw_description) = &detail.raw_description
        && raw_description != &detail.description
    {
        lines.push(Line::from(vec![
            Span::styled("raw ", app.theme.section_heading),
            Span::styled(raw_description.clone(), app.theme.footer_meta),
        ]));
    }
    if let Some(clean_description) = &detail.clean_description
        && clean_description != &detail.description
    {
        lines.push(Line::from(vec![
            Span::styled("clean ", app.theme.section_heading),
            Span::styled(clean_description.clone(), app.theme.footer_meta),
        ]));
    }
    if let Some(counterparty) = &detail.counterparty
        && !counterparty.trim().is_empty()
    {
        lines.push(Line::from(vec![
            Span::styled("detail cp ", app.theme.section_heading),
            Span::styled(counterparty.clone(), app.theme.footer_meta),
        ]));
    }

    if detail.pair_postings.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("pairs ", app.theme.section_heading),
            Span::styled("none", app.theme.footer_meta),
        ]));
    } else {
        for (index, posting) in detail.pair_postings.iter().enumerate() {
            let prefix = if index == 0 { "pairs " } else { "      " };
            let memo = posting
                .memo
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!(" | memo {value}"))
                .unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled(prefix, app.theme.section_heading),
                Span::styled(
                    format!(
                        "{} {}{}",
                        posting.account_id,
                        signed_minor(posting.amount_minor),
                        memo
                    ),
                    app.theme.footer_meta,
                ),
            ]));
        }
    }

    lines
}

fn transaction_sort_label(sort_field: fin_sdk::TransactionSortField) -> &'static str {
    match sort_field {
        fin_sdk::TransactionSortField::PostedAt => "posted",
        fin_sdk::TransactionSortField::AmountMinor => "amount",
        fin_sdk::TransactionSortField::Description => "description",
        fin_sdk::TransactionSortField::Counterparty => "counterparty",
        fin_sdk::TransactionSortField::AccountId => "account",
    }
}

fn sort_direction_label(direction: fin_sdk::SortDirection) -> &'static str {
    match direction {
        fin_sdk::SortDirection::Asc => "asc",
        fin_sdk::SortDirection::Desc => "desc",
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn signed_minor(value: i64) -> String {
    if value > 0 {
        return format!("+{}", format_minor_compact(value));
    }
    format_minor_compact(value)
}

fn signed_minor_style(value: i64, app: &App) -> ratatui::style::Style {
    if value >= 0 {
        app.theme.section_heading
    } else {
        app.theme.body
    }
}

fn fallback_text(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_owned()
    } else {
        value.to_owned()
    }
}

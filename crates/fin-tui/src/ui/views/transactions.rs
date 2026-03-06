use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};

use crate::{
    app::App,
    fetch::{TransactionsPayload, transaction_matches_query},
    ui::widgets::{render_empty_state, truncate_text},
};

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    selected_row: usize,
    payload: &TransactionsPayload,
    app: &App,
) {
    if payload.rows.is_empty() {
        render_empty_state(frame, area, "No rows.", app.theme);
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
        render_empty_state(frame, table_area, "No matching transactions.", app.theme);
        return;
    }

    let header = Row::new(vec![
        Cell::from(Span::styled("date", app.theme.section_heading)),
        Cell::from(Span::styled("from", app.theme.section_heading)),
        Cell::from(Span::styled("to", app.theme.section_heading)),
        Cell::from(Span::styled("amount", app.theme.section_heading)),
        Cell::from(Span::styled("description", app.theme.section_heading)),
    ]);

    let rows = filtered[offset..end]
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let visual_index = offset + index;
            let style = if visual_index == selected {
                app.theme.selected
            } else {
                app.theme.body
            };
            Row::new(vec![
                Cell::from(truncate_text(&row.posted_at, 10)),
                Cell::from(truncate_text(&row.from_account, 16)),
                Cell::from(truncate_text(&row.to_account, 16)),
                Cell::from(row.amount_minor.to_string()),
                Cell::from(truncate_text(&row.description, 24)),
            ])
            .style(style)
        });

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(17),
            Constraint::Length(17),
            Constraint::Length(12),
            Constraint::Min(18),
        ],
    )
    .header(header)
    .column_spacing(1);
    frame.render_widget(table, table_area);
}

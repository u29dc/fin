use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Tabs, Wrap},
};

use crate::{
    app::App,
    fetch::{RoutePayload, TransactionsPayload, transaction_matches_query},
    palette::PaletteRow,
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

    let summary = format!(
        "rows={} total={} has_more={} preview_limit={} range {}-{}",
        filtered.len(),
        payload.rows.len(),
        payload.has_more,
        payload.limit,
        offset + 1,
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

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(76)])
        .split(area);

    let hints = Paragraph::new(Line::from(vec![
        Span::styled("tab ", app.theme.section_heading),
        Span::styled("focus ", app.theme.footer_meta),
        Span::styled("left/right ", app.theme.section_heading),
        Span::styled("tabs ", app.theme.footer_meta),
        Span::styled("up/down ", app.theme.section_heading),
        Span::styled("rows ", app.theme.footer_meta),
        Span::styled("cmd/ctrl+f ", app.theme.section_heading),
        Span::styled("find ", app.theme.footer_meta),
        Span::styled("pgup/pgdn ", app.theme.section_heading),
        Span::styled("jump ", app.theme.footer_meta),
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
    let area = centered_rect_with_min(50, 50, 56, 16, frame.area());
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

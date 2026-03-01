use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        Axis, BarChart, Block, Borders, Cell, Chart, Clear, Dataset, GraphType, List, ListItem,
        Paragraph, Row, Table, Tabs, Wrap,
    },
};

use crate::{
    app::App,
    fetch::{
        CategoryBarsPayload, ChartTone, LineChartPayload, RoutePayload, TransactionsPayload,
        transaction_matches_query,
    },
    palette::PaletteRow,
    palette::{ACCENT_1, ACCENT_2, ACCENT_3, ACCENT_4},
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
        RoutePayload::LineChart(payload) => render_line_chart(frame, inner, payload, app),
        RoutePayload::CategoryBars(payload) => render_category_bars(frame, inner, payload, app),
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

fn render_line_chart(frame: &mut Frame<'_>, area: Rect, payload: &LineChartPayload, app: &App) {
    if payload.series.is_empty() {
        frame.render_widget(
            Paragraph::new("No chart series available.").style(app.theme.footer_meta),
            area,
        );
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{} ", payload.title), app.theme.section_heading),
            Span::styled(payload.subtitle.clone(), app.theme.footer_meta),
        ])),
        sections[0],
    );

    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    let mut x_max = 0_f64;
    for series in &payload.series {
        if let Some(last) = series.points.last() {
            x_max = x_max.max(last.0);
        }
        for (_, value) in &series.points {
            y_min = y_min.min(*value);
            y_max = y_max.max(*value);
        }
    }
    if !y_min.is_finite() || !y_max.is_finite() {
        frame.render_widget(
            Paragraph::new("Chart values are unavailable.").style(app.theme.footer_meta),
            sections[1],
        );
        return;
    }
    if (y_max - y_min).abs() < f64::EPSILON {
        y_min -= 1.0;
        y_max += 1.0;
    } else {
        let padding = (y_max - y_min) * 0.12;
        y_min -= padding;
        y_max += padding;
    }
    let mid_y = (y_min + y_max) / 2.0;

    let datasets = payload
        .series
        .iter()
        .map(|series| {
            Dataset::default()
                .name(series.label.clone())
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(app.theme.body.fg(chart_tone_color(series.tone)))
                .data(&series.points)
        })
        .collect::<Vec<_>>();

    let x_labels = axis_labels(&payload.x_labels);
    let y_labels = vec![
        Span::raw(format_major_axis(y_min)),
        Span::raw(format_major_axis(mid_y)),
        Span::raw(format_major_axis(y_max)),
    ];

    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .style(app.theme.footer_meta)
                .bounds([0.0, x_max.max(1.0)])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .style(app.theme.footer_meta)
                .bounds([y_min, y_max])
                .labels(y_labels),
        );

    frame.render_widget(chart, sections[1]);

    let legend = payload
        .series
        .iter()
        .map(|series| {
            let latest = series.points.last().map(|(_, y)| *y).unwrap_or_default();
            Span::styled(
                format!("{} {}", series.label, format_major_axis(latest)),
                app.theme.body.fg(chart_tone_color(series.tone)),
            )
        })
        .collect::<Vec<_>>();

    let mut legend_spans = Vec::new();
    for (index, span) in legend.into_iter().enumerate() {
        legend_spans.push(span);
        if index + 1 < payload.series.len() {
            legend_spans.push(Span::styled("  |  ", app.theme.footer_meta));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(legend_spans)), sections[2]);
}

fn render_category_bars(
    frame: &mut Frame<'_>,
    area: Rect,
    payload: &CategoryBarsPayload,
    app: &App,
) {
    if payload.points.is_empty() {
        frame.render_widget(
            Paragraph::new("No category bars to render.").style(app.theme.footer_meta),
            area,
        );
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{} ", payload.title), app.theme.section_heading),
            Span::styled(payload.subtitle.clone(), app.theme.footer_meta),
        ])),
        sections[0],
    );

    let bars = payload
        .points
        .iter()
        .map(|point| (truncate_text(&point.label, 18), point.total_major))
        .collect::<Vec<_>>();
    let bar_refs = bars
        .iter()
        .map(|(label, value)| (label.as_str(), *value))
        .collect::<Vec<_>>();

    let bar_chart = BarChart::default()
        .data(bar_refs.as_slice())
        .bar_width(8)
        .bar_gap(1)
        .bar_style(app.theme.body.fg(ACCENT_2))
        .value_style(app.theme.body.fg(ACCENT_1))
        .label_style(app.theme.footer_meta)
        .max(
            payload
                .points
                .iter()
                .map(|point| point.total_major)
                .max()
                .unwrap_or(1),
        );

    frame.render_widget(bar_chart, sections[1]);
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

fn axis_labels(labels: &[String]) -> Vec<Span<'static>> {
    if labels.is_empty() {
        return vec![Span::raw(""), Span::raw(""), Span::raw("")];
    }
    let first = labels.first().cloned().unwrap_or_default();
    let mid = labels
        .get(labels.len() / 2)
        .cloned()
        .unwrap_or_else(|| first.clone());
    let last = labels.last().cloned().unwrap_or_else(|| first.clone());
    vec![Span::raw(first), Span::raw(mid), Span::raw(last)]
}

fn chart_tone_color(tone: ChartTone) -> ratatui::style::Color {
    match tone {
        ChartTone::Accent1 => ACCENT_1,
        ChartTone::Accent2 => ACCENT_2,
        ChartTone::Accent3 => ACCENT_3,
        ChartTone::Accent4 => ACCENT_4,
    }
}

fn format_major_axis(value: f64) -> String {
    let abs = value.abs();
    if abs >= 1_000_000.0 {
        format!("{:.1}m", value / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
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

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs},
};

use crate::{app::App, palette::PaletteRow, routes::Route};

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
            Constraint::Length(28),
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
        "cmd+p | ctrl+p palette",
        app.theme.header_meta,
    )))
    .alignment(Alignment::Right);
    frame.render_widget(right, chunks[2]);
}

fn render_body(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

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
    frame.render_widget(tabs, sections[0]);

    let title = if app.is_pending_refresh() {
        format!(" {} [loading] ", app.route.label())
    } else {
        format!(" {} ", app.route.label())
    };
    let body = Paragraph::new(app.body_text()).style(app.theme.body).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(title, app.theme.header_meta))
            .border_style(app.theme.border),
    );
    frame.render_widget(body, sections[1]);
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(60)])
        .split(area);

    let hints = Paragraph::new(Line::from(vec![
        Span::styled("tab ", app.theme.footer_key),
        Span::styled("switch ", app.theme.footer_meta),
        Span::styled("1/2/3 ", app.theme.footer_key),
        Span::styled("routes ", app.theme.footer_meta),
        Span::styled("cmd/ctrl+p ", app.theme.footer_key),
        Span::styled("palette ", app.theme.footer_meta),
        Span::styled("r ", app.theme.footer_key),
        Span::styled("refresh ", app.theme.footer_meta),
        Span::styled("q ", app.theme.footer_key),
        Span::styled("quit", app.theme.footer_meta),
    ]));
    frame.render_widget(hints, chunks[0]);

    let (route_index, route_total) = app.route_position();
    let fetch_state = if app.is_pending_refresh() {
        "busy"
    } else {
        "idle"
    };
    let status = format!(
        "fetch:{fetch_state} | route:{route_index}/{route_total} | {}",
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
        .title(Span::styled(" Command Palette ", app.theme.footer_key));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let input = Paragraph::new(Line::from(vec![
        Span::styled("> ", app.theme.footer_key),
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
                    app.theme.footer_key,
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
                    app.theme.tabs_active
                } else {
                    app.theme.body
                };
                let context_style = if Some(*index) == selected_source {
                    app.theme.tabs_active
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

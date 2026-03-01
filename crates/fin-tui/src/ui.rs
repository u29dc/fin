use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::{app::App, routes::Route};

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
        "tab | left/right switch",
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

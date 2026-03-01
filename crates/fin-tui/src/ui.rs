use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
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
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let header = Paragraph::new(app.header_text())
        .style(app.theme.header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border),
        );
    frame.render_widget(header, sections[0]);

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
        .divider(" | ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border),
        );
    frame.render_widget(tabs, sections[1]);

    let body = Paragraph::new(app.body_text()).style(app.theme.body).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(app.theme.border),
    );
    frame.render_widget(body, sections[2]);

    let footer = Paragraph::new(app.footer_text()).style(app.theme.footer);
    frame.render_widget(footer, sections[3]);
}

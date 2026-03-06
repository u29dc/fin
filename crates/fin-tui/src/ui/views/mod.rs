mod cashflow;
mod categories;
mod overview;
mod summary;
mod transactions;

use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Paragraph, Wrap},
};

use crate::{app::App, fetch::RoutePayload};

pub fn render_payload(frame: &mut Frame<'_>, area: Rect, app: &App, payload: &RoutePayload) {
    match payload {
        RoutePayload::Text(text) => {
            frame.render_widget(
                Paragraph::new(text.clone())
                    .style(app.theme.body)
                    .wrap(Wrap { trim: false }),
                area,
            );
        }
        RoutePayload::Transactions(payload) => {
            transactions::render(frame, area, app.selected_row(), payload, app)
        }
        RoutePayload::SummaryDashboard(payload) => summary::render(frame, area, payload, app),
        RoutePayload::CashflowDashboard(payload) => cashflow::render(frame, area, payload, app),
        RoutePayload::OverviewDashboard(payload) => overview::render(frame, area, payload, app),
        RoutePayload::CategoriesDashboard(payload) => categories::render(frame, area, payload, app),
    }
}

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    cache::RouteCache,
    fetch::FetchClient,
    routes::Route,
    theme::{HEADER_CONTRACT, HeaderContract, Theme},
};

#[derive(Debug)]
pub struct App {
    pub route: Route,
    pub should_quit: bool,
    pub theme: Theme,
    pub header: HeaderContract,
    fetch_client: FetchClient,
    cache: RouteCache,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            route: Route::Overview,
            should_quit: false,
            theme: Theme::default(),
            header: HEADER_CONTRACT,
            fetch_client: FetchClient::new(),
            cache: RouteCache::new(),
        };
        app.refresh();
        app
    }

    pub fn on_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('1') => self.set_route(Route::Overview),
            KeyCode::Char('2') => self.set_route(Route::Transactions),
            KeyCode::Char('3') => self.set_route(Route::Reports),
            KeyCode::Char('r') => self.refresh(),
            _ => {}
        }
    }

    pub fn header_text(&self) -> String {
        self.header.render()
    }

    pub fn body_text(&self) -> &str {
        self.cache
            .get(self.route)
            .unwrap_or("No data loaded for this route.")
    }

    pub fn footer_text(&self) -> &'static str {
        "q quit | 1 overview | 2 transactions | 3 reports | r refresh"
    }

    fn set_route(&mut self, route: Route) {
        if self.route != route {
            self.route = route;
            self.refresh();
        }
    }

    fn refresh(&mut self) {
        let payload = self.fetch_client.fetch_route(self.route);
        self.cache.store(self.route, payload);
    }
}

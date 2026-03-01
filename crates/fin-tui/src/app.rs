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
    pub status: String,
    pub theme: Theme,
    pub header: HeaderContract,
    pending_refresh: bool,
    fetch_client: FetchClient,
    cache: RouteCache,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            route: Route::Overview,
            should_quit: false,
            status: "Initializing".to_owned(),
            theme: Theme::default(),
            header: HEADER_CONTRACT,
            pending_refresh: false,
            fetch_client: FetchClient::new(),
            cache: RouteCache::new(),
        };
        app.request_refresh("startup");
        app
    }

    pub fn on_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('1') => self.set_route(Route::Overview),
            KeyCode::Char('2') => self.set_route(Route::Transactions),
            KeyCode::Char('3') => self.set_route(Route::Reports),
            KeyCode::Tab | KeyCode::Right => self.next_route(),
            KeyCode::BackTab | KeyCode::Left => self.prev_route(),
            KeyCode::Char('r') => self.request_refresh("manual refresh"),
            _ => {}
        }
    }

    pub fn on_tick(&mut self) {
        if !self.pending_refresh {
            return;
        }
        self.pending_refresh = false;
        self.refresh();
    }

    pub fn header_text(&self) -> String {
        self.header.render()
    }

    pub fn route_context(&self) -> String {
        format!("finance/{}", self.route.id())
    }

    pub fn route_position(&self) -> (usize, usize) {
        (self.current_route_index() + 1, Route::ALL.len())
    }

    pub fn is_pending_refresh(&self) -> bool {
        self.pending_refresh
    }

    pub fn body_text(&self) -> &str {
        self.cache
            .get(self.route)
            .or({
                if self.pending_refresh {
                    Some("Loading route data...")
                } else {
                    None
                }
            })
            .unwrap_or("No data loaded for this route.")
    }

    fn set_route(&mut self, route: Route) {
        if self.route != route {
            self.route = route;
            self.request_refresh("route changed");
        }
    }

    fn refresh(&mut self) {
        self.status = format!("Loading {}...", self.route.label().to_ascii_lowercase());
        let payload = self.fetch_client.fetch_route(self.route);
        if payload.starts_with("Route unavailable:") {
            self.status = payload.clone();
        } else {
            self.status = format!("Loaded {}", self.route.label().to_ascii_lowercase());
        }
        self.cache.store(self.route, payload);
    }

    fn request_refresh(&mut self, reason: &str) {
        self.pending_refresh = true;
        self.status = format!("{reason}: {}", self.route.label().to_ascii_lowercase());
    }

    fn next_route(&mut self) {
        let current = self.current_route_index();
        let next = (current + 1) % Route::ALL.len();
        self.set_route(Route::ALL[next]);
    }

    fn prev_route(&mut self) {
        let current = self.current_route_index();
        let previous = if current == 0 {
            Route::ALL.len() - 1
        } else {
            current - 1
        };
        self.set_route(Route::ALL[previous]);
    }

    fn current_route_index(&self) -> usize {
        Route::ALL
            .iter()
            .position(|candidate| *candidate == self.route)
            .unwrap_or(0)
    }
}

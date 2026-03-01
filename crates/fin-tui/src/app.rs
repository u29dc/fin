use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    cache::RouteCache,
    fetch::{FetchClient, RoutePayload},
    palette::{
        PaletteAction, PaletteActionKind, PaletteRow, PaletteSection, PaletteState, build_rows,
        filtered_action_indices,
    },
    routes::Route,
    theme::{HEADER_CONTRACT, HeaderContract, Theme},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Navigation,
    Main,
}

#[derive(Debug)]
pub struct App {
    pub route: Route,
    pub should_quit: bool,
    pub status: String,
    pub theme: Theme,
    pub header: HeaderContract,
    pub focus: FocusTarget,
    pub nav_cursor: usize,
    pub palette: PaletteState,
    pub palette_actions: Vec<PaletteAction>,
    pub palette_filtered: Vec<usize>,
    pending_refresh: bool,
    fetch_client: FetchClient,
    cache: RouteCache,
    selected_rows: BTreeMap<Route, usize>,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            route: Route::Overview,
            should_quit: false,
            status: "Initializing".to_owned(),
            theme: Theme::default(),
            header: HEADER_CONTRACT,
            focus: FocusTarget::Navigation,
            nav_cursor: 0,
            palette: PaletteState::default(),
            palette_actions: Vec::new(),
            palette_filtered: Vec::new(),
            pending_refresh: false,
            fetch_client: FetchClient::new(),
            cache: RouteCache::new(),
            selected_rows: BTreeMap::new(),
        };
        app.request_refresh("startup");
        app
    }

    pub fn on_key(&mut self, key_event: KeyEvent) {
        if self.handle_palette_key(key_event) {
            return;
        }
        if is_palette_trigger(key_event) {
            self.open_palette();
            return;
        }

        match key_event.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('1') => {
                self.set_route(Route::Overview);
                return;
            }
            KeyCode::Char('2') => {
                self.set_route(Route::Transactions);
                return;
            }
            KeyCode::Char('3') => {
                self.set_route(Route::Reports);
                return;
            }
            KeyCode::Left => {
                self.prev_route();
                return;
            }
            KeyCode::Right => {
                self.next_route();
                return;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    FocusTarget::Navigation => FocusTarget::Main,
                    FocusTarget::Main => FocusTarget::Navigation,
                };
                return;
            }
            KeyCode::Char('r') => {
                self.request_refresh("manual refresh");
                return;
            }
            _ => {}
        }

        match self.focus {
            FocusTarget::Navigation => self.handle_navigation_key(key_event),
            FocusTarget::Main => self.handle_main_key(key_event),
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

    pub fn palette_rows(&self) -> Vec<PaletteRow> {
        build_rows(&self.palette_actions, &self.palette_filtered)
    }

    pub fn route_payload(&self) -> Option<&RoutePayload> {
        self.cache.get(self.route)
    }

    pub fn selected_row(&self) -> usize {
        self.selected_rows.get(&self.route).copied().unwrap_or(0)
    }

    pub fn is_navigation_focused(&self) -> bool {
        self.focus == FocusTarget::Navigation
    }

    pub fn set_selected_row(&mut self, row: usize) {
        self.selected_rows.insert(self.route, row);
    }

    fn set_route(&mut self, route: Route) {
        if self.route == route {
            return;
        }
        self.route = route;
        self.nav_cursor = self.current_route_index();
        self.clamp_selected_row();

        if self.cache.get(route).is_some() {
            self.status = format!("Loaded {}", route.label().to_ascii_lowercase());
        } else {
            self.request_refresh("route changed");
        }
    }

    fn refresh(&mut self) {
        self.status = format!("Loading {}...", self.route.label().to_ascii_lowercase());
        match self.fetch_client.fetch_route(self.route) {
            Ok(payload) => {
                self.cache.store(self.route, payload);
                self.clamp_selected_row();
                self.status = format!("Loaded {}", self.route.label().to_ascii_lowercase());
            }
            Err(error) => {
                let message = format!("Route unavailable: {error}");
                self.cache
                    .store(self.route, RoutePayload::Text(message.clone()));
                self.status = message;
            }
        }
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

    fn current_row_count(&self) -> usize {
        match self.cache.get(self.route) {
            Some(RoutePayload::Transactions(payload)) => payload.rows.len(),
            Some(RoutePayload::Text(_)) | None => 0,
        }
    }

    fn clamp_selected_row(&mut self) {
        let len = self.current_row_count();
        let selected = self.selected_row();
        if len == 0 {
            self.selected_rows.insert(self.route, 0);
        } else if selected >= len {
            self.selected_rows.insert(self.route, len - 1);
        }
    }

    fn handle_navigation_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('h') => self.prev_route(),
            KeyCode::Char('l') => self.next_route(),
            KeyCode::Enter => self.focus = FocusTarget::Main,
            _ => {}
        }
    }

    fn handle_main_key(&mut self, key_event: KeyEvent) {
        let len = self.current_row_count();
        if len == 0 {
            return;
        }

        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.set_selected_row(self.selected_row().saturating_sub(1));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let next = (self.selected_row() + 1).min(len - 1);
                self.set_selected_row(next);
            }
            KeyCode::PageUp => {
                self.set_selected_row(self.selected_row().saturating_sub(10));
            }
            KeyCode::PageDown => {
                let next = (self.selected_row() + 10).min(len - 1);
                self.set_selected_row(next);
            }
            KeyCode::Home => self.set_selected_row(0),
            KeyCode::End => self.set_selected_row(len - 1),
            _ => {}
        }
    }

    fn open_palette(&mut self) {
        self.palette.open = true;
        self.palette.query.clear();
        self.palette.selected = 0;
        self.rebuild_palette_actions();
    }

    fn close_palette(&mut self) {
        self.palette.open = false;
        self.palette.query.clear();
        self.palette.selected = 0;
        self.palette_filtered.clear();
        self.palette_actions.clear();
    }

    fn rebuild_palette_actions(&mut self) {
        self.palette_actions = self.build_palette_actions();
        self.palette_filtered = filtered_action_indices(&self.palette_actions, &self.palette.query);
        self.clamp_palette_selection();
    }

    fn clamp_palette_selection(&mut self) {
        if self.palette_filtered.is_empty() {
            self.palette.selected = 0;
            return;
        }
        if self.palette.selected >= self.palette_filtered.len() {
            self.palette.selected = self.palette_filtered.len().saturating_sub(1);
        }
    }

    fn build_palette_actions(&self) -> Vec<PaletteAction> {
        let mut actions = vec![PaletteAction {
            title: "Refresh current page".to_owned(),
            context: self.route.label().to_ascii_lowercase(),
            section: PaletteSection::Context,
            kind: PaletteActionKind::Refresh,
            keywords: vec!["refresh".to_owned(), "reload".to_owned()],
        }];

        for route in Route::ALL {
            actions.push(PaletteAction {
                title: format!("Go to {}", route.label()),
                context: "finance".to_owned(),
                section: PaletteSection::Navigate,
                kind: PaletteActionKind::Navigate(route),
                keywords: vec![
                    "navigate".to_owned(),
                    route.id().to_owned(),
                    route.label().to_ascii_lowercase(),
                ],
            });
        }

        actions.push(PaletteAction {
            title: "Refresh".to_owned(),
            context: "global".to_owned(),
            section: PaletteSection::Global,
            kind: PaletteActionKind::Refresh,
            keywords: vec!["refresh".to_owned(), "reload".to_owned()],
        });
        actions.push(PaletteAction {
            title: "Quit".to_owned(),
            context: "global".to_owned(),
            section: PaletteSection::Global,
            kind: PaletteActionKind::Quit,
            keywords: vec!["quit".to_owned(), "exit".to_owned()],
        });

        actions
    }

    fn handle_palette_key(&mut self, key_event: KeyEvent) -> bool {
        if !self.palette.open {
            return false;
        }

        match key_event.code {
            KeyCode::Esc => self.close_palette(),
            KeyCode::Up => {
                if self.palette.selected > 0 {
                    self.palette.selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.palette.selected + 1 < self.palette_filtered.len() {
                    self.palette.selected += 1;
                }
            }
            KeyCode::PageUp => {
                self.palette.selected = self.palette.selected.saturating_sub(10);
            }
            KeyCode::PageDown => {
                if self.palette_filtered.is_empty() {
                    self.palette.selected = 0;
                } else {
                    let max_index = self.palette_filtered.len() - 1;
                    self.palette.selected = (self.palette.selected + 10).min(max_index);
                }
            }
            KeyCode::Home => self.palette.selected = 0,
            KeyCode::End => {
                if self.palette_filtered.is_empty() {
                    self.palette.selected = 0;
                } else {
                    self.palette.selected = self.palette_filtered.len() - 1;
                }
            }
            KeyCode::Enter => self.execute_selected_palette_action(),
            KeyCode::Backspace => {
                self.palette.query.pop();
                self.rebuild_palette_actions();
            }
            KeyCode::Char(character) => {
                if key_event.modifiers.contains(KeyModifiers::CONTROL)
                    || key_event.modifiers.contains(KeyModifiers::SUPER)
                {
                    return true;
                }
                self.palette.query.push(character);
                self.rebuild_palette_actions();
            }
            _ => {}
        }

        true
    }

    fn execute_selected_palette_action(&mut self) {
        let Some(source_index) = self.palette_filtered.get(self.palette.selected).copied() else {
            return;
        };
        let Some(action) = self.palette_actions.get(source_index).cloned() else {
            return;
        };

        match action.kind {
            PaletteActionKind::Navigate(route) => {
                self.close_palette();
                self.set_route(route);
            }
            PaletteActionKind::Refresh => {
                self.close_palette();
                self.request_refresh("palette refresh");
            }
            PaletteActionKind::Quit => self.should_quit = true,
        }
    }
}

fn is_palette_trigger(key_event: KeyEvent) -> bool {
    match key_event.code {
        KeyCode::Char(character) => {
            character.eq_ignore_ascii_case(&'p')
                && (key_event.modifiers.contains(KeyModifiers::SUPER)
                    || key_event.modifiers.contains(KeyModifiers::CONTROL))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{App, FocusTarget};
    use crate::{
        fetch::{RoutePayload, TransactionTableRow, TransactionsPayload},
        routes::Route,
    };

    fn seed_transactions(app: &mut App, count: usize) {
        let rows = (0..count)
            .map(|index| TransactionTableRow {
                posted_at: format!("2026-03-{:02}T10:00:00", (index % 28) + 1),
                from_account: "Assets:Personal:Monzo".to_owned(),
                to_account: "Expenses:Food:Groceries".to_owned(),
                amount_minor: index as i64,
                description: format!("Test {index}"),
                counterparty: "Demo".to_owned(),
            })
            .collect::<Vec<_>>();
        app.cache.store(
            Route::Transactions,
            RoutePayload::Transactions(TransactionsPayload {
                rows,
                limit: 1000,
                has_more: false,
            }),
        );
    }

    #[test]
    fn left_and_right_cycle_routes() {
        let mut app = App::new();
        assert_eq!(app.route, Route::Overview);

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.route, Route::Transactions);

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.route, Route::Reports);

        app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.route, Route::Transactions);
    }

    #[test]
    fn tab_toggles_focus() {
        let mut app = App::new();
        assert_eq!(app.focus, FocusTarget::Navigation);

        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Main);

        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Navigation);
    }

    #[test]
    fn main_focus_moves_transaction_selection() {
        let mut app = App::new();
        seed_transactions(&mut app, 5);
        app.set_route(Route::Transactions);

        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Main);
        assert_eq!(app.selected_row(), 0);

        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected_row(), 1);

        app.on_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(app.selected_row(), 4);

        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected_row(), 4);
    }

    #[test]
    fn palette_trigger_opens_and_navigates_to_filtered_route() {
        let mut app = App::new();
        assert!(!app.palette.open);

        app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert!(app.palette.open);

        for ch in "transactions".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.route, Route::Transactions);
        assert!(!app.palette.open);
    }
}

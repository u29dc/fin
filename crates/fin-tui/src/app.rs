use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    cache::{RouteCache, RouteViewKey},
    fetch::{FetchClient, FetchContext, OverviewScope, RoutePayload, transaction_matches_query},
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
    selected_rows: BTreeMap<RouteViewKey, usize>,
    available_groups: Vec<String>,
    fetch_context: FetchContext,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            route: Route::Summary,
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
            available_groups: vec![
                "business".to_owned(),
                "joint".to_owned(),
                "personal".to_owned(),
            ],
            fetch_context: FetchContext::default(),
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
        if self.handle_transactions_search_key(key_event) {
            return;
        }

        match key_event.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('1') => {
                self.set_route(Route::Summary);
                return;
            }
            KeyCode::Char('2') => {
                self.set_route(Route::Transactions);
                return;
            }
            KeyCode::Char('3') => {
                self.set_route(Route::Cashflow);
                return;
            }
            KeyCode::Char('4') => {
                self.set_route(Route::Overview);
                return;
            }
            KeyCode::Char('5') => {
                self.set_route(Route::Categories);
                return;
            }
            KeyCode::Char('6') => {
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
        self.fetch_context.route_context(self.route)
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
        self.cache.get(&self.current_cache_key())
    }

    pub fn selected_row(&self) -> usize {
        self.selected_rows
            .get(&self.current_view_key())
            .copied()
            .unwrap_or(0)
    }

    pub fn is_navigation_focused(&self) -> bool {
        self.focus == FocusTarget::Navigation
    }

    pub fn transactions_search_query(&self) -> &str {
        &self.fetch_context.transactions.search_query
    }

    pub fn transactions_search_visible(&self) -> bool {
        self.fetch_context.transactions.search_active
            || !self.fetch_context.transactions.search_query.is_empty()
    }

    pub fn set_selected_row(&mut self, row: usize) {
        let key = self.current_view_key();
        self.selected_rows.insert(key, row);
    }

    fn current_cache_key(&self) -> crate::cache::RouteCacheKey {
        self.fetch_context.route_cache_key(self.route)
    }

    fn current_view_key(&self) -> RouteViewKey {
        self.fetch_context.route_view_key(self.route)
    }

    fn set_route(&mut self, route: Route) {
        if self.route == route {
            return;
        }
        self.route = route;
        self.nav_cursor = self.current_route_index();
        self.clamp_selected_row();

        if self.cache.get(&self.current_cache_key()).is_some() {
            self.status = format!("Loaded {}", route.label().to_ascii_lowercase());
        } else {
            self.request_refresh("route changed");
        }

        if route != Route::Transactions {
            self.fetch_context.transactions.search_query.clear();
            self.fetch_context.transactions.search_active = false;
        }
    }

    fn refresh(&mut self) {
        if let Ok(groups) = self.fetch_client.available_groups()
            && !groups.is_empty()
        {
            self.available_groups = groups;
        }
        self.reconcile_fetch_context();
        let cache_key = self.current_cache_key();
        self.status = format!("Loading {}...", self.route.label().to_ascii_lowercase());
        match self
            .fetch_client
            .fetch_route(self.route, &self.fetch_context)
        {
            Ok(payload) => {
                self.cache.store(cache_key, payload);
                self.clamp_selected_row();
                self.status = format!("Loaded {}", self.route.label().to_ascii_lowercase());
            }
            Err(error) => {
                let message = format!("Route unavailable: {error}");
                self.cache
                    .store(cache_key, RoutePayload::Text(message.clone()));
                self.status = message;
            }
        }
    }

    fn reconcile_fetch_context(&mut self) {
        if self.available_groups.is_empty() {
            return;
        }
        let first_group = self.available_groups.first().cloned();
        reconcile_group_state(
            &self.available_groups,
            &mut self.fetch_context.cashflow.group_id,
            first_group.as_deref(),
        );
        reconcile_group_state(
            &self.available_groups,
            &mut self.fetch_context.categories.group_id,
            first_group.as_deref(),
        );
        reconcile_group_state(
            &self.available_groups,
            &mut self.fetch_context.reports.group_id,
            first_group.as_deref(),
        );
        if let Some(group_id) = &mut self.fetch_context.transactions.group_id
            && !self
                .available_groups
                .iter()
                .any(|candidate| candidate == group_id)
        {
            *group_id = first_group.clone().unwrap_or_default();
        }
        if self.fetch_context.transactions.group_id.as_deref() == Some("") {
            self.fetch_context.transactions.group_id = None;
        }

        if let OverviewScope::Group(group) = &self.fetch_context.overview.scope
            && !self
                .available_groups
                .iter()
                .any(|candidate| candidate == group)
        {
            self.fetch_context.overview.scope = OverviewScope::All;
        }
    }

    fn request_refresh(&mut self, reason: &str) {
        self.pending_refresh = true;
        self.status = format!("{reason}: {}", self.route.label().to_ascii_lowercase());
    }

    fn current_route_group(&self) -> Option<&str> {
        match self.route {
            Route::Cashflow => Some(self.fetch_context.cashflow.group_id.as_str()),
            Route::Categories => Some(self.fetch_context.categories.group_id.as_str()),
            Route::Reports => Some(self.fetch_context.reports.group_id.as_str()),
            Route::Transactions => self.fetch_context.transactions.group_id.as_deref(),
            Route::Summary | Route::Overview => None,
        }
    }

    fn set_current_route_group(&mut self, group: String) -> bool {
        match self.route {
            Route::Cashflow => {
                self.fetch_context.cashflow.group_id = group;
                true
            }
            Route::Categories => {
                self.fetch_context.categories.group_id = group;
                true
            }
            Route::Reports => {
                self.fetch_context.reports.group_id = group;
                true
            }
            Route::Transactions => {
                self.fetch_context.transactions.group_id = Some(group);
                true
            }
            Route::Summary | Route::Overview => false,
        }
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
        match self.route_payload() {
            Some(RoutePayload::Transactions(payload)) => payload
                .rows
                .iter()
                .filter(|row| {
                    transaction_matches_query(row, &self.fetch_context.transactions.search_query)
                })
                .count(),
            Some(RoutePayload::OverviewDashboard(payload)) => payload.accounts.len(),
            Some(RoutePayload::Text(_))
            | Some(RoutePayload::SummaryDashboard(_))
            | Some(RoutePayload::CashflowDashboard(_))
            | Some(RoutePayload::CategoriesDashboard(_))
            | None => 0,
        }
    }

    fn clamp_selected_row(&mut self) {
        let len = self.current_row_count();
        let selected = self.selected_row();
        let key = self.current_view_key();
        if len == 0 {
            self.selected_rows.insert(key, 0);
        } else if selected >= len {
            self.selected_rows.insert(key, len - 1);
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
            KeyCode::Up => {
                self.set_selected_row(self.selected_row().saturating_sub(1));
            }
            KeyCode::Down => {
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

    fn handle_transactions_search_key(&mut self, key_event: KeyEvent) -> bool {
        let is_transactions_main =
            self.route == Route::Transactions && self.focus == FocusTarget::Main;
        if !is_transactions_main {
            return false;
        }

        if matches!(key_event.code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'f'))
            && (key_event.modifiers.contains(KeyModifiers::CONTROL)
                || key_event.modifiers.contains(KeyModifiers::SUPER))
        {
            self.fetch_context.transactions.search_active = true;
            if self.fetch_context.transactions.search_query.is_empty() {
                self.status = "Search transactions".to_owned();
            } else {
                self.status = format!(
                    "Filtered transactions: {}",
                    self.fetch_context.transactions.search_query
                );
            }
            return true;
        }

        if self.fetch_context.transactions.search_active {
            match key_event.code {
                KeyCode::Esc => {
                    self.fetch_context.transactions.search_query.clear();
                    self.fetch_context.transactions.search_active = false;
                    self.clamp_selected_row();
                    self.status = "Cleared transaction search".to_owned();
                    return true;
                }
                KeyCode::Enter => {
                    self.fetch_context.transactions.search_active = false;
                    return true;
                }
                KeyCode::Backspace => {
                    self.fetch_context.transactions.search_query.pop();
                    if self.fetch_context.transactions.search_query.is_empty() {
                        self.fetch_context.transactions.search_active = false;
                        self.status = "Cleared transaction search".to_owned();
                    } else {
                        self.status = format!(
                            "Filtered transactions: {}",
                            self.fetch_context.transactions.search_query
                        );
                    }
                    self.clamp_selected_row();
                    return true;
                }
                KeyCode::Char(character)
                    if !key_event.modifiers.contains(KeyModifiers::CONTROL)
                        && !key_event.modifiers.contains(KeyModifiers::SUPER)
                        && !key_event.modifiers.contains(KeyModifiers::ALT)
                        && !character.is_control() =>
                {
                    self.fetch_context.transactions.search_query.push(character);
                    self.status = format!(
                        "Filtered transactions: {}",
                        self.fetch_context.transactions.search_query
                    );
                    self.clamp_selected_row();
                    return true;
                }
                _ => return false,
            }
        }

        match key_event.code {
            KeyCode::Backspace if !self.fetch_context.transactions.search_query.is_empty() => {
                self.fetch_context.transactions.search_query.pop();
                if self.fetch_context.transactions.search_query.is_empty() {
                    self.fetch_context.transactions.search_active = false;
                    self.status = "Cleared transaction search".to_owned();
                } else {
                    self.status = format!(
                        "Filtered transactions: {}",
                        self.fetch_context.transactions.search_query
                    );
                }
                self.clamp_selected_row();
                true
            }
            KeyCode::Char(character)
                if !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::SUPER)
                    && !key_event.modifiers.contains(KeyModifiers::ALT)
                    && !character.is_control()
                    && character != 'q' =>
            {
                self.fetch_context.transactions.search_active = true;
                self.fetch_context.transactions.search_query.push(character);
                self.status = format!(
                    "Filtered transactions: {}",
                    self.fetch_context.transactions.search_query
                );
                self.clamp_selected_row();
                true
            }
            _ => false,
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

        if let Some(selected_group) = self.current_route_group() {
            for group in &self.available_groups {
                let selected = if group == selected_group {
                    " (selected)"
                } else {
                    ""
                };
                actions.push(PaletteAction {
                    title: format!("Set group: {group}{selected}"),
                    context: self.route.label().to_ascii_lowercase(),
                    section: PaletteSection::Context,
                    kind: PaletteActionKind::SetRouteGroup(group.clone()),
                    keywords: vec![
                        "group".to_owned(),
                        self.route.id().to_owned(),
                        group.clone(),
                    ],
                });
            }
        }

        if self.route == Route::Overview {
            let all_selected = matches!(self.fetch_context.overview.scope, OverviewScope::All);
            let all_suffix = if all_selected { " (selected)" } else { "" };
            actions.push(PaletteAction {
                title: format!("Set overview scope: all{all_suffix}"),
                context: "overview".to_owned(),
                section: PaletteSection::Context,
                kind: PaletteActionKind::SetOverviewScopeAll,
                keywords: vec!["overview".to_owned(), "scope".to_owned(), "all".to_owned()],
            });

            for group in &self.available_groups {
                let selected = matches!(
                    &self.fetch_context.overview.scope,
                    OverviewScope::Group(current) if current == group
                );
                let suffix = if selected { " (selected)" } else { "" };
                actions.push(PaletteAction {
                    title: format!("Set overview scope: {group}{suffix}"),
                    context: "overview".to_owned(),
                    section: PaletteSection::Context,
                    kind: PaletteActionKind::SetOverviewScopeGroup(group.clone()),
                    keywords: vec![
                        "overview".to_owned(),
                        "scope".to_owned(),
                        "group".to_owned(),
                        group.clone(),
                    ],
                });
            }
        }

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
            PaletteActionKind::SetRouteGroup(group) => {
                self.close_palette();
                if self.set_current_route_group(group.clone()) {
                    self.request_refresh("group changed");
                } else {
                    self.status = format!("Set group to {group}");
                }
            }
            PaletteActionKind::SetOverviewScopeAll => {
                self.close_palette();
                self.fetch_context.overview.scope = OverviewScope::All;
                if self.route == Route::Overview {
                    self.request_refresh("overview scope changed");
                } else {
                    self.status = "Set overview scope to all".to_owned();
                }
            }
            PaletteActionKind::SetOverviewScopeGroup(group) => {
                self.close_palette();
                self.fetch_context.overview.scope = OverviewScope::Group(group.clone());
                if self.route == Route::Overview {
                    self.request_refresh("overview scope changed");
                } else {
                    self.status = format!("Set overview scope to {group}");
                }
            }
            PaletteActionKind::Quit => self.should_quit = true,
        }
    }
}

fn reconcile_group_state(
    available_groups: &[String],
    group_id: &mut String,
    fallback: Option<&str>,
) {
    if available_groups
        .iter()
        .any(|candidate| candidate == group_id)
    {
        return;
    }
    if let Some(fallback) = fallback {
        *group_id = fallback.to_owned();
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
        fetch::{
            AccountFreshnessRow, OverviewDashboardPayload, RoutePayload, TransactionTableRow,
            TransactionsPayload,
        },
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
            app.fetch_context.route_cache_key(Route::Transactions),
            RoutePayload::Transactions(TransactionsPayload {
                rows,
                limit: 1000,
                has_more: false,
            }),
        );
    }

    fn seed_overview(app: &mut App, count: usize) {
        let accounts = (0..count)
            .map(|index| AccountFreshnessRow {
                label: format!("Account {index}"),
                balance_minor: (index as i64 + 1) * 1_000,
                updated_at: Some("2026-03-01".to_owned()),
                stale_days: Some(index as i64),
                is_investment: index % 2 == 0,
                history: Vec::new(),
                contributions: Vec::new(),
            })
            .collect::<Vec<_>>();
        app.cache.store(
            app.fetch_context.route_cache_key(Route::Overview),
            RoutePayload::OverviewDashboard(OverviewDashboardPayload {
                scope_label: "all accounts".to_owned(),
                total_balance_minor: accounts.iter().map(|row| row.balance_minor).sum(),
                scope_history: Vec::new(),
                projection: None,
                accounts,
            }),
        );
    }

    #[test]
    fn left_and_right_cycle_routes() {
        let mut app = App::new();
        assert_eq!(app.route, Route::Summary);

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.route, Route::Transactions);

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.route, Route::Cashflow);

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
    fn main_focus_moves_overview_selection() {
        let mut app = App::new();
        seed_overview(&mut app, 4);
        app.set_route(Route::Overview);

        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Main);
        assert_eq!(app.selected_row(), 0);

        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected_row(), 1);

        app.on_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(app.selected_row(), 3);
    }

    #[test]
    fn typing_in_transactions_main_starts_search() {
        let mut app = App::new();
        seed_transactions(&mut app, 5);
        app.set_route(Route::Transactions);
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Main);

        app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

        assert_eq!(app.transactions_search_query(), "rent");
        assert!(app.transactions_search_visible());
    }

    #[test]
    fn cmd_f_and_backspace_clear_search_state() {
        let mut app = App::new();
        seed_transactions(&mut app, 5);
        app.set_route(Route::Transactions);
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Main);

        app.on_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert!(app.transactions_search_visible());
        assert_eq!(app.transactions_search_query(), "");

        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(app.transactions_search_query(), "a");
        app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(app.transactions_search_query(), "");
        assert!(!app.transactions_search_visible());
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

    #[test]
    fn palette_updates_cashflow_group_context() {
        let mut app = App::new();
        app.set_route(Route::Cashflow);
        assert_eq!(app.fetch_context.cashflow.group_id, "business");

        app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        for ch in "joint".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.fetch_context.cashflow.group_id, "joint");
    }

    #[test]
    fn selection_is_scoped_to_transaction_search_context() {
        let mut app = App::new();
        seed_transactions(&mut app, 20);
        app.set_route(Route::Transactions);
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected_row(), 2);

        app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(app.selected_row(), 0);

        app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(app.selected_row(), 2);
    }

    #[test]
    fn route_context_reflects_route_specific_state() {
        let mut app = App::new();
        app.set_route(Route::Categories);
        assert_eq!(app.route_context(), "finance/categories/business/6m");

        app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        for ch in "joint".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.route_context(), "finance/categories/joint/6m");
        assert_eq!(app.fetch_context.cashflow.group_id, "business");
    }
}

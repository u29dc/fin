use std::collections::BTreeMap;

use crate::routes::Route;

#[derive(Debug, Default)]
pub struct RouteCache {
    entries: BTreeMap<Route, String>,
}

impl RouteCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&mut self, route: Route, payload: String) {
        self.entries.insert(route, payload);
    }

    pub fn get(&self, route: Route) -> Option<&str> {
        self.entries.get(&route).map(String::as_str)
    }
}

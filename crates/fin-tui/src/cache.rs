use std::collections::BTreeMap;

use crate::fetch::RoutePayload;
use crate::routes::Route;

#[derive(Debug, Default)]
pub struct RouteCache {
    entries: BTreeMap<Route, RoutePayload>,
}

impl RouteCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&mut self, route: Route, payload: RoutePayload) {
        self.entries.insert(route, payload);
    }

    pub fn get(&self, route: Route) -> Option<&RoutePayload> {
        self.entries.get(&route)
    }
}

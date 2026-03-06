use std::collections::BTreeMap;

use crate::fetch::RoutePayload;
use crate::routes::Route;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RouteCacheKey {
    pub route: Route,
    pub context_id: String,
}

impl RouteCacheKey {
    #[must_use]
    pub fn new(route: Route, context_id: impl Into<String>) -> Self {
        Self {
            route,
            context_id: context_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RouteViewKey {
    pub route: Route,
    pub context_id: String,
}

impl RouteViewKey {
    #[must_use]
    pub fn new(route: Route, context_id: impl Into<String>) -> Self {
        Self {
            route,
            context_id: context_id.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct RouteCache {
    entries: BTreeMap<RouteCacheKey, RoutePayload>,
}

impl RouteCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&mut self, key: RouteCacheKey, payload: RoutePayload) {
        self.entries.insert(key, payload);
    }

    pub fn get(&self, key: &RouteCacheKey) -> Option<&RoutePayload> {
        self.entries.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::{RouteCache, RouteCacheKey};
    use crate::{fetch::RoutePayload, routes::Route};

    #[test]
    fn route_cache_keeps_independent_context_variants() {
        let mut cache = RouteCache::new();
        let business_key = RouteCacheKey::new(Route::Cashflow, "group=business|months=12");
        let joint_key = RouteCacheKey::new(Route::Cashflow, "group=joint|months=12");

        cache.store(
            business_key.clone(),
            RoutePayload::Text("business".to_owned()),
        );
        cache.store(joint_key.clone(), RoutePayload::Text("joint".to_owned()));

        assert!(matches!(
            cache.get(&business_key),
            Some(RoutePayload::Text(value)) if value == "business"
        ));
        assert!(matches!(
            cache.get(&joint_key),
            Some(RoutePayload::Text(value)) if value == "joint"
        ));
    }
}

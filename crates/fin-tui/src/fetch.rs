use crate::routes::Route;

#[derive(Debug, Default)]
pub struct FetchClient;

impl FetchClient {
    pub fn new() -> Self {
        Self
    }

    pub fn fetch_stub(&self, route: Route) -> String {
        format!(
            "{} data is not wired yet. This is a compile-safe TUI scaffold.",
            route.label()
        )
    }
}

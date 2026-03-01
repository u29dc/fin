#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Route {
    Overview,
    Transactions,
    Reports,
}

impl Route {
    pub const ALL: [Route; 3] = [Route::Overview, Route::Transactions, Route::Reports];

    pub const fn label(self) -> &'static str {
        match self {
            Route::Overview => "Overview",
            Route::Transactions => "Transactions",
            Route::Reports => "Reports",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Route::Overview => "overview",
            Route::Transactions => "transactions",
            Route::Reports => "reports",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Route {
    Summary,
    Transactions,
    Cashflow,
    Overview,
    Categories,
    Reports,
}

impl Route {
    pub const ALL: [Route; 6] = [
        Route::Summary,
        Route::Transactions,
        Route::Cashflow,
        Route::Overview,
        Route::Categories,
        Route::Reports,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Route::Summary => "Summary",
            Route::Transactions => "Transactions",
            Route::Cashflow => "Cashflow",
            Route::Overview => "Overview",
            Route::Categories => "Categories",
            Route::Reports => "Reports",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Route::Summary => "summary",
            Route::Transactions => "transactions",
            Route::Cashflow => "cashflow",
            Route::Overview => "overview",
            Route::Categories => "categories",
            Route::Reports => "reports",
        }
    }
}

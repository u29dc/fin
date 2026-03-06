use fin_sdk::{SortDirection, TransactionSortField};

use crate::{
    cache::{RouteCacheKey, RouteViewKey},
    routes::Route,
};

pub const TUI_TRANSACTIONS_PREVIEW_LIMIT: usize = 1000;
pub const CASHFLOW_LOOKBACK_MONTHS: usize = 120;
pub const CASHFLOW_VIEW_MONTHS: usize = 12;
pub const CATEGORY_MONTHS: usize = 6;
pub const CATEGORY_LIMIT: usize = 12;
pub const CATEGORY_STABILITY_LIMIT: usize = 6;
pub const OVERVIEW_MAX_ACCOUNTS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverviewScope {
    All,
    Group(String),
}

impl OverviewScope {
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::All => "all".to_owned(),
            Self::Group(group) => group.clone(),
        }
    }

    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::All => "all accounts".to_owned(),
            Self::Group(group) => format!("{group} accounts"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryRouteState {
    pub trailing_months: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionsRouteState {
    pub group_id: Option<String>,
    pub search_query: String,
    pub search_active: bool,
    pub sort_field: TransactionSortField,
    pub sort_direction: SortDirection,
    pub window_months: Option<usize>,
    pub page_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashflowRouteState {
    pub group_id: String,
    pub view_months: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewRouteState {
    pub scope: OverviewScope,
    pub max_accounts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoriesRouteState {
    pub group_id: String,
    pub months: usize,
    pub pareto_limit: usize,
    pub stability_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportsRouteState {
    pub group_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchContext {
    pub summary: SummaryRouteState,
    pub transactions: TransactionsRouteState,
    pub cashflow: CashflowRouteState,
    pub overview: OverviewRouteState,
    pub categories: CategoriesRouteState,
    pub reports: ReportsRouteState,
}

impl Default for FetchContext {
    fn default() -> Self {
        Self {
            summary: SummaryRouteState {
                trailing_months: CASHFLOW_VIEW_MONTHS,
            },
            transactions: TransactionsRouteState {
                group_id: None,
                search_query: String::new(),
                search_active: false,
                sort_field: TransactionSortField::PostedAt,
                sort_direction: SortDirection::Desc,
                window_months: None,
                page_limit: TUI_TRANSACTIONS_PREVIEW_LIMIT,
            },
            cashflow: CashflowRouteState {
                group_id: "business".to_owned(),
                view_months: CASHFLOW_VIEW_MONTHS,
            },
            overview: OverviewRouteState {
                scope: OverviewScope::All,
                max_accounts: OVERVIEW_MAX_ACCOUNTS,
            },
            categories: CategoriesRouteState {
                group_id: "business".to_owned(),
                months: CATEGORY_MONTHS,
                pareto_limit: CATEGORY_LIMIT,
                stability_limit: CATEGORY_STABILITY_LIMIT,
            },
            reports: ReportsRouteState {
                group_id: "business".to_owned(),
            },
        }
    }
}

impl FetchContext {
    #[must_use]
    pub fn route_cache_key(&self, route: Route) -> RouteCacheKey {
        RouteCacheKey::new(route, self.route_cache_fingerprint(route))
    }

    #[must_use]
    pub fn route_view_key(&self, route: Route) -> RouteViewKey {
        RouteViewKey::new(route, self.route_view_fingerprint(route))
    }

    #[must_use]
    pub fn route_context(&self, route: Route) -> String {
        match route {
            Route::Summary => format!("finance/summary/{}m", self.summary.trailing_months),
            Route::Transactions => {
                let group = self.transactions.group_id.as_deref().unwrap_or("all");
                let window = self
                    .transactions
                    .window_months
                    .map(|months| format!("{months}m"))
                    .unwrap_or_else(|| "all".to_owned());
                format!(
                    "finance/transactions/{group}/{}-{}/{}",
                    transaction_sort_id(self.transactions.sort_field),
                    sort_direction_id(self.transactions.sort_direction),
                    window
                )
            }
            Route::Cashflow => format!(
                "finance/cashflow/{}/{}m",
                self.cashflow.group_id, self.cashflow.view_months
            ),
            Route::Overview => format!(
                "finance/overview/{}/max-{}",
                self.overview.scope.id(),
                self.overview.max_accounts
            ),
            Route::Categories => format!(
                "finance/categories/{}/{}m",
                self.categories.group_id, self.categories.months
            ),
            Route::Reports => format!("finance/reports/{}", self.reports.group_id),
        }
    }

    fn route_cache_fingerprint(&self, route: Route) -> String {
        match route {
            Route::Summary => format!("trailing-months={}", self.summary.trailing_months),
            Route::Transactions => {
                let group = self.transactions.group_id.as_deref().unwrap_or("all");
                let window = self
                    .transactions
                    .window_months
                    .map_or_else(|| "all".to_owned(), |months| months.to_string());
                format!(
                    "group={group}|sort={}-{}|window={window}|limit={}",
                    transaction_sort_id(self.transactions.sort_field),
                    sort_direction_id(self.transactions.sort_direction),
                    self.transactions.page_limit
                )
            }
            Route::Cashflow => format!(
                "group={}|months={}",
                self.cashflow.group_id, self.cashflow.view_months
            ),
            Route::Overview => format!(
                "scope={}|max-accounts={}",
                self.overview.scope.id(),
                self.overview.max_accounts
            ),
            Route::Categories => format!(
                "group={}|months={}|pareto={}|stability={}",
                self.categories.group_id,
                self.categories.months,
                self.categories.pareto_limit,
                self.categories.stability_limit
            ),
            Route::Reports => format!("group={}", self.reports.group_id),
        }
    }

    fn route_view_fingerprint(&self, route: Route) -> String {
        match route {
            Route::Transactions => format!(
                "{}|search={}|active={}",
                self.route_cache_fingerprint(route),
                encode_context_fragment(&self.transactions.search_query),
                self.transactions.search_active
            ),
            _ => self.route_cache_fingerprint(route),
        }
    }
}

fn encode_context_fragment(value: &str) -> String {
    value.replace('|', "%7C")
}

fn transaction_sort_id(sort_field: TransactionSortField) -> &'static str {
    match sort_field {
        TransactionSortField::PostedAt => "posted_at",
        TransactionSortField::AmountMinor => "amount_minor",
        TransactionSortField::Description => "description",
        TransactionSortField::Counterparty => "counterparty",
        TransactionSortField::AccountId => "account_id",
    }
}

fn sort_direction_id(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Asc => "asc",
        SortDirection::Desc => "desc",
    }
}

#[cfg(test)]
mod tests {
    use super::FetchContext;
    use crate::routes::Route;

    #[test]
    fn fetch_context_uses_distinct_cache_and_view_keys() {
        let mut context = FetchContext::default();
        let base_cache_key = context.route_cache_key(Route::Transactions);
        let base_view_key = context.route_view_key(Route::Transactions);

        context.transactions.search_query = "rent".to_owned();
        context.transactions.search_active = true;

        assert_eq!(base_cache_key, context.route_cache_key(Route::Transactions));
        assert_ne!(base_view_key, context.route_view_key(Route::Transactions));

        context.cashflow.group_id = "joint".to_owned();
        assert_ne!(
            context.route_cache_key(Route::Cashflow),
            FetchContext::default().route_cache_key(Route::Cashflow)
        );
    }
}

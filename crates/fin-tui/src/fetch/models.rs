use fin_sdk::{AllocationBucket, ShortTermTrend};

#[derive(Debug, Clone)]
pub struct TransactionTableRow {
    pub posted_at: String,
    pub from_account: String,
    pub to_account: String,
    pub amount_minor: i64,
    pub description: String,
    pub counterparty: String,
}

pub fn transaction_matches_query(row: &TransactionTableRow, query: &str) -> bool {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }

    row.posted_at.to_ascii_lowercase().contains(&needle)
        || row.from_account.to_ascii_lowercase().contains(&needle)
        || row.to_account.to_ascii_lowercase().contains(&needle)
        || row.amount_minor.to_string().contains(&needle)
        || row.description.to_ascii_lowercase().contains(&needle)
        || row.counterparty.to_ascii_lowercase().contains(&needle)
}

#[derive(Debug, Clone)]
pub struct TransactionsPayload {
    pub rows: Vec<TransactionTableRow>,
    pub limit: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct SummaryAllocationSegment {
    pub bucket: AllocationBucket,
    pub label: String,
    pub amount_minor: i64,
    pub share_pct: f64,
}

#[derive(Debug, Clone)]
pub struct SummaryAllocation {
    pub basis_label: String,
    pub balance_basis_minor: i64,
    pub display_total_minor: i64,
    pub available_minor: i64,
    pub expense_reserve_minor: i64,
    pub expense_reserve_display_minor: i64,
    pub tax_reserve_minor: i64,
    pub emergency_fund_minor: i64,
    pub savings_minor: i64,
    pub investment_minor: i64,
    pub shortfall_minor: i64,
    pub under_reserved: bool,
    pub segments: Vec<SummaryAllocationSegment>,
}

#[derive(Debug, Clone)]
pub struct SummaryMonthSnapshot {
    pub month: String,
    pub income_minor: i64,
    pub expense_minor: i64,
    pub net_minor: i64,
    pub savings_rate_pct: Option<f64>,
    pub income_change_pct: Option<f64>,
    pub expense_change_pct: Option<f64>,
    pub net_change_pct: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct SummaryGroupPanel {
    pub group_id: String,
    pub label: String,
    pub net_worth_minor: i64,
    pub runway_months: Option<f64>,
    pub available_minor: Option<i64>,
    pub last_full_month_net_minor: Option<i64>,
    pub avg_six_month_net_minor: Option<i64>,
    pub median_spend_minor: Option<i64>,
    pub short_term_trend: Option<ShortTermTrend>,
    pub anomaly_count_last_12_months: usize,
    pub recent_anomaly_months: Vec<String>,
    pub allocation: SummaryAllocation,
    pub last_month: Option<SummaryMonthSnapshot>,
}

#[derive(Debug, Clone)]
pub struct SummaryDashboardPayload {
    pub generated_at: String,
    pub consolidated_net_worth_minor: i64,
    pub groups: Vec<SummaryGroupPanel>,
}

#[derive(Debug, Clone)]
pub struct CashflowPoint {
    pub month: String,
    pub income_minor: i64,
    pub expense_minor: i64,
    pub net_minor: i64,
    pub savings_rate_pct: Option<f64>,
    pub rolling_median_expense_minor: Option<i64>,
    pub expense_deviation_ratio: Option<f64>,
    pub is_anomaly: bool,
}

#[derive(Debug, Clone)]
pub struct CashflowDashboardPayload {
    pub group_id: String,
    pub points: Vec<CashflowPoint>,
    pub latest_full_month: Option<CashflowPoint>,
    pub avg_six_month_income_minor: Option<i64>,
    pub avg_six_month_expense_minor: Option<i64>,
    pub avg_six_month_net_minor: Option<i64>,
    pub runway_months: Option<f64>,
    pub available_minor: Option<i64>,
    pub expense_reserve_minor: Option<i64>,
    pub tax_reserve_minor: Option<i64>,
    pub median_spend_minor: Option<i64>,
    pub short_term_trend: Option<ShortTermTrend>,
    pub anomaly_count_last_12_months: usize,
    pub recent_anomaly_months: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CategoryParetoPoint {
    pub category: String,
    pub total_minor: i64,
    pub transaction_count: i64,
    pub share_pct: f64,
}

#[derive(Debug, Clone)]
pub struct CategoryStabilityRow {
    pub category: String,
    pub month_values_minor: Vec<i64>,
    pub total_minor: i64,
}

#[derive(Debug, Clone)]
pub struct UncategorizedLeakage {
    pub total_expense_minor: i64,
    pub uncategorized_minor: i64,
    pub uncategorized_count: i64,
    pub leakage_pct: f64,
}

#[derive(Debug, Clone)]
pub struct CategoriesDashboardPayload {
    pub group_id: String,
    pub pareto: Vec<CategoryParetoPoint>,
    pub months: Vec<String>,
    pub stability: Vec<CategoryStabilityRow>,
    pub leakage: UncategorizedLeakage,
}

#[derive(Debug, Clone)]
pub struct AccountFreshnessRow {
    pub label: String,
    pub balance_minor: i64,
    pub updated_at: Option<String>,
    pub stale_days: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct OverviewDashboardPayload {
    pub scope_label: String,
    pub total_balance_minor: i64,
    pub accounts: Vec<AccountFreshnessRow>,
}

#[derive(Debug, Clone)]
pub enum RoutePayload {
    Text(String),
    Transactions(TransactionsPayload),
    SummaryDashboard(SummaryDashboardPayload),
    CashflowDashboard(CashflowDashboardPayload),
    OverviewDashboard(OverviewDashboardPayload),
    CategoriesDashboard(CategoriesDashboardPayload),
}

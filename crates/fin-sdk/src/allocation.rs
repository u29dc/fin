use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::{AccountConfig, FinConfig};
use crate::dashboard::{current_reporting_month, summarize_cashflow_kpis};
use crate::error::Result;
use crate::queries::{AccountBalanceRow, group_monthly_cashflow, view_accounts};
use crate::reports::report_reserves;
use crate::stats::round_ratio;

/// Bucket used to classify balances for account allocation and dashboard composition.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AllocationBucket {
    AvailableCash,
    ExpenseReserve,
    TaxReserve,
    EmergencyFund,
    Savings,
    Investment,
    Other,
}

impl AllocationBucket {
    fn label(self) -> &'static str {
        match self {
            Self::AvailableCash => "Available Cash",
            Self::ExpenseReserve => "Expense Reserve",
            Self::TaxReserve => "Tax Reserve",
            Self::EmergencyFund => "Emergency Fund",
            Self::Savings => "Savings",
            Self::Investment => "Investments",
            Self::Other => "Other",
        }
    }
}

/// Segment that can be rendered directly as a table row or segmented bar section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AllocationSegment {
    pub bucket: AllocationBucket,
    pub label: String,
    pub amount_minor: i64,
    pub share_pct: f64,
    pub account_ids: Vec<String>,
    pub derived: bool,
}

/// Display basis used for archive-style dashboard allocation summaries.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DashboardAllocationBasis {
    PersonalBuffer,
    ReserveComposition,
}

/// Dashboard-ready allocation summary that preserves the archive's group-specific semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardAllocationSummary {
    pub basis: DashboardAllocationBasis,
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
    pub segments: Vec<AllocationSegment>,
}

/// Full allocation snapshot for one group, including raw account composition and dashboard-ready data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupAllocationSnapshot {
    pub group_id: String,
    pub group_label: String,
    pub net_total_minor: i64,
    pub positive_total_minor: i64,
    pub account_segments: Vec<AllocationSegment>,
    pub dashboard: DashboardAllocationSummary,
}

#[derive(Debug, Clone, Default)]
struct SegmentAccumulator {
    amount_minor: i64,
    account_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct SegmentInput {
    bucket: AllocationBucket,
    amount_minor: i64,
    account_ids: Vec<String>,
    derived: bool,
}

pub fn report_group_allocation(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
) -> Result<GroupAllocationSnapshot> {
    let current_month = current_reporting_month(connection)?;
    report_group_allocation_for_month(connection, config, group_id, &current_month)
}

pub fn report_group_allocation_for_month(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    current_month: &str,
) -> Result<GroupAllocationSnapshot> {
    let metadata = config.resolve_group_metadata(group_id);
    let account_rows = view_accounts(connection, config, Some(group_id))?;
    let account_buckets = collect_account_buckets(config, &account_rows);
    let account_segments = materialize_segments(
        account_buckets
            .iter()
            .map(|(bucket, accumulator)| SegmentInput {
                bucket: *bucket,
                amount_minor: accumulator.amount_minor,
                account_ids: accumulator.account_ids.clone(),
                derived: false,
            })
            .collect(),
    );
    let net_total_minor = account_rows
        .iter()
        .map(|row| row.balance_minor.unwrap_or(0))
        .sum::<i64>();
    let positive_total_minor = account_rows
        .iter()
        .map(|row| row.balance_minor.unwrap_or(0).max(0))
        .sum::<i64>();

    let dashboard = if group_id == "personal" {
        let cashflow = group_monthly_cashflow(connection, config, group_id, None, None, 120)?;
        let cashflow_kpis = summarize_cashflow_kpis(&cashflow, current_month);
        build_personal_dashboard_summary(
            &account_buckets,
            i64::from(metadata.expense_reserve_months),
            cashflow_kpis.median_spend_minor.unwrap_or(0),
        )
    } else {
        let latest_reserve_target = report_reserves(connection, config, group_id, None, None)?
            .into_iter()
            .last();
        build_reserve_dashboard_summary(
            net_total_minor,
            latest_reserve_target
                .as_ref()
                .map(|point| point.expense_reserve_minor)
                .unwrap_or(0),
            latest_reserve_target
                .as_ref()
                .map(|point| point.tax_reserve_minor)
                .unwrap_or(0),
            account_rows.iter().map(|row| row.id.clone()).collect(),
        )
    };

    Ok(GroupAllocationSnapshot {
        group_id: group_id.to_owned(),
        group_label: metadata.label,
        net_total_minor,
        positive_total_minor,
        account_segments,
        dashboard,
    })
}

fn collect_account_buckets(
    config: &FinConfig,
    rows: &[AccountBalanceRow],
) -> BTreeMap<AllocationBucket, SegmentAccumulator> {
    let mut buckets = BTreeMap::<AllocationBucket, SegmentAccumulator>::new();
    for row in rows {
        let amount_minor = row.balance_minor.unwrap_or(0);
        if amount_minor == 0 {
            continue;
        }
        let bucket = config
            .account_by_id(&row.id)
            .map(classify_account_bucket)
            .unwrap_or(AllocationBucket::AvailableCash);
        let entry = buckets.entry(bucket).or_default();
        entry.amount_minor += amount_minor;
        entry.account_ids.push(row.id.clone());
    }
    for accumulator in buckets.values_mut() {
        accumulator.account_ids.sort();
    }
    buckets
}

fn classify_account_bucket(account: &AccountConfig) -> AllocationBucket {
    let Some(raw_subtype) = account.subtype.as_deref() else {
        return AllocationBucket::AvailableCash;
    };
    let subtype = raw_subtype.trim().to_ascii_lowercase();
    match subtype.as_str() {
        "" => AllocationBucket::AvailableCash,
        "checking" | "cash" | "current" | "available_cash" => AllocationBucket::AvailableCash,
        "expense_reserve" => AllocationBucket::ExpenseReserve,
        "tax_reserve" => AllocationBucket::TaxReserve,
        "emergency" | "emergency_fund" => AllocationBucket::EmergencyFund,
        "savings" => {
            if account.group == "personal" {
                AllocationBucket::EmergencyFund
            } else {
                AllocationBucket::Savings
            }
        }
        "investment" => AllocationBucket::Investment,
        _ => AllocationBucket::Other,
    }
}

fn build_personal_dashboard_summary(
    account_buckets: &BTreeMap<AllocationBucket, SegmentAccumulator>,
    expense_reserve_months: i64,
    median_spend_minor: i64,
) -> DashboardAllocationSummary {
    let checking_minor = bucket_amount(account_buckets, AllocationBucket::AvailableCash);
    let emergency_fund_minor = bucket_amount(account_buckets, AllocationBucket::EmergencyFund);
    let investment_minor = bucket_amount(account_buckets, AllocationBucket::Investment);
    let expense_reserve_minor = median_spend_minor.max(0) * expense_reserve_months.max(0);
    let expense_reserve_display_minor = checking_minor.max(0).min(expense_reserve_minor);
    let available_minor = (checking_minor - expense_reserve_minor).max(0);
    let shortfall_minor = (expense_reserve_minor - checking_minor.max(0)).max(0);

    let display_segments = materialize_segments(vec![
        SegmentInput {
            bucket: AllocationBucket::AvailableCash,
            amount_minor: available_minor,
            account_ids: bucket_account_ids(account_buckets, AllocationBucket::AvailableCash),
            derived: true,
        },
        SegmentInput {
            bucket: AllocationBucket::ExpenseReserve,
            amount_minor: expense_reserve_display_minor,
            account_ids: bucket_account_ids(account_buckets, AllocationBucket::AvailableCash),
            derived: true,
        },
        SegmentInput {
            bucket: AllocationBucket::EmergencyFund,
            amount_minor: emergency_fund_minor,
            account_ids: bucket_account_ids(account_buckets, AllocationBucket::EmergencyFund),
            derived: false,
        },
        SegmentInput {
            bucket: AllocationBucket::Investment,
            amount_minor: investment_minor,
            account_ids: bucket_account_ids(account_buckets, AllocationBucket::Investment),
            derived: false,
        },
    ]);
    let display_total_minor = display_segments
        .iter()
        .map(|segment| segment.amount_minor)
        .sum();

    DashboardAllocationSummary {
        basis: DashboardAllocationBasis::PersonalBuffer,
        balance_basis_minor: checking_minor,
        display_total_minor,
        available_minor,
        expense_reserve_minor,
        expense_reserve_display_minor,
        tax_reserve_minor: 0,
        emergency_fund_minor,
        savings_minor: 0,
        investment_minor,
        shortfall_minor,
        under_reserved: shortfall_minor > 0,
        segments: display_segments,
    }
}

fn build_reserve_dashboard_summary(
    balance_minor: i64,
    expense_reserve_minor: i64,
    tax_reserve_minor: i64,
    account_ids: Vec<String>,
) -> DashboardAllocationSummary {
    let available_raw_minor = balance_minor - expense_reserve_minor - tax_reserve_minor;
    let available_minor = available_raw_minor.max(0);
    let shortfall_minor = (-available_raw_minor).max(0);
    let display_segments = materialize_segments(vec![
        SegmentInput {
            bucket: AllocationBucket::AvailableCash,
            amount_minor: available_minor,
            account_ids: account_ids.clone(),
            derived: true,
        },
        SegmentInput {
            bucket: AllocationBucket::ExpenseReserve,
            amount_minor: expense_reserve_minor.max(0),
            account_ids: Vec::new(),
            derived: true,
        },
        SegmentInput {
            bucket: AllocationBucket::TaxReserve,
            amount_minor: tax_reserve_minor.max(0),
            account_ids: Vec::new(),
            derived: true,
        },
    ]);
    let display_total_minor = display_segments
        .iter()
        .map(|segment| segment.amount_minor)
        .sum();

    DashboardAllocationSummary {
        basis: DashboardAllocationBasis::ReserveComposition,
        balance_basis_minor: balance_minor,
        display_total_minor,
        available_minor,
        expense_reserve_minor: expense_reserve_minor.max(0),
        expense_reserve_display_minor: expense_reserve_minor.max(0),
        tax_reserve_minor: tax_reserve_minor.max(0),
        emergency_fund_minor: 0,
        savings_minor: 0,
        investment_minor: 0,
        shortfall_minor,
        under_reserved: shortfall_minor > 0,
        segments: display_segments,
    }
}

fn bucket_amount(
    account_buckets: &BTreeMap<AllocationBucket, SegmentAccumulator>,
    bucket: AllocationBucket,
) -> i64 {
    account_buckets
        .get(&bucket)
        .map(|value| value.amount_minor)
        .unwrap_or(0)
}

fn bucket_account_ids(
    account_buckets: &BTreeMap<AllocationBucket, SegmentAccumulator>,
    bucket: AllocationBucket,
) -> Vec<String> {
    account_buckets
        .get(&bucket)
        .map(|value| value.account_ids.clone())
        .unwrap_or_default()
}

fn materialize_segments(inputs: Vec<SegmentInput>) -> Vec<AllocationSegment> {
    let positive_total_minor = inputs
        .iter()
        .map(|input| input.amount_minor.max(0))
        .sum::<i64>();
    let mut segments = inputs
        .into_iter()
        .filter(|input| input.amount_minor != 0)
        .map(|mut input| {
            input.account_ids.sort();
            AllocationSegment {
                bucket: input.bucket,
                label: input.bucket.label().to_owned(),
                amount_minor: input.amount_minor,
                share_pct: share_pct(input.amount_minor, positive_total_minor),
                account_ids: input.account_ids,
                derived: input.derived,
            }
        })
        .collect::<Vec<_>>();
    segments.sort_by(|left, right| left.bucket.cmp(&right.bucket));
    segments
}

fn share_pct(amount_minor: i64, positive_total_minor: i64) -> f64 {
    if amount_minor <= 0 || positive_total_minor <= 0 {
        return 0.0;
    }
    round_ratio((amount_minor as f64 / positive_total_minor as f64) * 100.0)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        AllocationBucket, DashboardAllocationBasis, build_reserve_dashboard_summary,
        classify_account_bucket, collect_account_buckets, report_group_allocation_for_month,
    };
    use crate::config::AccountConfig;
    use crate::runtime::{RuntimeContext, RuntimeContextOptions};
    use crate::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    #[test]
    fn fixture_personal_allocation_uses_personal_buffer_semantics() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        let runtime = RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime");

        let snapshot = report_group_allocation_for_month(
            runtime.connection(),
            runtime.config(),
            "personal",
            "2026-03",
        )
        .expect("build personal allocation");

        assert_eq!(
            snapshot.dashboard.basis,
            DashboardAllocationBasis::PersonalBuffer
        );
        assert!(snapshot.net_total_minor > 0);
        assert!(snapshot.positive_total_minor > 0);
        assert!(
            snapshot
                .account_segments
                .iter()
                .any(|segment| segment.bucket == AllocationBucket::EmergencyFund)
        );
        assert!(
            snapshot
                .account_segments
                .iter()
                .any(|segment| segment.bucket == AllocationBucket::Investment)
        );
        assert!(
            snapshot
                .dashboard
                .segments
                .iter()
                .any(|segment| segment.bucket == AllocationBucket::ExpenseReserve)
        );
        assert!(
            snapshot.dashboard.expense_reserve_minor
                >= snapshot.dashboard.expense_reserve_display_minor
        );
        assert_eq!(
            snapshot
                .account_segments
                .iter()
                .map(|segment| segment.amount_minor)
                .sum::<i64>(),
            snapshot.net_total_minor
        );
    }

    #[test]
    fn fixture_business_allocation_exposes_actual_savings_and_reserve_dashboard() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        let runtime = RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime");

        let snapshot = report_group_allocation_for_month(
            runtime.connection(),
            runtime.config(),
            "business",
            "2026-03",
        )
        .expect("build business allocation");

        assert_eq!(
            snapshot.dashboard.basis,
            DashboardAllocationBasis::ReserveComposition
        );
        assert!(
            snapshot
                .account_segments
                .iter()
                .any(|segment| segment.bucket == AllocationBucket::Savings)
        );
        assert!(
            snapshot
                .account_segments
                .iter()
                .any(|segment| segment.bucket == AllocationBucket::TaxReserve)
        );
        assert!(
            snapshot
                .dashboard
                .segments
                .iter()
                .any(|segment| segment.bucket == AllocationBucket::ExpenseReserve)
        );
        assert!(snapshot.dashboard.expense_reserve_minor > 0);
        assert!(snapshot.dashboard.tax_reserve_minor >= 0);
    }

    #[test]
    fn fixture_joint_savings_stays_savings_not_emergency() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        let runtime = RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime");

        let rows =
            crate::queries::view_accounts(runtime.connection(), runtime.config(), Some("joint"))
                .expect("view accounts");
        let buckets = collect_account_buckets(runtime.config(), &rows);

        assert!(buckets.contains_key(&AllocationBucket::Savings));
        assert!(!buckets.contains_key(&AllocationBucket::EmergencyFund));
    }

    #[test]
    fn missing_subtype_defaults_to_available_cash() {
        let account = AccountConfig {
            id: "Assets:Personal:Cash".to_owned(),
            group: "personal".to_owned(),
            account_type: "asset".to_owned(),
            provider: "synthetic".to_owned(),
            label: Some("Cash".to_owned()),
            subtype: None,
            inbox_folder: None,
        };

        assert_eq!(
            classify_account_bucket(&account),
            AllocationBucket::AvailableCash
        );
    }

    #[test]
    fn reserve_dashboard_clamps_negative_available_into_shortfall() {
        let dashboard = build_reserve_dashboard_summary(
            125_000,
            110_000,
            40_000,
            vec!["Assets:Business:Operating".to_owned()],
        );

        assert_eq!(dashboard.available_minor, 0);
        assert_eq!(dashboard.shortfall_minor, 25_000);
        assert!(dashboard.under_reserved);
        assert!(
            !dashboard
                .segments
                .iter()
                .any(|segment| segment.bucket == AllocationBucket::AvailableCash)
        );
    }

    #[test]
    fn unknown_explicit_subtype_isolated_into_other_bucket() {
        let account = AccountConfig {
            id: "Assets:Personal:Crypto".to_owned(),
            group: "personal".to_owned(),
            account_type: "asset".to_owned(),
            provider: "synthetic".to_owned(),
            label: Some("Crypto".to_owned()),
            subtype: Some("speculative".to_owned()),
            inbox_folder: None,
        };

        assert_eq!(classify_account_bucket(&account), AllocationBucket::Other);
    }
}

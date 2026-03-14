use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::str::FromStr;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::FinConfig;
use crate::error::Result;
use crate::reports::{ReserveBreakdownPoint, report_reserves};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipMode {
    Gross,
    UserShare,
}

impl FromStr for OwnershipMode {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gross" => Ok(Self::Gross),
            "user-share" | "user_share" => Ok(Self::UserShare),
            _ => Err(format!("unsupported ownership mode: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BurnWindowMode {
    ExplicitRange,
    TrailingClosedMonths,
    TrailingIncludingPartialMonth,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BurnBucketKind {
    RecurringBaseline,
    PeriodicObligation,
    NonRecurring,
    VatPassThrough,
    TransferExcluded,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RecurrenceKind {
    Monthly,
    VariableRecurring,
    Quarterly,
    Annual,
    OneOff,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PeriodicKind {
    Vat,
    CorporationTax,
    SelfAssessment,
    Paye,
    OtherTax,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TwoPoolScenarioKind {
    Config,
    TaxEfficient,
    Custom,
}

impl FromStr for TwoPoolScenarioKind {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "config" => Ok(Self::Config),
            "tax-efficient" | "tax_efficient" => Ok(Self::TaxEfficient),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("unsupported scenario: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BurnBucketSummary {
    pub total_minor: i64,
    pub monthly_equivalent_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BurnItemSummary {
    pub label: String,
    pub total_minor: i64,
    pub transaction_count: usize,
    pub recurrence: RecurrenceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BurnGroupSummary {
    pub group_id: String,
    pub gross_minor: i64,
    pub effective_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BurnMonthlyPoint {
    pub month: String,
    pub recurring_baseline_minor: i64,
    pub periodic_obligations_minor: i64,
    pub non_recurring_minor: i64,
    pub vat_pass_through_minor: i64,
    pub transfers_excluded_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BurnReport {
    pub from_date: String,
    pub to_date: String,
    pub requested_to_date: String,
    pub window_mode: BurnWindowMode,
    pub includes_partial_month: bool,
    pub ownership_mode: OwnershipMode,
    pub groups: Vec<String>,
    pub group_totals: Vec<BurnGroupSummary>,
    pub recurring_baseline: BurnBucketSummary,
    pub periodic_obligations: BurnBucketSummary,
    pub non_recurring: BurnBucketSummary,
    pub vat_pass_through: BurnBucketSummary,
    pub transfers_excluded: BurnBucketSummary,
    pub periodic_items: Vec<BurnItemSummary>,
    pub non_recurring_items: Vec<BurnItemSummary>,
    pub monthly_series: Vec<BurnMonthlyPoint>,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractionValveSummary {
    pub salary_monthly_minor: i64,
    pub dividends_monthly_minor: i64,
    pub total_monthly_minor: i64,
    pub include_joint_expenses: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TwoPoolScenarioSummary {
    pub scenario: TwoPoolScenarioKind,
    pub scenario_source: String,
    pub salary_monthly_minor: i64,
    pub dividends_monthly_minor: i64,
    pub include_joint_expenses: bool,
    pub lookback_months: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TwoPoolRunwayPool {
    pub balance_minor: i64,
    pub recurring_burn_minor: i64,
    pub depletion_rate_minor: i64,
    pub runway_months: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TwoPoolRunwayReport {
    pub as_of_date: String,
    pub ownership_mode: OwnershipMode,
    pub scenario: TwoPoolScenarioKind,
    pub scenario_source: String,
    pub assumptions_applied: TwoPoolScenarioSummary,
    pub warnings: Vec<String>,
    pub business_pool: TwoPoolRunwayPool,
    pub personal_pool: TwoPoolRunwayPool,
    pub extraction_valve: ExtractionValveSummary,
    pub constraint_pool: String,
    pub constraint_months: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct BurnReportOptions<'a> {
    pub months: usize,
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub ownership_mode: OwnershipMode,
    pub include_partial_month: bool,
}

impl Default for BurnReportOptions<'_> {
    fn default() -> Self {
        Self {
            months: 12,
            from: None,
            to: None,
            ownership_mode: OwnershipMode::Gross,
            include_partial_month: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TwoPoolRunwayOptions<'a> {
    pub months: usize,
    pub to: Option<&'a str>,
    pub ownership_mode: OwnershipMode,
    pub scenario: TwoPoolScenarioKind,
    pub salary_monthly_minor: Option<i64>,
    pub dividends_monthly_minor: Option<i64>,
    pub include_joint_expenses: Option<bool>,
}

impl Default for TwoPoolRunwayOptions<'_> {
    fn default() -> Self {
        Self {
            months: 12,
            to: None,
            ownership_mode: OwnershipMode::Gross,
            scenario: TwoPoolScenarioKind::TaxEfficient,
            salary_monthly_minor: None,
            dividends_monthly_minor: None,
            include_joint_expenses: None,
        }
    }
}

#[derive(Debug, Clone)]
struct BurnPosting {
    account_id: String,
    account_type: String,
    amount_minor: i64,
}

#[derive(Debug, Clone)]
struct BurnEntry {
    id: String,
    posted_date: String,
    is_transfer: bool,
    description: String,
    raw_description: String,
    clean_description: String,
    counterparty: Option<String>,
    postings: Vec<BurnPosting>,
}

#[derive(Debug, Clone)]
struct EntryOutflowPortion {
    group_id: String,
    gross_minor: i64,
    effective_minor: i64,
}

#[derive(Debug, Clone)]
struct ClassifiedEntry {
    month: String,
    label: String,
    bucket: BurnBucketKind,
    periodic_kind: Option<PeriodicKind>,
    recurrence: RecurrenceKind,
    effective_minor: i64,
    portions: Vec<EntryOutflowPortion>,
}

#[derive(Debug, Clone)]
struct RecurrenceObservation {
    month: String,
    amount_minor: i64,
}

#[derive(Debug, Clone, Copy)]
struct RecurrenceAssessment {
    kind: RecurrenceKind,
    baseline_eligible: bool,
}

#[derive(Debug, Clone)]
struct ResolvedBurnWindow {
    from_date: String,
    to_date: String,
    requested_to_date: String,
    window_mode: BurnWindowMode,
    includes_partial_month: bool,
}

#[derive(Debug, Clone)]
struct ResolvedTwoPoolScenario {
    summary: TwoPoolScenarioSummary,
    warnings: Vec<String>,
}

pub fn report_burn(
    connection: &Connection,
    config: &FinConfig,
    group_ids: &[String],
    options: &BurnReportOptions<'_>,
) -> Result<BurnReport> {
    let groups = if group_ids.is_empty() {
        config.group_ids()
    } else {
        group_ids.to_vec()
    };
    let window = resolve_burn_window(
        connection,
        options.from,
        options.to,
        options.months,
        options.include_partial_month,
    )?;
    let window_months = months_between_inclusive(&window.from_date, &window.to_date).max(1);

    let included_asset_prefixes = scoped_asset_prefixes(config, &groups);
    let owned_asset_prefixes = scoped_asset_prefixes(config, &config.group_ids());
    let household_counterparties = config
        .financial_array_strings("household_counterparties")
        .into_iter()
        .map(|value| normalize_label(&value))
        .collect::<BTreeSet<_>>();

    let entries = load_entries_in_window(connection, &window.from_date, &window.to_date)?;
    let mut classified = Vec::<ClassifiedEntry>::new();
    let mut recurrence_inputs = Vec::<(String, RecurrenceObservation)>::new();

    for entry in entries {
        let Some(portions) = classify_outflow_portions(
            config,
            &entry,
            &included_asset_prefixes,
            options.ownership_mode,
        ) else {
            continue;
        };
        if portions.iter().all(|portion| portion.effective_minor == 0) {
            continue;
        }

        let effective_minor = portions
            .iter()
            .map(|portion| portion.effective_minor)
            .sum::<i64>();
        let month = year_month(&entry.posted_date);
        let label = preferred_label(&entry);
        let normalized_label = normalize_label(&label);
        let tax_kind = detect_tax_kind(config, &entry, &portions);

        let bucket =
            if is_internal_transfer_entry(&entry, &owned_asset_prefixes, &household_counterparties)
            {
                BurnBucketKind::TransferExcluded
            } else if tax_kind == Some(PeriodicKind::Vat) {
                BurnBucketKind::VatPassThrough
            } else if tax_kind.is_some() {
                BurnBucketKind::PeriodicObligation
            } else {
                BurnBucketKind::RecurringBaseline
            };

        if bucket == BurnBucketKind::RecurringBaseline {
            recurrence_inputs.push((
                normalized_label.clone(),
                RecurrenceObservation {
                    month: month.clone(),
                    amount_minor: effective_minor,
                },
            ));
        }

        classified.push(ClassifiedEntry {
            month,
            label: if normalized_label.is_empty() {
                "unknown".to_owned()
            } else {
                label
            },
            bucket,
            periodic_kind: tax_kind,
            recurrence: RecurrenceKind::Unknown,
            effective_minor,
            portions,
        });
    }

    let recurrence_by_label = infer_recurrence_by_label(&recurrence_inputs, window_months);
    for entry in &mut classified {
        if entry.bucket != BurnBucketKind::RecurringBaseline {
            continue;
        }
        let assessment = recurrence_by_label
            .get(&normalize_label(&entry.label))
            .copied()
            .unwrap_or(RecurrenceAssessment {
                kind: RecurrenceKind::Unknown,
                baseline_eligible: false,
            });
        entry.recurrence = assessment.kind;
        if !assessment.baseline_eligible {
            entry.bucket = BurnBucketKind::NonRecurring;
        }
    }

    let mut monthly = BTreeMap::<String, BurnMonthlyPoint>::new();
    let mut group_totals = BTreeMap::<String, (i64, i64)>::new();
    let mut periodic_items = BTreeMap::<String, (i64, usize)>::new();
    let mut non_recurring_items = BTreeMap::<String, (i64, usize, RecurrenceKind)>::new();
    let mut bucket_totals = BTreeMap::<BurnBucketKind, i64>::new();
    let mut unknown_count = 0usize;

    for entry in &classified {
        let slot = monthly
            .entry(entry.month.clone())
            .or_insert_with(|| BurnMonthlyPoint {
                month: entry.month.clone(),
                recurring_baseline_minor: 0,
                periodic_obligations_minor: 0,
                non_recurring_minor: 0,
                vat_pass_through_minor: 0,
                transfers_excluded_minor: 0,
            });

        match entry.bucket {
            BurnBucketKind::RecurringBaseline => {
                slot.recurring_baseline_minor += entry.effective_minor;
            }
            BurnBucketKind::PeriodicObligation => {
                slot.periodic_obligations_minor += entry.effective_minor;
            }
            BurnBucketKind::NonRecurring => {
                slot.non_recurring_minor += entry.effective_minor;
            }
            BurnBucketKind::VatPassThrough => {
                slot.vat_pass_through_minor += entry.effective_minor;
            }
            BurnBucketKind::TransferExcluded => {
                slot.transfers_excluded_minor += entry.effective_minor;
            }
        }

        *bucket_totals.entry(entry.bucket).or_default() += entry.effective_minor;
        if entry.recurrence == RecurrenceKind::Unknown {
            unknown_count += 1;
        }

        for portion in &entry.portions {
            let group_slot = group_totals.entry(portion.group_id.clone()).or_default();
            group_slot.0 += portion.gross_minor;
            group_slot.1 += portion.effective_minor;
        }

        if entry.bucket == BurnBucketKind::PeriodicObligation {
            let label = match entry.periodic_kind.unwrap_or(PeriodicKind::OtherTax) {
                PeriodicKind::Vat => "vat".to_owned(),
                PeriodicKind::CorporationTax => "corporation_tax".to_owned(),
                PeriodicKind::SelfAssessment => "self_assessment".to_owned(),
                PeriodicKind::Paye => "paye".to_owned(),
                PeriodicKind::OtherTax => "other_tax".to_owned(),
            };
            let item = periodic_items.entry(label).or_default();
            item.0 += entry.effective_minor;
            item.1 += 1;
        }

        if entry.bucket == BurnBucketKind::NonRecurring {
            let item =
                non_recurring_items
                    .entry(entry.label.clone())
                    .or_insert((0, 0, entry.recurrence));
            item.0 += entry.effective_minor;
            item.1 += 1;
        }
    }

    let recurring_total = *bucket_totals
        .get(&BurnBucketKind::RecurringBaseline)
        .unwrap_or(&0);
    let periodic_total = *bucket_totals
        .get(&BurnBucketKind::PeriodicObligation)
        .unwrap_or(&0);
    let non_recurring_total = *bucket_totals
        .get(&BurnBucketKind::NonRecurring)
        .unwrap_or(&0);
    let vat_total = *bucket_totals
        .get(&BurnBucketKind::VatPassThrough)
        .unwrap_or(&0);
    let transfers_total = *bucket_totals
        .get(&BurnBucketKind::TransferExcluded)
        .unwrap_or(&0);

    let periodic_items = periodic_items
        .into_iter()
        .map(
            |(label, (total_minor, transaction_count))| BurnItemSummary {
                label,
                total_minor,
                transaction_count,
                recurrence: RecurrenceKind::Unknown,
            },
        )
        .collect::<Vec<_>>();

    let mut non_recurring_items = non_recurring_items
        .into_iter()
        .map(
            |(label, (total_minor, transaction_count, recurrence))| BurnItemSummary {
                label,
                total_minor,
                transaction_count,
                recurrence,
            },
        )
        .collect::<Vec<_>>();
    non_recurring_items.sort_by(|left, right| {
        right
            .total_minor
            .cmp(&left.total_minor)
            .then(left.label.cmp(&right.label))
    });

    let group_totals = groups
        .iter()
        .map(|group_id| {
            let (gross_minor, effective_minor) =
                group_totals.get(group_id).copied().unwrap_or_default();
            BurnGroupSummary {
                group_id: group_id.clone(),
                gross_minor,
                effective_minor,
            }
        })
        .collect::<Vec<_>>();

    Ok(BurnReport {
        from_date: window.from_date.clone(),
        to_date: window.to_date.clone(),
        requested_to_date: window.requested_to_date.clone(),
        window_mode: window.window_mode,
        includes_partial_month: window.includes_partial_month,
        ownership_mode: options.ownership_mode,
        groups,
        group_totals,
        recurring_baseline: BurnBucketSummary {
            total_minor: recurring_total,
            monthly_equivalent_minor: recurring_total / i64::try_from(window_months).unwrap_or(1),
        },
        periodic_obligations: BurnBucketSummary {
            total_minor: periodic_total,
            monthly_equivalent_minor: periodic_total / i64::try_from(window_months).unwrap_or(1),
        },
        non_recurring: BurnBucketSummary {
            total_minor: non_recurring_total,
            monthly_equivalent_minor: non_recurring_total
                / i64::try_from(window_months).unwrap_or(1),
        },
        vat_pass_through: BurnBucketSummary {
            total_minor: vat_total,
            monthly_equivalent_minor: vat_total / i64::try_from(window_months).unwrap_or(1),
        },
        transfers_excluded: BurnBucketSummary {
            total_minor: transfers_total,
            monthly_equivalent_minor: transfers_total / i64::try_from(window_months).unwrap_or(1),
        },
        periodic_items,
        non_recurring_items,
        monthly_series: enumerate_months(&window.from_date, &window.to_date)
            .into_iter()
            .map(|month| {
                monthly.remove(&month).unwrap_or(BurnMonthlyPoint {
                    month,
                    recurring_baseline_minor: 0,
                    periodic_obligations_minor: 0,
                    non_recurring_minor: 0,
                    vat_pass_through_minor: 0,
                    transfers_excluded_minor: 0,
                })
            })
            .collect(),
        confidence: if classified.is_empty() || unknown_count == 0 {
            "high".to_owned()
        } else if unknown_count * 4 <= classified.len() {
            "medium".to_owned()
        } else {
            "low".to_owned()
        },
    })
}

pub fn report_two_pool_runway(
    connection: &Connection,
    config: &FinConfig,
    options: &TwoPoolRunwayOptions<'_>,
) -> Result<TwoPoolRunwayReport> {
    let as_of_date = resolved_as_of_date(connection, options.to)?;
    let scenario = resolve_two_pool_scenario(config, options)?;
    let business_reserves =
        report_reserves(connection, config, "business", None, Some(&as_of_date))?;
    let business_available_minor = latest_available_minor(&business_reserves);

    let business_burn = report_burn(
        connection,
        config,
        &["business".to_owned()],
        &BurnReportOptions {
            months: options.months,
            from: None,
            to: Some(&as_of_date),
            ownership_mode: OwnershipMode::Gross,
            include_partial_month: false,
        },
    )?;
    let personal_burn = report_burn(
        connection,
        config,
        &["personal".to_owned()],
        &BurnReportOptions {
            months: options.months,
            from: None,
            to: Some(&as_of_date),
            ownership_mode: OwnershipMode::Gross,
            include_partial_month: false,
        },
    )?;
    let joint_burn = report_burn(
        connection,
        config,
        &["joint".to_owned()],
        &BurnReportOptions {
            months: options.months,
            from: None,
            to: Some(&as_of_date),
            ownership_mode: options.ownership_mode,
            include_partial_month: false,
        },
    )?;

    let include_joint_expenses = scenario.summary.include_joint_expenses;
    let salary_monthly_minor = scenario.summary.salary_monthly_minor;
    let dividends_monthly_minor = scenario.summary.dividends_monthly_minor;
    let extraction_valve_minor = salary_monthly_minor + dividends_monthly_minor;

    let personal_balance_minor =
        liquid_balance_for_group(connection, config, "personal", Some(&as_of_date), false)?;
    let joint_balance_minor =
        liquid_balance_for_group(connection, config, "joint", Some(&as_of_date), false)?;
    let personal_pool_balance_minor = personal_balance_minor
        + if options.ownership_mode == OwnershipMode::UserShare {
            scale_minor(joint_balance_minor, config.joint_share_you())
        } else {
            joint_balance_minor
        };

    let personal_recurring_burn_minor = personal_burn.recurring_baseline.monthly_equivalent_minor
        + if include_joint_expenses {
            joint_burn.recurring_baseline.monthly_equivalent_minor
        } else {
            0
        };
    let business_recurring_burn_minor = business_burn.recurring_baseline.monthly_equivalent_minor;
    let personal_depletion_minor = (personal_recurring_burn_minor - extraction_valve_minor).max(0);
    let business_depletion_minor = business_recurring_burn_minor + extraction_valve_minor;
    let personal_runway_months =
        runway_months_for_balance(personal_pool_balance_minor, personal_depletion_minor);
    let business_runway_months =
        runway_months_for_balance(business_available_minor, business_depletion_minor);

    let (constraint_pool, constraint_months) = if personal_runway_months <= business_runway_months {
        ("personal_pool".to_owned(), personal_runway_months)
    } else {
        ("business_pool".to_owned(), business_runway_months)
    };

    Ok(TwoPoolRunwayReport {
        as_of_date,
        ownership_mode: options.ownership_mode,
        scenario: scenario.summary.scenario,
        scenario_source: scenario.summary.scenario_source.clone(),
        assumptions_applied: scenario.summary.clone(),
        warnings: scenario.warnings,
        business_pool: TwoPoolRunwayPool {
            balance_minor: business_available_minor,
            recurring_burn_minor: business_recurring_burn_minor,
            depletion_rate_minor: business_depletion_minor,
            runway_months: business_runway_months,
        },
        personal_pool: TwoPoolRunwayPool {
            balance_minor: personal_pool_balance_minor,
            recurring_burn_minor: personal_recurring_burn_minor,
            depletion_rate_minor: personal_depletion_minor,
            runway_months: personal_runway_months,
        },
        extraction_valve: ExtractionValveSummary {
            salary_monthly_minor,
            dividends_monthly_minor,
            total_monthly_minor: extraction_valve_minor,
            include_joint_expenses,
        },
        constraint_pool,
        constraint_months,
    })
}

#[derive(Debug)]
struct EntryRow {
    journal_entry_id: String,
    posted_date: String,
    is_transfer: bool,
    description: String,
    raw_description: Option<String>,
    clean_description: Option<String>,
    counterparty: Option<String>,
    account_id: String,
    account_type: String,
    amount_minor: i64,
}

fn load_entries_in_window(
    connection: &Connection,
    from_date: &str,
    to_date: &str,
) -> Result<Vec<BurnEntry>> {
    let sql = "SELECT je.id,\n                     je.posted_date,\n                     je.is_transfer,\n                     je.description,\n                     je.raw_description,\n                     je.clean_description,\n                     je.counterparty,\n                     p.account_id,\n                     coa.account_type,\n                     p.amount_minor\n              FROM journal_entries je\n              JOIN postings p ON p.journal_entry_id = je.id\n              JOIN chart_of_accounts coa ON coa.id = p.account_id\n              WHERE je.posted_date >= ?1\n                AND je.posted_date <= ?2\n              ORDER BY je.posted_at ASC, je.id ASC, p.id ASC";
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map([from_date, to_date], |row| {
        Ok(EntryRow {
            journal_entry_id: row.get(0)?,
            posted_date: row.get(1)?,
            is_transfer: row.get::<usize, i64>(2)? == 1,
            description: row.get(3)?,
            raw_description: row.get(4)?,
            clean_description: row.get(5)?,
            counterparty: row.get(6)?,
            account_id: row.get(7)?,
            account_type: row.get(8)?,
            amount_minor: row.get(9)?,
        })
    })?;

    let mut entries = Vec::<BurnEntry>::new();
    for row in rows {
        let row = row?;
        let start_new = entries
            .last()
            .map(|entry| entry.id != row.journal_entry_id)
            .unwrap_or(true)
            || entries
                .last()
                .map(|entry| entry.posted_date != row.posted_date)
                .unwrap_or(false);

        if start_new {
            entries.push(BurnEntry {
                id: row.journal_entry_id.clone(),
                posted_date: row.posted_date,
                is_transfer: row.is_transfer,
                description: row.description.clone(),
                raw_description: row
                    .raw_description
                    .unwrap_or_else(|| row.description.clone()),
                clean_description: row
                    .clean_description
                    .unwrap_or_else(|| row.description.clone()),
                counterparty: row.counterparty,
                postings: Vec::new(),
            });
        }

        entries
            .last_mut()
            .expect("entry inserted")
            .postings
            .push(BurnPosting {
                account_id: row.account_id,
                account_type: row.account_type,
                amount_minor: row.amount_minor,
            });
    }
    Ok(entries)
}

fn latest_available_minor(points: &[ReserveBreakdownPoint]) -> i64 {
    points
        .last()
        .map(|point| point.available_minor)
        .unwrap_or_default()
}

fn liquid_balance_for_group(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    as_of_date: Option<&str>,
    include_investments: bool,
) -> Result<i64> {
    let account_ids = config
        .accounts
        .iter()
        .filter(|account| account.group == group_id && account.account_type == "asset")
        .filter(|account| include_investments || account.subtype.as_deref() != Some("investment"))
        .map(|account| account.id.clone())
        .collect::<Vec<_>>();

    if account_ids.is_empty() {
        return Ok(0);
    }

    let (asset_clause, mut params) = account_match_clause("p", &account_ids);
    let mut clauses = vec!["coa.account_type = 'asset'".to_owned(), asset_clause];
    if let Some(as_of_date) = as_of_date {
        clauses.push("je.posted_at <= ?".to_owned());
        params.push(format!("{as_of_date}T23:59:59.999"));
    }

    let sql = format!(
        "SELECT COALESCE(SUM(p.amount_minor), 0)\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON coa.id = p.account_id\n         WHERE {}",
        clauses.join(" AND ")
    );
    connection
        .query_row(&sql, rusqlite::params_from_iter(params.iter()), |row| {
            row.get(0)
        })
        .map_err(Into::into)
}

fn runway_months_for_balance(balance_minor: i64, burn_rate_minor: i64) -> f64 {
    if burn_rate_minor <= 0 {
        999.0
    } else {
        (balance_minor as f64) / (burn_rate_minor as f64)
    }
}

fn scoped_asset_prefixes(config: &FinConfig, group_ids: &[String]) -> Vec<String> {
    let scope = group_ids.iter().collect::<BTreeSet<_>>();
    config
        .accounts
        .iter()
        .filter(|account| account.account_type == "asset" && scope.contains(&account.group))
        .map(|account| account.id.clone())
        .collect::<Vec<_>>()
}

fn account_match_clause(alias: &str, account_ids: &[String]) -> (String, Vec<String>) {
    let mut clauses = Vec::new();
    let mut params = Vec::new();
    for account_id in account_ids {
        clauses.push(format!(
            "({alias}.account_id = ? OR {alias}.account_id LIKE ?)"
        ));
        params.push(account_id.clone());
        params.push(format!("{account_id}:%"));
    }
    (format!("({})", clauses.join(" OR ")), params)
}

fn matches_any_account_prefix(account_id: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|prefix| account_id == prefix || account_id.starts_with(&format!("{prefix}:")))
}

fn scale_minor(amount_minor: i64, scale: f64) -> i64 {
    ((amount_minor as f64) * scale).round() as i64
}

fn classify_outflow_portions(
    config: &FinConfig,
    entry: &BurnEntry,
    included_asset_prefixes: &[String],
    ownership_mode: OwnershipMode,
) -> Option<Vec<EntryOutflowPortion>> {
    let mut by_group = BTreeMap::<String, i64>::new();

    for posting in &entry.postings {
        if posting.account_type != "asset" || posting.amount_minor >= 0 {
            continue;
        }
        let Some(group_id) = group_for_account(config, &posting.account_id) else {
            continue;
        };
        if !matches_any_account_prefix(&posting.account_id, included_asset_prefixes) {
            continue;
        }
        *by_group.entry(group_id).or_default() += -posting.amount_minor;
    }

    if by_group.is_empty() {
        return None;
    }

    Some(
        by_group
            .into_iter()
            .map(|(group_id, gross_minor)| {
                let effective_minor =
                    if ownership_mode == OwnershipMode::UserShare && group_id == "joint" {
                        scale_minor(gross_minor, config.joint_share_you())
                    } else {
                        gross_minor
                    };
                EntryOutflowPortion {
                    group_id,
                    gross_minor,
                    effective_minor,
                }
            })
            .collect::<Vec<_>>(),
    )
}

fn group_for_account(config: &FinConfig, account_id: &str) -> Option<String> {
    config
        .accounts
        .iter()
        .filter(|account| account.account_type == "asset")
        .filter(|account| {
            account.id == account_id || account_id.starts_with(&format!("{}:", account.id))
        })
        .max_by_key(|account| account.id.len())
        .map(|account| account.group.clone())
}

fn preferred_label(entry: &BurnEntry) -> String {
    entry
        .counterparty
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            if !entry.clean_description.trim().is_empty() {
                Some(entry.clean_description.clone())
            } else if !entry.raw_description.trim().is_empty() {
                Some(entry.raw_description.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| entry.description.clone())
}

fn normalize_label(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn is_internal_transfer_entry(
    entry: &BurnEntry,
    owned_asset_prefixes: &[String],
    household_counterparties: &BTreeSet<String>,
) -> bool {
    if entry.is_transfer {
        return true;
    }
    if household_counterparties.contains(&normalize_label(
        entry.counterparty.as_deref().unwrap_or_default(),
    )) {
        return true;
    }
    let has_owned_asset_in = entry.postings.iter().any(|posting| {
        posting.account_type == "asset"
            && posting.amount_minor > 0
            && matches_any_account_prefix(&posting.account_id, owned_asset_prefixes)
    });
    if has_owned_asset_in {
        return true;
    }
    entry.postings.iter().any(|posting| {
        posting.account_id == "Equity:Transfers"
            || posting.account_id.starts_with("Equity:Transfers:")
            || posting.account_id == "Equity:Investments"
            || posting.account_id.starts_with("Equity:Investments:")
    })
}

fn detect_tax_kind(
    config: &FinConfig,
    entry: &BurnEntry,
    portions: &[EntryOutflowPortion],
) -> Option<PeriodicKind> {
    let account_ids = entry
        .postings
        .iter()
        .map(|posting| posting.account_id.as_str())
        .collect::<Vec<_>>();
    if account_ids
        .iter()
        .any(|account_id| account_id.starts_with("Expenses:Taxes:VAT"))
    {
        return Some(PeriodicKind::Vat);
    }
    if account_ids
        .iter()
        .any(|account_id| account_id.starts_with("Expenses:Taxes:CorporationTax"))
    {
        return Some(PeriodicKind::CorporationTax);
    }
    if account_ids
        .iter()
        .any(|account_id| account_id.starts_with("Expenses:Taxes:SelfAssessment"))
    {
        return Some(PeriodicKind::SelfAssessment);
    }
    if account_ids
        .iter()
        .any(|account_id| account_id.starts_with("Expenses:Taxes:PAYE"))
    {
        return Some(PeriodicKind::Paye);
    }

    let haystack = format!(
        "{} {} {}",
        normalize_label(entry.counterparty.as_deref().unwrap_or_default()),
        normalize_label(&entry.clean_description),
        normalize_label(&entry.raw_description)
    );

    if haystack.contains("hmrc vat") {
        return Some(PeriodicKind::Vat);
    }
    if !(haystack.contains("hmrc")
        || haystack.contains("cumbernauld")
        || haystack.contains("hm revenue"))
    {
        return None;
    }
    if haystack.contains(" vat") {
        return Some(PeriodicKind::Vat);
    }
    if haystack.contains("paye") || haystack.contains(" nic") || haystack.contains(" ni ") {
        return Some(PeriodicKind::Paye);
    }

    let all_corp = portions.iter().all(|portion| {
        config
            .resolve_group_metadata(&portion.group_id)
            .tax_type
            .eq("corp")
    });
    if all_corp {
        Some(PeriodicKind::CorporationTax)
    } else {
        Some(PeriodicKind::SelfAssessment)
    }
}

fn infer_recurrence_by_label(
    inputs: &[(String, RecurrenceObservation)],
    total_window_months: usize,
) -> HashMap<String, RecurrenceAssessment> {
    let mut observations_by_label = HashMap::<String, Vec<RecurrenceObservation>>::new();
    for (label, observation) in inputs {
        observations_by_label
            .entry(label.clone())
            .or_default()
            .push(observation.clone());
    }

    observations_by_label
        .into_iter()
        .map(|(label, observations)| {
            let recurrence = infer_recurrence(&observations, total_window_months);
            (label, recurrence)
        })
        .collect()
}

fn infer_recurrence(
    observations: &[RecurrenceObservation],
    total_window_months: usize,
) -> RecurrenceAssessment {
    let mut months = observations
        .iter()
        .map(|observation| observation.month.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if months.len() <= 1 {
        return RecurrenceAssessment {
            kind: if observations.len() >= 2 {
                RecurrenceKind::VariableRecurring
            } else {
                RecurrenceKind::OneOff
            },
            baseline_eligible: false,
        };
    }
    months.sort();

    let gaps = months
        .windows(2)
        .filter_map(|window| month_gap(&window[0], &window[1]))
        .collect::<Vec<_>>();
    if gaps.is_empty() {
        return RecurrenceAssessment {
            kind: RecurrenceKind::Unknown,
            baseline_eligible: false,
        };
    }

    let active_months = months.len();
    let tx_count = observations.len();
    let max_gap = *gaps.iter().max().unwrap_or(&0);
    let min_gap = *gaps.iter().min().unwrap_or(&0);
    let median_gap = {
        let mut sorted_gaps = gaps.clone();
        sorted_gaps.sort_unstable();
        sorted_gaps[sorted_gaps.len() / 2]
    };
    let coverage_ratio = if total_window_months == 0 {
        0.0
    } else {
        active_months as f64 / total_window_months as f64
    };
    let average_tx_per_active_month = tx_count as f64 / active_months as f64;
    let amount_stability = {
        let amounts = observations
            .iter()
            .map(|observation| observation.amount_minor.unsigned_abs() as f64)
            .collect::<Vec<_>>();
        let mean = amounts.iter().sum::<f64>() / amounts.len() as f64;
        if mean == 0.0 {
            0.0
        } else {
            let variance = amounts
                .iter()
                .map(|amount| {
                    let delta = amount - mean;
                    delta * delta
                })
                .sum::<f64>()
                / amounts.len() as f64;
            variance.sqrt() / mean
        }
    };

    if active_months >= 2 && max_gap >= 10 {
        return RecurrenceAssessment {
            kind: RecurrenceKind::Annual,
            baseline_eligible: false,
        };
    }
    if active_months >= 2 && min_gap >= 2 && max_gap <= 4 {
        return RecurrenceAssessment {
            kind: RecurrenceKind::Quarterly,
            baseline_eligible: true,
        };
    }
    if active_months >= 2 && max_gap <= 1 {
        if coverage_ratio >= 0.8
            && average_tx_per_active_month <= 1.5
            && (amount_stability <= 0.35 || total_window_months <= 3)
        {
            return RecurrenceAssessment {
                kind: RecurrenceKind::Monthly,
                baseline_eligible: true,
            };
        }
        return RecurrenceAssessment {
            kind: RecurrenceKind::VariableRecurring,
            baseline_eligible: coverage_ratio >= 0.5,
        };
    }
    if tx_count >= 2 && active_months >= 2 && median_gap <= 2 && coverage_ratio >= 0.3 {
        return RecurrenceAssessment {
            kind: RecurrenceKind::VariableRecurring,
            baseline_eligible: coverage_ratio >= 0.5,
        };
    }
    RecurrenceAssessment {
        kind: if tx_count >= 2 {
            RecurrenceKind::VariableRecurring
        } else {
            RecurrenceKind::Unknown
        },
        baseline_eligible: false,
    }
}

fn month_gap(left: &str, right: &str) -> Option<i64> {
    let (left_year, left_month) = parse_year_month(left)?;
    let (right_year, right_month) = parse_year_month(right)?;
    Some(i64::from((right_year - left_year) * 12) + i64::from(right_month) - i64::from(left_month))
}

fn parse_year_month(value: &str) -> Option<(i32, u32)> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    Some((year, month))
}

fn parse_iso_date(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    Some((year, month, day))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let is_leap_year = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if is_leap_year { 29 } else { 28 }
        }
        _ => 30,
    }
}

fn month_end_date(year: i32, month: u32) -> String {
    format!("{year:04}-{month:02}-{:02}", days_in_month(year, month))
}

fn previous_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

fn is_partial_month(date: &str) -> bool {
    let Some((year, month, day)) = parse_iso_date(date) else {
        return false;
    };
    day < days_in_month(year, month)
}

fn last_closed_month_end(date: &str) -> String {
    let Some((year, month, day)) = parse_iso_date(date) else {
        return date.to_owned();
    };
    if day == days_in_month(year, month) {
        return date.to_owned();
    }
    let (closed_year, closed_month) = previous_month(year, month);
    month_end_date(closed_year, closed_month)
}

fn year_month(date: &str) -> String {
    date.chars().take(7).collect()
}

fn resolved_as_of_date(connection: &Connection, as_of: Option<&str>) -> Result<String> {
    if let Some(as_of) = as_of {
        return Ok(as_of.to_owned());
    }
    connection
        .query_row(
            "SELECT COALESCE(MAX(date(posted_at)), date('now')) FROM journal_entries",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn resolve_burn_window(
    connection: &Connection,
    from: Option<&str>,
    to: Option<&str>,
    months: usize,
    include_partial_month: bool,
) -> Result<ResolvedBurnWindow> {
    let requested_to_date = resolved_as_of_date(connection, to)?;
    if let Some(from) = from {
        let includes_partial_month = is_partial_month(&requested_to_date);
        return Ok(ResolvedBurnWindow {
            from_date: from.to_owned(),
            to_date: requested_to_date.clone(),
            requested_to_date,
            window_mode: BurnWindowMode::ExplicitRange,
            includes_partial_month,
        });
    }

    let to_date = if include_partial_month {
        requested_to_date.clone()
    } else {
        last_closed_month_end(&requested_to_date)
    };

    Ok(ResolvedBurnWindow {
        from_date: infer_window_start(&to_date, months.max(1)),
        to_date,
        requested_to_date,
        window_mode: if include_partial_month {
            BurnWindowMode::TrailingIncludingPartialMonth
        } else {
            BurnWindowMode::TrailingClosedMonths
        },
        includes_partial_month: include_partial_month,
    })
}

fn resolve_two_pool_scenario(
    config: &FinConfig,
    options: &TwoPoolRunwayOptions<'_>,
) -> Result<ResolvedTwoPoolScenario> {
    let mut warnings = Vec::<String>::new();
    let has_cli_override = options.salary_monthly_minor.is_some()
        || options.dividends_monthly_minor.is_some()
        || options.include_joint_expenses.is_some();

    let (scenario_source, mut summary) = match options.scenario {
        TwoPoolScenarioKind::Config => {
            let include_salary = config.scenario_toggle("include_salary");
            let include_dividends = config.scenario_toggle("include_dividends");
            let include_joint_expenses = config.scenario_toggle("include_joint_expenses");
            (
                "config".to_owned(),
                TwoPoolScenarioSummary {
                    scenario: TwoPoolScenarioKind::Config,
                    scenario_source: "config".to_owned(),
                    salary_monthly_minor: if include_salary {
                        config
                            .scenario_monthly_minor("salary_monthly_minor")
                            .unwrap_or_default()
                    } else {
                        0
                    },
                    dividends_monthly_minor: if include_dividends {
                        config
                            .scenario_monthly_minor("dividends_monthly_minor")
                            .unwrap_or_default()
                    } else {
                        0
                    },
                    include_joint_expenses,
                    lookback_months: options.months.max(1),
                },
            )
        }
        TwoPoolScenarioKind::TaxEfficient => (
            "preset".to_owned(),
            TwoPoolScenarioSummary {
                scenario: TwoPoolScenarioKind::TaxEfficient,
                scenario_source: "preset".to_owned(),
                salary_monthly_minor: config
                    .scenario_monthly_minor("tax_efficient_salary_monthly_minor")
                    .unwrap_or(104_750),
                dividends_monthly_minor: config
                    .scenario_monthly_minor("tax_efficient_dividends_monthly_minor")
                    .unwrap_or(314_167),
                include_joint_expenses: config
                    .scenario_bool("tax_efficient_include_joint_expenses")
                    .unwrap_or(true),
                lookback_months: options.months.max(1),
            },
        ),
        TwoPoolScenarioKind::Custom => (
            "cli".to_owned(),
            TwoPoolScenarioSummary {
                scenario: TwoPoolScenarioKind::Custom,
                scenario_source: "cli".to_owned(),
                salary_monthly_minor: 0,
                dividends_monthly_minor: 0,
                include_joint_expenses: true,
                lookback_months: options.months.max(1),
            },
        ),
    };

    if options.scenario == TwoPoolScenarioKind::Config {
        warnings.push(
            "using config-driven extraction assumptions; switch to --scenario tax-efficient or pass explicit monthly overrides for decision-grade analysis"
                .to_owned(),
        );
    }

    if let Some(salary_monthly_minor) = options.salary_monthly_minor {
        summary.salary_monthly_minor = salary_monthly_minor.max(0);
    }
    if let Some(dividends_monthly_minor) = options.dividends_monthly_minor {
        summary.dividends_monthly_minor = dividends_monthly_minor.max(0);
    }
    if let Some(include_joint_expenses) = options.include_joint_expenses {
        summary.include_joint_expenses = include_joint_expenses;
    }
    if has_cli_override {
        match options.scenario {
            TwoPoolScenarioKind::Custom => {
                summary.scenario_source = "cli".to_owned();
            }
            _ => {
                summary.scenario_source = format!("{scenario_source}+cli");
                warnings
                    .push("CLI overrides applied on top of the base two-pool scenario".to_owned());
            }
        }
    }

    if options.scenario == TwoPoolScenarioKind::Custom && !has_cli_override {
        return Err(crate::error::FinError::InvalidInput {
            code: "INVALID_INPUT",
            message: "custom two-pool scenario requires at least one explicit override".to_owned(),
        });
    }

    Ok(ResolvedTwoPoolScenario { summary, warnings })
}

fn infer_window_start(to_date: &str, months: usize) -> String {
    let Some((year, month, _)) = parse_iso_date(to_date) else {
        return to_date.to_owned();
    };
    let total_months = i64::from(year) * 12 + i64::from(month) - 1;
    let window_offset = i64::try_from(months.saturating_sub(1)).unwrap_or(0);
    let start_month_index = total_months - window_offset;
    let start_year = i32::try_from(start_month_index.div_euclid(12)).unwrap_or(year);
    let start_month = u32::try_from(start_month_index.rem_euclid(12) + 1).unwrap_or(month);
    format!("{start_year:04}-{start_month:02}-01")
}

fn months_between_inclusive(from_date: &str, to_date: &str) -> usize {
    let Some((from_year, from_month, _)) = parse_iso_date(from_date) else {
        return 1;
    };
    let Some((to_year, to_month, _)) = parse_iso_date(to_date) else {
        return 1;
    };
    let diff = (to_year - from_year) * 12 + i32::try_from(to_month).unwrap_or(0)
        - i32::try_from(from_month).unwrap_or(0);
    usize::try_from(diff.max(0) + 1).unwrap_or(1)
}

fn enumerate_months(from_date: &str, to_date: &str) -> Vec<String> {
    let Some((from_year, from_month, _)) = parse_iso_date(from_date) else {
        return vec![];
    };
    let Some((to_year, to_month, _)) = parse_iso_date(to_date) else {
        return vec![];
    };
    let mut months = Vec::new();
    let mut current_year = from_year;
    let mut current_month = from_month;
    loop {
        months.push(format!("{current_year:04}-{current_month:02}"));
        if current_year == to_year && current_month == to_month {
            break;
        }
        current_month += 1;
        if current_month > 12 {
            current_month = 1;
            current_year += 1;
        }
    }
    months
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{
        BurnBucketKind, BurnReportOptions, BurnWindowMode, OwnershipMode, PeriodicKind,
        RecurrenceObservation, TwoPoolRunwayOptions, TwoPoolScenarioKind, infer_recurrence,
        report_burn, report_two_pool_runway,
    };
    use crate::config::parse_fin_config;
    use crate::db::{ensure_chart_of_accounts_seeded, migrate_to_latest};

    #[test]
    fn recurrence_detection_identifies_monthly_variable_and_one_off() {
        let monthly = infer_recurrence(
            &[
                RecurrenceObservation {
                    month: "2025-09".to_owned(),
                    amount_minor: 1_000,
                },
                RecurrenceObservation {
                    month: "2025-10".to_owned(),
                    amount_minor: 1_000,
                },
                RecurrenceObservation {
                    month: "2025-11".to_owned(),
                    amount_minor: 1_000,
                },
            ],
            3,
        );
        assert_eq!(monthly.kind, super::RecurrenceKind::Monthly);
        assert!(monthly.baseline_eligible);

        let variable = infer_recurrence(
            &[
                RecurrenceObservation {
                    month: "2025-09".to_owned(),
                    amount_minor: 1_000,
                },
                RecurrenceObservation {
                    month: "2025-10".to_owned(),
                    amount_minor: 500,
                },
                RecurrenceObservation {
                    month: "2025-10".to_owned(),
                    amount_minor: 700,
                },
                RecurrenceObservation {
                    month: "2025-11".to_owned(),
                    amount_minor: 900,
                },
            ],
            6,
        );
        assert_eq!(variable.kind, super::RecurrenceKind::VariableRecurring);
        assert!(variable.baseline_eligible);

        let clustered = infer_recurrence(
            &[
                RecurrenceObservation {
                    month: "2025-11".to_owned(),
                    amount_minor: 1_000,
                },
                RecurrenceObservation {
                    month: "2025-11".to_owned(),
                    amount_minor: 900,
                },
                RecurrenceObservation {
                    month: "2025-11".to_owned(),
                    amount_minor: 1_100,
                },
            ],
            6,
        );
        assert_eq!(clustered.kind, super::RecurrenceKind::VariableRecurring);
        assert!(!clustered.baseline_eligible);

        let one_off = infer_recurrence(
            &[RecurrenceObservation {
                month: "2025-09".to_owned(),
                amount_minor: 2_000,
            }],
            6,
        );
        assert_eq!(one_off.kind, super::RecurrenceKind::OneOff);
        assert!(!one_off.baseline_eligible);
    }

    #[test]
    fn burn_report_separates_transfers_vat_periodic_and_non_recurring() {
        let mut connection = Connection::open_in_memory().expect("open sqlite");
        migrate_to_latest(&mut connection).expect("migrate schema");
        let config = parse_fin_config(
            r#"
[financial]
joint_share_you = 0.5

[[groups]]
id = "business"
label = "Business"
tax_type = "corp"

[[groups]]
id = "personal"
label = "Personal"
tax_type = "income"

[[groups]]
id = "joint"
label = "Joint"

[[accounts]]
id = "Assets:Business:Monzo"
group = "business"
type = "asset"
provider = "monzo"

[[accounts]]
id = "Assets:Business:Wise"
group = "business"
type = "asset"
provider = "wise"

[[accounts]]
id = "Assets:Personal:Monzo"
group = "personal"
type = "asset"
provider = "monzo"

[[accounts]]
id = "Assets:Joint:Monzo"
group = "joint"
type = "asset"
provider = "monzo"

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"

[[banks]]
name = "wise"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"
"#,
        )
        .expect("config parses");
        ensure_chart_of_accounts_seeded(&connection, &config).expect("seed chart of accounts");

        connection
            .execute_batch(
                r#"
INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, raw_description, clean_description, counterparty, source_file) VALUES
    ('je-open-business', '2025-08-31T08:00:00', '2025-08-31', 0, 'Open', 'Open', 'Open', 'Fixture', 'fixture.csv'),
    ('je-open-personal', '2025-08-31T08:01:00', '2025-08-31', 0, 'Open', 'Open', 'Open', 'Fixture', 'fixture.csv'),
    ('je-open-joint', '2025-08-31T08:02:00', '2025-08-31', 0, 'Open', 'Open', 'Open', 'Fixture', 'fixture.csv'),
    ('je-saas-sep', '2025-09-05T08:00:00', '2025-09-05', 0, 'SaaS', 'SaaS', 'SaaS', 'Claude', 'fixture.csv'),
    ('je-saas-oct', '2025-10-05T08:00:00', '2025-10-05', 0, 'SaaS', 'SaaS', 'SaaS', 'Claude', 'fixture.csv'),
    ('je-rent-sep', '2025-09-01T09:00:00', '2025-09-01', 0, 'Rent', 'Rent', 'Rent', 'Landlord', 'fixture.csv'),
    ('je-rent-oct', '2025-10-01T09:00:00', '2025-10-01', 0, 'Rent', 'Rent', 'Rent', 'Landlord', 'fixture.csv'),
    ('je-vat', '2025-10-10T12:00:00', '2025-10-10', 0, 'HMRC VAT', 'HMRC VAT', 'HMRC VAT', 'HMRC VAT', 'fixture.csv'),
    ('je-sa', '2025-10-20T12:00:00', '2025-10-20', 0, 'HMRC Cumbernauld', 'HMRC Cumbernauld', 'HMRC Cumbernauld', 'HMRC Cumbernauld', 'fixture.csv'),
    ('je-transfer', '2025-10-22T12:00:00', '2025-10-22', 1, 'Transfer', 'Transfer', 'Transfer', 'Transfer', 'fixture.csv'),
    ('je-setup', '2025-10-25T12:00:00', '2025-10-25', 0, 'Setup fee', 'Setup fee', 'Setup fee', 'Companies House', 'fixture.csv');

INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency) VALUES
    ('p-open-business-asset', 'je-open-business', 'Assets:Business:Monzo', 100000, 'GBP'),
    ('p-open-business-equity', 'je-open-business', 'Equity:OpeningBalances', -100000, 'GBP'),
    ('p-open-personal-asset', 'je-open-personal', 'Assets:Personal:Monzo', 80000, 'GBP'),
    ('p-open-personal-equity', 'je-open-personal', 'Equity:OpeningBalances', -80000, 'GBP'),
    ('p-open-joint-asset', 'je-open-joint', 'Assets:Joint:Monzo', 60000, 'GBP'),
    ('p-open-joint-equity', 'je-open-joint', 'Equity:OpeningBalances', -60000, 'GBP'),
    ('p-saas-sep-asset', 'je-saas-sep', 'Assets:Business:Monzo', -1000, 'GBP'),
    ('p-saas-sep-expense', 'je-saas-sep', 'Expenses:Business:Software', 1000, 'GBP'),
    ('p-saas-oct-asset', 'je-saas-oct', 'Assets:Business:Monzo', -1000, 'GBP'),
    ('p-saas-oct-expense', 'je-saas-oct', 'Expenses:Business:Software', 1000, 'GBP'),
    ('p-rent-sep-asset', 'je-rent-sep', 'Assets:Joint:Monzo', -2000, 'GBP'),
    ('p-rent-sep-expense', 'je-rent-sep', 'Expenses:Housing:Rent', 2000, 'GBP'),
    ('p-rent-oct-asset', 'je-rent-oct', 'Assets:Joint:Monzo', -2000, 'GBP'),
    ('p-rent-oct-expense', 'je-rent-oct', 'Expenses:Housing:Rent', 2000, 'GBP'),
    ('p-vat-asset', 'je-vat', 'Assets:Business:Monzo', -3000, 'GBP'),
    ('p-vat-expense', 'je-vat', 'Expenses:Taxes:VAT', 3000, 'GBP'),
    ('p-sa-asset', 'je-sa', 'Assets:Personal:Monzo', -4000, 'GBP'),
    ('p-sa-expense', 'je-sa', 'Expenses:Taxes:HMRC', 4000, 'GBP'),
    ('p-transfer-out', 'je-transfer', 'Assets:Personal:Monzo', -5000, 'GBP'),
    ('p-transfer-in', 'je-transfer', 'Assets:Business:Wise', 5000, 'GBP'),
    ('p-setup-asset', 'je-setup', 'Assets:Business:Monzo', -1500, 'GBP'),
    ('p-setup-expense', 'je-setup', 'Expenses:Other', 1500, 'GBP');
"#,
            )
            .expect("seed ledger");

        let report = report_burn(
            &connection,
            &config,
            &[
                "business".to_owned(),
                "personal".to_owned(),
                "joint".to_owned(),
            ],
            &BurnReportOptions {
                months: 2,
                from: Some("2025-09-01"),
                to: Some("2025-10-31"),
                ownership_mode: OwnershipMode::UserShare,
                include_partial_month: false,
            },
        )
        .expect("burn report");

        assert_eq!(report.window_mode, BurnWindowMode::ExplicitRange);
        assert_eq!(report.recurring_baseline.total_minor, 4_000);
        assert_eq!(report.periodic_obligations.total_minor, 4_000);
        assert_eq!(report.vat_pass_through.total_minor, 3_000);
        assert_eq!(report.non_recurring.total_minor, 1_500);
        assert_eq!(report.transfers_excluded.total_minor, 5_000);
        assert!(
            report
                .periodic_items
                .iter()
                .any(|item| item.label == "self_assessment" && item.total_minor == 4_000)
        );
        assert!(
            report
                .non_recurring_items
                .iter()
                .any(|item| item.label == "Companies House" && item.total_minor == 1_500)
        );
        assert!(
            report
                .group_totals
                .iter()
                .any(|group| { group.group_id == "joint" && group.effective_minor == 2_000 })
        );
        assert_eq!(
            report.monthly_series[0].recurring_baseline_minor
                + report.monthly_series[1].recurring_baseline_minor,
            4_000
        );

        let transfer_bucket = classified_bucket(&report, BurnBucketKind::TransferExcluded);
        assert_eq!(transfer_bucket, 5_000);
        let vat_bucket = classified_periodic(&report, PeriodicKind::Vat);
        assert_eq!(vat_bucket, 0);
    }

    #[test]
    fn burn_report_defaults_to_closed_months_when_to_date_is_partial() {
        let mut connection = Connection::open_in_memory().expect("open sqlite");
        migrate_to_latest(&mut connection).expect("migrate schema");
        let config = parse_fin_config(
            r#"
[financial]

[[groups]]
id = "personal"
label = "Personal"
tax_type = "income"

[[accounts]]
id = "Assets:Personal:Monzo"
group = "personal"
type = "asset"
provider = "monzo"

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"
"#,
        )
        .expect("config parses");
        ensure_chart_of_accounts_seeded(&connection, &config).expect("seed chart of accounts");

        connection
            .execute_batch(
                r#"
INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, raw_description, clean_description, counterparty, source_file) VALUES
    ('je-open', '2026-01-01T08:00:00', '2026-01-01', 0, 'Open', 'Open', 'Open', 'Fixture', 'fixture.csv'),
    ('je-jan', '2026-01-10T08:00:00', '2026-01-10', 0, 'Groceries', 'Groceries', 'Groceries', 'Groceries Market', 'fixture.csv'),
    ('je-feb', '2026-02-10T08:00:00', '2026-02-10', 0, 'Groceries', 'Groceries', 'Groceries', 'Groceries Market', 'fixture.csv'),
    ('je-mar', '2026-03-10T08:00:00', '2026-03-10', 0, 'Groceries', 'Groceries', 'Groceries', 'Groceries Market', 'fixture.csv');

INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency) VALUES
    ('po-open-asset', 'je-open', 'Assets:Personal:Monzo', 100000, 'GBP'),
    ('po-open-equity', 'je-open', 'Equity:OpeningBalances', -100000, 'GBP'),
    ('po-jan-asset', 'je-jan', 'Assets:Personal:Monzo', -1000, 'GBP'),
    ('po-jan-expense', 'je-jan', 'Expenses:Food:Groceries', 1000, 'GBP'),
    ('po-feb-asset', 'je-feb', 'Assets:Personal:Monzo', -1200, 'GBP'),
    ('po-feb-expense', 'je-feb', 'Expenses:Food:Groceries', 1200, 'GBP'),
    ('po-mar-asset', 'je-mar', 'Assets:Personal:Monzo', -900, 'GBP'),
    ('po-mar-expense', 'je-mar', 'Expenses:Food:Groceries', 900, 'GBP');
"#,
            )
            .expect("seed ledger");

        let closed = report_burn(
            &connection,
            &config,
            &["personal".to_owned()],
            &BurnReportOptions {
                months: 2,
                from: None,
                to: Some("2026-03-14"),
                ownership_mode: OwnershipMode::Gross,
                include_partial_month: false,
            },
        )
        .expect("closed-month burn");
        let partial = report_burn(
            &connection,
            &config,
            &["personal".to_owned()],
            &BurnReportOptions {
                months: 2,
                from: None,
                to: Some("2026-03-14"),
                ownership_mode: OwnershipMode::Gross,
                include_partial_month: true,
            },
        )
        .expect("partial burn");

        assert_eq!(closed.window_mode, BurnWindowMode::TrailingClosedMonths);
        assert_eq!(closed.to_date, "2026-02-28");
        assert_eq!(closed.from_date, "2026-01-01");
        assert_eq!(closed.recurring_baseline.total_minor, 2_200);

        assert_eq!(
            partial.window_mode,
            BurnWindowMode::TrailingIncludingPartialMonth
        );
        assert_eq!(partial.to_date, "2026-03-14");
        assert_eq!(partial.from_date, "2026-02-01");
        assert_eq!(partial.recurring_baseline.total_minor, 2_100);
    }

    #[test]
    fn two_pool_runway_surfaces_tax_efficient_and_config_scenarios() {
        let mut connection = Connection::open_in_memory().expect("open sqlite");
        migrate_to_latest(&mut connection).expect("migrate schema");
        let config = parse_fin_config(
            r#"
[financial]
joint_share_you = 0.5

[financial.scenario]
salary_monthly_minor = 300000
dividends_monthly_minor = 600000
tax_efficient_salary_monthly_minor = 104750
tax_efficient_dividends_monthly_minor = 314167
tax_efficient_include_joint_expenses = true

[financial.scenario.toggles]
include_salary = true
include_dividends = true
include_joint_expenses = true

[[groups]]
id = "business"
label = "Business"
tax_type = "corp"

[[groups]]
id = "personal"
label = "Personal"
tax_type = "income"

[[groups]]
id = "joint"
label = "Joint"
tax_type = "none"

[[accounts]]
id = "Assets:Business:Monzo"
group = "business"
type = "asset"
provider = "monzo"

[[accounts]]
id = "Assets:Personal:Monzo"
group = "personal"
type = "asset"
provider = "monzo"

[[accounts]]
id = "Assets:Joint:Monzo"
group = "joint"
type = "asset"
provider = "monzo"

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"
"#,
        )
        .expect("config parses");
        ensure_chart_of_accounts_seeded(&connection, &config).expect("seed chart of accounts");

        connection
            .execute_batch(
                r#"
INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, raw_description, clean_description, counterparty, source_file) VALUES
    ('je-open-business', '2026-01-01T08:00:00', '2026-01-01', 0, 'Open', 'Open', 'Open', 'Fixture', 'fixture.csv'),
    ('je-open-personal', '2026-01-01T08:01:00', '2026-01-01', 0, 'Open', 'Open', 'Open', 'Fixture', 'fixture.csv'),
    ('je-open-joint', '2026-01-01T08:02:00', '2026-01-01', 0, 'Open', 'Open', 'Open', 'Fixture', 'fixture.csv'),
    ('je-business-jan', '2026-01-10T08:00:00', '2026-01-10', 0, 'Software', 'Software', 'Software', 'Vendor', 'fixture.csv'),
    ('je-business-feb', '2026-02-10T08:00:00', '2026-02-10', 0, 'Software', 'Software', 'Software', 'Vendor', 'fixture.csv'),
    ('je-personal-jan', '2026-01-11T08:00:00', '2026-01-11', 0, 'Groceries', 'Groceries', 'Groceries', 'Asda', 'fixture.csv'),
    ('je-personal-feb', '2026-02-11T08:00:00', '2026-02-11', 0, 'Groceries', 'Groceries', 'Groceries', 'Tesco', 'fixture.csv'),
    ('je-joint-jan', '2026-01-12T08:00:00', '2026-01-12', 0, 'Rent', 'Rent', 'Rent', 'Landlord', 'fixture.csv'),
    ('je-joint-feb', '2026-02-12T08:00:00', '2026-02-12', 0, 'Rent', 'Rent', 'Rent', 'Landlord', 'fixture.csv');

INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency) VALUES
    ('po-open-business-asset', 'je-open-business', 'Assets:Business:Monzo', 500000, 'GBP'),
    ('po-open-business-equity', 'je-open-business', 'Equity:OpeningBalances', -500000, 'GBP'),
    ('po-open-personal-asset', 'je-open-personal', 'Assets:Personal:Monzo', 200000, 'GBP'),
    ('po-open-personal-equity', 'je-open-personal', 'Equity:OpeningBalances', -200000, 'GBP'),
    ('po-open-joint-asset', 'je-open-joint', 'Assets:Joint:Monzo', 160000, 'GBP'),
    ('po-open-joint-equity', 'je-open-joint', 'Equity:OpeningBalances', -160000, 'GBP'),
    ('po-business-jan-asset', 'je-business-jan', 'Assets:Business:Monzo', -1000, 'GBP'),
    ('po-business-jan-expense', 'je-business-jan', 'Expenses:Business:Software', 1000, 'GBP'),
    ('po-business-feb-asset', 'je-business-feb', 'Assets:Business:Monzo', -1000, 'GBP'),
    ('po-business-feb-expense', 'je-business-feb', 'Expenses:Business:Software', 1000, 'GBP'),
    ('po-personal-jan-asset', 'je-personal-jan', 'Assets:Personal:Monzo', -1200, 'GBP'),
    ('po-personal-jan-expense', 'je-personal-jan', 'Expenses:Food:Groceries', 1200, 'GBP'),
    ('po-personal-feb-asset', 'je-personal-feb', 'Assets:Personal:Monzo', -1200, 'GBP'),
    ('po-personal-feb-expense', 'je-personal-feb', 'Expenses:Food:Groceries', 1200, 'GBP'),
    ('po-joint-jan-asset', 'je-joint-jan', 'Assets:Joint:Monzo', -2000, 'GBP'),
    ('po-joint-jan-expense', 'je-joint-jan', 'Expenses:Housing:Rent', 2000, 'GBP'),
    ('po-joint-feb-asset', 'je-joint-feb', 'Assets:Joint:Monzo', -2000, 'GBP'),
    ('po-joint-feb-expense', 'je-joint-feb', 'Expenses:Housing:Rent', 2000, 'GBP');
"#,
            )
            .expect("seed runway ledger");

        let tax_efficient = report_two_pool_runway(
            &connection,
            &config,
            &TwoPoolRunwayOptions {
                months: 2,
                to: Some("2026-03-14"),
                ownership_mode: OwnershipMode::UserShare,
                scenario: TwoPoolScenarioKind::TaxEfficient,
                salary_monthly_minor: None,
                dividends_monthly_minor: None,
                include_joint_expenses: None,
            },
        )
        .expect("tax-efficient runway");
        let config_based = report_two_pool_runway(
            &connection,
            &config,
            &TwoPoolRunwayOptions {
                months: 2,
                to: Some("2026-03-14"),
                ownership_mode: OwnershipMode::UserShare,
                scenario: TwoPoolScenarioKind::Config,
                salary_monthly_minor: None,
                dividends_monthly_minor: None,
                include_joint_expenses: None,
            },
        )
        .expect("config runway");
        let custom = report_two_pool_runway(
            &connection,
            &config,
            &TwoPoolRunwayOptions {
                months: 2,
                to: Some("2026-03-14"),
                ownership_mode: OwnershipMode::UserShare,
                scenario: TwoPoolScenarioKind::Custom,
                salary_monthly_minor: Some(104_750),
                dividends_monthly_minor: Some(200_000),
                include_joint_expenses: Some(false),
            },
        )
        .expect("custom runway");

        assert_eq!(tax_efficient.scenario, TwoPoolScenarioKind::TaxEfficient);
        assert_eq!(tax_efficient.scenario_source, "preset");
        assert_eq!(
            tax_efficient.assumptions_applied.salary_monthly_minor,
            104_750
        );
        assert_eq!(
            tax_efficient.assumptions_applied.dividends_monthly_minor,
            314_167
        );
        assert!(tax_efficient.warnings.is_empty());

        assert_eq!(config_based.scenario, TwoPoolScenarioKind::Config);
        assert_eq!(
            config_based.assumptions_applied.salary_monthly_minor,
            300_000
        );
        assert_eq!(
            config_based.assumptions_applied.dividends_monthly_minor,
            600_000
        );
        assert!(!config_based.warnings.is_empty());

        assert_eq!(custom.scenario, TwoPoolScenarioKind::Custom);
        assert_eq!(custom.scenario_source, "cli");
        assert_eq!(custom.assumptions_applied.salary_monthly_minor, 104_750);
        assert_eq!(custom.assumptions_applied.dividends_monthly_minor, 200_000);
        assert!(!custom.extraction_valve.include_joint_expenses);
        assert!(custom.warnings.is_empty());
    }

    fn classified_bucket(report: &super::BurnReport, kind: BurnBucketKind) -> i64 {
        match kind {
            BurnBucketKind::RecurringBaseline => report.recurring_baseline.total_minor,
            BurnBucketKind::PeriodicObligation => report.periodic_obligations.total_minor,
            BurnBucketKind::NonRecurring => report.non_recurring.total_minor,
            BurnBucketKind::VatPassThrough => report.vat_pass_through.total_minor,
            BurnBucketKind::TransferExcluded => report.transfers_excluded.total_minor,
        }
    }

    fn classified_periodic(report: &super::BurnReport, kind: PeriodicKind) -> i64 {
        let label = match kind {
            PeriodicKind::Vat => "vat",
            PeriodicKind::CorporationTax => "corporation_tax",
            PeriodicKind::SelfAssessment => "self_assessment",
            PeriodicKind::Paye => "paye",
            PeriodicKind::OtherTax => "other_tax",
        };
        report
            .periodic_items
            .iter()
            .find(|item| item.label == label)
            .map(|item| item.total_minor)
            .unwrap_or_default()
    }
}

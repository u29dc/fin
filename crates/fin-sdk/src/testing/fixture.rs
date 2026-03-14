use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::config::load_config;
use crate::db::{OpenDatabaseOptions, ensure_chart_of_accounts_seeded, open_database};
use crate::error::{FinError, Result};
use crate::queries::{group_asset_account_ids, transaction_counts_by_group};
use crate::rules::load_rules;

const SOURCE_CONFIG_FILE: &str = "fin.config.toml";
const SOURCE_RULES_FILE: &str = "fin.rules.json";
const SOURCE_SPEC_FILE: &str = "fixture-spec.json";

#[derive(Debug, Clone)]
pub struct FixtureBuildOptions {
    pub source_dir: PathBuf,
    pub months_override: Option<usize>,
    pub transaction_scale: usize,
}

impl Default for FixtureBuildOptions {
    fn default() -> Self {
        Self {
            source_dir: committed_fixture_source_dir(),
            months_override: None,
            transaction_scale: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixturePaths {
    pub home_dir: PathBuf,
    pub data_dir: PathBuf,
    pub imports_dir: PathBuf,
    pub inbox_dir: PathBuf,
    pub archive_dir: PathBuf,
    pub config_path: PathBuf,
    pub rules_path: PathBuf,
    pub spec_path: PathBuf,
    pub db_path: PathBuf,
}

impl FixturePaths {
    #[must_use]
    pub fn from_home(home_dir: &Path) -> Self {
        let data_dir = home_dir.join("data");
        let imports_dir = home_dir.join("imports");
        Self {
            home_dir: home_dir.to_path_buf(),
            data_dir: data_dir.clone(),
            imports_dir: imports_dir.clone(),
            inbox_dir: imports_dir.join("inbox"),
            archive_dir: imports_dir.join("archive"),
            config_path: data_dir.join(SOURCE_CONFIG_FILE),
            rules_path: data_dir.join(SOURCE_RULES_FILE),
            spec_path: home_dir.join(SOURCE_SPEC_FILE),
            db_path: data_dir.join("fin.db"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureMaterialization {
    pub paths: FixturePaths,
    pub stats: FixtureStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixtureStats {
    pub seed: String,
    pub months: usize,
    pub account_count: usize,
    pub journal_entries: usize,
    pub postings: usize,
    pub transfer_entries: usize,
    pub first_posted_at: String,
    pub last_posted_at: String,
    pub groups: BTreeMap<String, GroupFixtureStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GroupFixtureStats {
    pub income_entries: usize,
    pub expense_entries: usize,
    pub transfer_entries: usize,
    pub other_entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureSpec {
    pub seed: String,
    pub start_month: String,
    pub months: usize,
    pub groups: Vec<GroupFixtureSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupFixtureSpec {
    pub id: String,
    pub monthly_income_minor: i64,
    pub monthly_expense_target_minor: i64,
    pub tx_per_month: usize,
    pub reserve_transfers: Vec<ReserveTransferSpec>,
    pub anomaly_months: Vec<AnomalyExpenseSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveTransferSpec {
    pub target_account_id: String,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnomalyExpenseSpec {
    pub month_index: usize,
    pub amount_minor: i64,
    pub expense_account_id: String,
    pub description: String,
    pub counterparty: String,
}

#[derive(Debug, Clone)]
struct GroupRuntimePlan {
    primary_asset_account_id: String,
    income_account_id: String,
    return_income_account_id: Option<String>,
    return_asset_account_id: Option<String>,
    opening_balances: Vec<(&'static str, i64)>,
    expense_templates: Vec<ExpenseTemplate>,
}

#[derive(Debug, Clone)]
struct ExpenseTemplate {
    expense_account_id: &'static str,
    description: &'static str,
    counterparty: &'static str,
}

#[derive(Debug, Clone, Copy)]
enum EntryKind {
    Income,
    Expense,
    Transfer,
    Other,
}

#[derive(Debug, Default)]
struct IdGenerator {
    journal_index: usize,
    posting_index: usize,
}

impl IdGenerator {
    fn next_journal_id(&mut self) -> String {
        self.journal_index += 1;
        format!("je-{:05}", self.journal_index)
    }

    fn next_posting_id(&mut self) -> String {
        self.posting_index += 1;
        format!("po-{:06}", self.posting_index)
    }
}

#[derive(Debug)]
struct JournalPosting {
    account_id: String,
    amount_minor: i64,
    provider_txn_id: Option<String>,
    provider_balance_minor: Option<i64>,
}

#[derive(Debug)]
struct JournalEntrySeed {
    group_id: String,
    entry_kind: EntryKind,
    is_transfer: bool,
    description: String,
    counterparty: Option<String>,
    source_file: Option<String>,
    posted_at: String,
    postings: Vec<JournalPosting>,
}

#[derive(Debug)]
struct BuildState {
    ids: IdGenerator,
    balances: BTreeMap<String, i64>,
    stats: FixtureStats,
}

impl BuildState {
    fn new(stats: FixtureStats, balances: BTreeMap<String, i64>) -> Self {
        Self {
            ids: IdGenerator::default(),
            balances,
            stats,
        }
    }

    fn into_stats(self) -> FixtureStats {
        self.stats
    }

    fn insert_opening_balances(
        &mut self,
        tx: &Transaction<'_>,
        loaded: &crate::config::LoadedConfig,
        year: i32,
        month: u32,
    ) -> Result<()> {
        let opening_day = 2;
        for group_id in loaded.config.group_ids() {
            let runtime = runtime_plan(&group_id)?;
            for (asset_account_id, amount_minor) in runtime.opening_balances {
                self.insert_entry(
                    tx,
                    JournalEntrySeed {
                        group_id: group_id.clone(),
                        entry_kind: EntryKind::Other,
                        is_transfer: false,
                        description: format!(
                            "Opening balance {}",
                            short_account_label(asset_account_id)
                        ),
                        counterparty: Some("Fixture Seed".to_owned()),
                        source_file: Some(format!("fixture/{group_id}/opening.csv")),
                        posted_at: iso_timestamp(year, month, opening_day, 8, 15),
                        postings: vec![
                            JournalPosting {
                                account_id: asset_account_id.to_owned(),
                                amount_minor,
                                provider_txn_id: Some(format!(
                                    "seed-{group_id}-{asset_account_id}"
                                )),
                                provider_balance_minor: None,
                            },
                            JournalPosting {
                                account_id: "Equity:OpeningBalances".to_owned(),
                                amount_minor: -amount_minor,
                                provider_txn_id: None,
                                provider_balance_minor: None,
                            },
                        ],
                    },
                )?;
            }
        }
        Ok(())
    }

    fn populate_group_month(
        &mut self,
        tx: &Transaction<'_>,
        spec: &GroupFixtureSpec,
        year: i32,
        month: u32,
        month_index: usize,
        transaction_scale: usize,
    ) -> Result<()> {
        let runtime = runtime_plan(&spec.id)?;
        let source_file = format!("fixture/{}/{:04}-{:02}.csv", spec.id, year, month);
        let income_variation = ((month_index * 17 + spec.id.len()) % 9) as i64 * 7_500;
        let income_minor = spec.monthly_income_minor + income_variation - 22_500;
        let income_description = match spec.id.as_str() {
            "business" => format!("Client retainer {:04}-{:02}", year, month),
            "joint" => format!("Household contribution {:04}-{:02}", year, month),
            _ => format!("Salary payroll {:04}-{:02}", year, month),
        };
        let income_counterparty = match spec.id.as_str() {
            "business" => "Anchor Client",
            "joint" => "Household Pool",
            _ => "Employer Ltd",
        };
        self.insert_entry(
            tx,
            JournalEntrySeed {
                group_id: spec.id.clone(),
                entry_kind: EntryKind::Income,
                is_transfer: false,
                description: income_description,
                counterparty: Some(income_counterparty.to_owned()),
                source_file: Some(source_file.clone()),
                posted_at: iso_timestamp(year, month, 3, 9, 30),
                postings: vec![
                    JournalPosting {
                        account_id: runtime.primary_asset_account_id.clone(),
                        amount_minor: income_minor,
                        provider_txn_id: Some(format!(
                            "income-{}-{:04}{:02}",
                            spec.id, year, month
                        )),
                        provider_balance_minor: None,
                    },
                    JournalPosting {
                        account_id: runtime.income_account_id.clone(),
                        amount_minor: -income_minor,
                        provider_txn_id: None,
                        provider_balance_minor: None,
                    },
                ],
            },
        )?;

        let expense_count = spec.tx_per_month.saturating_mul(transaction_scale);
        let base_amount = std::cmp::max(
            1_800,
            spec.monthly_expense_target_minor / i64::try_from(expense_count).unwrap_or(1),
        );
        for expense_index in 0..expense_count {
            let template = &runtime.expense_templates
                [(month_index + expense_index) % runtime.expense_templates.len()];
            let weight =
                82 + i64::try_from((month_index * 11 + expense_index * 7) % 39).unwrap_or(0);
            let amount_minor = std::cmp::max(900, base_amount * weight / 100);
            self.insert_entry(
                tx,
                JournalEntrySeed {
                    group_id: spec.id.clone(),
                    entry_kind: EntryKind::Expense,
                    is_transfer: false,
                    description: format!(
                        "{} {:02}/{:02}",
                        template.description,
                        (expense_index % 28) + 1,
                        month
                    ),
                    counterparty: Some(template.counterparty.to_owned()),
                    source_file: Some(source_file.clone()),
                    posted_at: iso_timestamp(year, month, 4 + (expense_index % 22) as u32, 12, 0),
                    postings: vec![
                        JournalPosting {
                            account_id: runtime.primary_asset_account_id.clone(),
                            amount_minor: -amount_minor,
                            provider_txn_id: Some(format!(
                                "expense-{}-{:04}{:02}-{:03}",
                                spec.id, year, month, expense_index
                            )),
                            provider_balance_minor: None,
                        },
                        JournalPosting {
                            account_id: template.expense_account_id.to_owned(),
                            amount_minor,
                            provider_txn_id: None,
                            provider_balance_minor: None,
                        },
                    ],
                },
            )?;
        }

        for anomaly in &spec.anomaly_months {
            if anomaly.month_index != month_index {
                continue;
            }
            self.insert_entry(
                tx,
                JournalEntrySeed {
                    group_id: spec.id.clone(),
                    entry_kind: EntryKind::Expense,
                    is_transfer: false,
                    description: anomaly.description.clone(),
                    counterparty: Some(anomaly.counterparty.clone()),
                    source_file: Some(source_file.clone()),
                    posted_at: iso_timestamp(year, month, 24, 16, 10),
                    postings: vec![
                        JournalPosting {
                            account_id: runtime.primary_asset_account_id.clone(),
                            amount_minor: -anomaly.amount_minor,
                            provider_txn_id: Some(format!(
                                "anomaly-{}-{:04}{:02}-{}",
                                spec.id, year, month, anomaly.month_index
                            )),
                            provider_balance_minor: None,
                        },
                        JournalPosting {
                            account_id: anomaly.expense_account_id.clone(),
                            amount_minor: anomaly.amount_minor,
                            provider_txn_id: None,
                            provider_balance_minor: None,
                        },
                    ],
                },
            )?;
        }

        for (transfer_index, transfer) in spec.reserve_transfers.iter().enumerate() {
            self.insert_entry(
                tx,
                JournalEntrySeed {
                    group_id: spec.id.clone(),
                    entry_kind: EntryKind::Transfer,
                    is_transfer: true,
                    description: format!(
                        "Reserve sweep {}",
                        short_account_label(&transfer.target_account_id)
                    ),
                    counterparty: Some("Internal transfer".to_owned()),
                    source_file: Some(source_file.clone()),
                    posted_at: iso_timestamp(year, month, 26 + transfer_index as u32, 10, 45),
                    postings: vec![
                        JournalPosting {
                            account_id: runtime.primary_asset_account_id.clone(),
                            amount_minor: -transfer.amount_minor,
                            provider_txn_id: None,
                            provider_balance_minor: None,
                        },
                        JournalPosting {
                            account_id: transfer.target_account_id.clone(),
                            amount_minor: transfer.amount_minor,
                            provider_txn_id: None,
                            provider_balance_minor: None,
                        },
                    ],
                },
            )?;
        }

        if let (Some(return_asset_account_id), Some(return_income_account_id)) = (
            runtime.return_asset_account_id.as_ref(),
            runtime.return_income_account_id.as_ref(),
        ) {
            let return_minor = 8_500 + i64::try_from((month_index % 6) * 1_250).unwrap_or(0);
            self.insert_entry(
                tx,
                JournalEntrySeed {
                    group_id: spec.id.clone(),
                    entry_kind: EntryKind::Income,
                    is_transfer: false,
                    description: format!("Dividend reinvestment {:04}-{:02}", year, month),
                    counterparty: Some("Brokerage".to_owned()),
                    source_file: Some(source_file),
                    posted_at: iso_timestamp(year, month, 28, 15, 20),
                    postings: vec![
                        JournalPosting {
                            account_id: return_asset_account_id.clone(),
                            amount_minor: return_minor,
                            provider_txn_id: Some(format!(
                                "return-{}-{:04}{:02}",
                                spec.id, year, month
                            )),
                            provider_balance_minor: None,
                        },
                        JournalPosting {
                            account_id: return_income_account_id.clone(),
                            amount_minor: -return_minor,
                            provider_txn_id: None,
                            provider_balance_minor: None,
                        },
                    ],
                },
            )?;
        }

        Ok(())
    }

    fn insert_entry(&mut self, tx: &Transaction<'_>, mut seed: JournalEntrySeed) -> Result<()> {
        let journal_id = self.ids.next_journal_id();
        let posted_date = &seed.posted_at[..10];
        tx.execute(
            "INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, raw_description, clean_description, counterparty, source_file, created_at, updated_at)\n         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &journal_id,
                &seed.posted_at,
                posted_date,
                i32::from(seed.is_transfer),
                &seed.description,
                &seed.description,
                &seed.description,
                seed.counterparty.as_deref(),
                seed.source_file.as_deref(),
                &seed.posted_at,
                &seed.posted_at,
            ],
        )?;

        for posting in &mut seed.postings {
            if self.balances.contains_key(&posting.account_id) {
                let next_balance = self
                    .balances
                    .get(&posting.account_id)
                    .copied()
                    .unwrap_or_default()
                    + posting.amount_minor;
                self.balances
                    .insert(posting.account_id.clone(), next_balance);
                posting.provider_balance_minor = Some(next_balance);
            }
            tx.execute(
                "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor, created_at)\n             VALUES (?1, ?2, ?3, ?4, 'GBP', NULL, ?5, ?6, ?7)",
                params![
                    self.ids.next_posting_id(),
                    &journal_id,
                    &posting.account_id,
                    posting.amount_minor,
                    posting.provider_txn_id.as_deref(),
                    posting.provider_balance_minor,
                    &seed.posted_at,
                ],
            )?;
            self.stats.postings += 1;
        }

        self.stats.journal_entries += 1;
        if self.stats.first_posted_at.is_empty()
            || seed.posted_at.as_str() < self.stats.first_posted_at.as_str()
        {
            self.stats.first_posted_at = seed.posted_at.clone();
        }
        if self.stats.last_posted_at.is_empty()
            || seed.posted_at.as_str() > self.stats.last_posted_at.as_str()
        {
            self.stats.last_posted_at = seed.posted_at.clone();
        }

        let group_stats = self.stats.groups.entry(seed.group_id).or_default();
        match seed.entry_kind {
            EntryKind::Income => group_stats.income_entries += 1,
            EntryKind::Expense => group_stats.expense_entries += 1,
            EntryKind::Transfer => {
                group_stats.transfer_entries += 1;
                self.stats.transfer_entries += 1;
            }
            EntryKind::Other => group_stats.other_entries += 1,
        }

        Ok(())
    }
}

#[must_use]
pub fn committed_fixture_source_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("benchmark")
}

pub fn fixture_paths(home_dir: &Path) -> FixturePaths {
    FixturePaths::from_home(home_dir)
}

pub fn load_fixture_spec(path: &Path) -> Result<FixtureSpec> {
    let raw = fs::read_to_string(path).map_err(|error| FinError::Io {
        message: format!("{}: {error}", path.display()),
    })?;
    serde_json::from_str(&raw).map_err(|error| FinError::Parse {
        context: "fixture-spec.json",
        message: error.to_string(),
    })
}

pub fn reset_fixture_db(db_path: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        let candidate = if suffix.is_empty() {
            db_path.to_path_buf()
        } else {
            PathBuf::from(format!("{}{}", db_path.display(), suffix))
        };
        if candidate.exists() {
            fs::remove_file(&candidate).map_err(|error| FinError::Io {
                message: format!("{}: {error}", candidate.display()),
            })?;
        }
    }
    Ok(())
}

pub fn materialize_fixture_home(
    home_dir: &Path,
    options: &FixtureBuildOptions,
) -> Result<FixtureMaterialization> {
    let paths = fixture_paths(home_dir);
    fs::create_dir_all(&paths.data_dir)?;
    fs::create_dir_all(&paths.inbox_dir)?;
    fs::create_dir_all(&paths.archive_dir)?;

    copy_fixture_sources(&paths, options)?;
    reset_fixture_db(&paths.db_path)?;
    let stats = build_fixture_ledger(&paths, options)?;

    Ok(FixtureMaterialization { paths, stats })
}

pub fn build_fixture_ledger(
    paths: &FixturePaths,
    options: &FixtureBuildOptions,
) -> Result<FixtureStats> {
    let loaded = load_config(Some(&paths.config_path))?;
    let _rules = load_rules(Some(&paths.rules_path), Some(&loaded), None)?;
    let mut spec = load_fixture_spec(&paths.spec_path)?;
    if let Some(months_override) = options.months_override {
        spec.months = months_override;
    }

    let mut connection = open_database(OpenDatabaseOptions {
        path: Some(paths.db_path.clone()),
        config_dir: Some(paths.data_dir.clone()),
        create: true,
        readonly: false,
        migrate: true,
    })?;
    ensure_chart_of_accounts_seeded(&connection, &loaded.config)?;

    let stats = FixtureStats {
        seed: spec.seed.clone(),
        months: spec.months,
        account_count: loaded.config.accounts.len(),
        journal_entries: 0,
        postings: 0,
        transfer_entries: 0,
        first_posted_at: String::new(),
        last_posted_at: String::new(),
        groups: BTreeMap::new(),
    };
    let balances = opening_balance_map(&loaded);
    let mut state = BuildState::new(stats, balances);

    let transaction_scale = options.transaction_scale.max(1);
    let (start_year, start_month) = parse_year_month(&spec.start_month)?;
    let (opening_year, opening_month) = offset_year_month(start_year, start_month, -1);

    let tx = connection.transaction()?;
    state.insert_opening_balances(&tx, &loaded, opening_year, opening_month)?;

    for month_index in 0..spec.months {
        let (year, month) = offset_year_month(start_year, start_month, month_index as i32);
        for group in &spec.groups {
            state.populate_group_month(&tx, group, year, month, month_index, transaction_scale)?;
        }
    }
    tx.commit()?;

    let mut stats = state.into_stats();
    let counts = transaction_counts_by_group(&connection, &loaded.config)?;
    for (group_id, count) in counts {
        stats.groups.entry(group_id).or_default().other_entries +=
            usize::try_from(count).unwrap_or_default();
    }
    for group_stats in stats.groups.values_mut() {
        group_stats.other_entries = group_stats.other_entries.saturating_sub(
            group_stats.income_entries + group_stats.expense_entries + group_stats.transfer_entries,
        );
    }

    Ok(stats)
}

fn copy_fixture_sources(paths: &FixturePaths, options: &FixtureBuildOptions) -> Result<()> {
    let config_source = options.source_dir.join(SOURCE_CONFIG_FILE);
    let rules_source = options.source_dir.join(SOURCE_RULES_FILE);
    let spec_source = options.source_dir.join(SOURCE_SPEC_FILE);
    copy_file(&config_source, &paths.config_path)?;
    copy_file(&rules_source, &paths.rules_path)?;
    copy_file(&spec_source, &paths.spec_path)?;
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> Result<()> {
    let raw = fs::read(source).map_err(|error| FinError::Io {
        message: format!("{}: {error}", source.display()),
    })?;
    fs::write(destination, raw).map_err(|error| FinError::Io {
        message: format!("{}: {error}", destination.display()),
    })
}

fn opening_balance_map(config: &crate::config::LoadedConfig) -> BTreeMap<String, i64> {
    let mut balances = BTreeMap::new();
    for account in &config.config.accounts {
        balances.insert(account.id.clone(), 0);
    }
    balances
}

fn runtime_plan(group_id: &str) -> Result<GroupRuntimePlan> {
    let plan = match group_id {
        "personal" => GroupRuntimePlan {
            primary_asset_account_id: "Assets:Personal:Checking".to_owned(),
            income_account_id: "Income:Salary".to_owned(),
            return_income_account_id: Some("Income:Dividends".to_owned()),
            return_asset_account_id: Some("Assets:Personal:Investments".to_owned()),
            opening_balances: vec![
                ("Assets:Personal:Checking", 480_000),
                ("Assets:Personal:Emergency", 850_000),
                ("Assets:Personal:Investments", 2_250_000),
            ],
            expense_templates: vec![
                ExpenseTemplate {
                    expense_account_id: "Expenses:Food:Groceries",
                    description: "Grocery run",
                    counterparty: "Supermarket",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Food:Restaurants",
                    description: "Dining out",
                    counterparty: "Neighbourhood Cafe",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Transport:PublicTransport",
                    description: "Tube travel",
                    counterparty: "TfL",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Entertainment:Subscriptions",
                    description: "Media subscription",
                    counterparty: "Streaming Co",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Health:Fitness",
                    description: "Gym membership",
                    counterparty: "Health Club",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Shopping:Home",
                    description: "Home supplies",
                    counterparty: "General Store",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Uncategorized",
                    description: "Misc purchase",
                    counterparty: "Unknown Merchant",
                },
            ],
        },
        "joint" => GroupRuntimePlan {
            primary_asset_account_id: "Assets:Joint:Current".to_owned(),
            income_account_id: "Income:Salary".to_owned(),
            return_income_account_id: None,
            return_asset_account_id: None,
            opening_balances: vec![
                ("Assets:Joint:Current", 920_000),
                ("Assets:Joint:HomeReserve", 410_000),
            ],
            expense_templates: vec![
                ExpenseTemplate {
                    expense_account_id: "Expenses:Housing:Rent",
                    description: "Monthly rent",
                    counterparty: "Landlord",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Bills:Energy",
                    description: "Energy bill",
                    counterparty: "Energy Provider",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Bills:Water",
                    description: "Water bill",
                    counterparty: "Water Utility",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Bills:Internet",
                    description: "Internet bill",
                    counterparty: "Broadband Co",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Food:Groceries",
                    description: "Household grocery",
                    counterparty: "Market Hall",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Bills:CouncilTax",
                    description: "Council tax",
                    counterparty: "Local Council",
                },
            ],
        },
        "business" => GroupRuntimePlan {
            primary_asset_account_id: "Assets:Business:Operating".to_owned(),
            income_account_id: "Income:Other".to_owned(),
            return_income_account_id: None,
            return_asset_account_id: None,
            opening_balances: vec![
                ("Assets:Business:Operating", 1_200_000),
                ("Assets:Business:TaxReserve", 350_000),
                ("Assets:Business:RainyDay", 500_000),
            ],
            expense_templates: vec![
                ExpenseTemplate {
                    expense_account_id: "Expenses:Business:Software",
                    description: "SaaS tooling",
                    counterparty: "Software Vendor",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Business:Services",
                    description: "Professional services",
                    counterparty: "Specialist Firm",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Business:Contractors",
                    description: "Contractor day rate",
                    counterparty: "Freelancer Collective",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Business:Insurance",
                    description: "Business insurance",
                    counterparty: "Insurer",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Transport:Travel",
                    description: "Client travel",
                    counterparty: "Rail Operator",
                },
                ExpenseTemplate {
                    expense_account_id: "Expenses:Business:Equipment",
                    description: "Studio equipment",
                    counterparty: "Hardware Supplier",
                },
            ],
        },
        _ => {
            return Err(FinError::InvalidInput {
                code: "FIXTURE_GROUP_UNSUPPORTED",
                message: format!("unsupported fixture group: {group_id}"),
            });
        }
    };
    Ok(plan)
}

fn parse_year_month(value: &str) -> Result<(i32, u32)> {
    let mut parts = value.split('-');
    let year = parts
        .next()
        .ok_or_else(|| FinError::InvalidInput {
            code: "FIXTURE_START_MONTH_INVALID",
            message: format!("missing year in start month: {value}"),
        })?
        .parse::<i32>()
        .map_err(|error| FinError::Parse {
            context: "fixture-spec.json",
            message: error.to_string(),
        })?;
    let month = parts
        .next()
        .ok_or_else(|| FinError::InvalidInput {
            code: "FIXTURE_START_MONTH_INVALID",
            message: format!("missing month in start month: {value}"),
        })?
        .parse::<u32>()
        .map_err(|error| FinError::Parse {
            context: "fixture-spec.json",
            message: error.to_string(),
        })?;
    if !(1..=12).contains(&month) {
        return Err(FinError::InvalidInput {
            code: "FIXTURE_START_MONTH_INVALID",
            message: format!("month must be between 1 and 12: {value}"),
        });
    }
    Ok((year, month))
}

fn offset_year_month(year: i32, month: u32, offset: i32) -> (i32, u32) {
    let absolute = year * 12 + i32::try_from(month).unwrap_or(1) - 1 + offset;
    let new_year = absolute.div_euclid(12);
    let new_month = absolute.rem_euclid(12) + 1;
    (new_year, u32::try_from(new_month).unwrap_or(1))
}

fn iso_timestamp(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> String {
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:00")
}

fn short_account_label(account_id: &str) -> String {
    account_id
        .rsplit(':')
        .next()
        .unwrap_or(account_id)
        .to_owned()
}

pub fn canonical_fixture_snapshot(db_path: &Path) -> Result<Vec<String>> {
    let connection = Connection::open(db_path)?;
    let mut rows = Vec::new();

    let mut journals = connection.prepare(
        "SELECT id, posted_at, posted_date, is_transfer, description, COALESCE(counterparty, ''), COALESCE(source_file, '')\n         FROM journal_entries\n         ORDER BY id ASC",
    )?;
    let journal_rows = journals.query_map([], |row| {
        Ok(format!(
            "journal|{}|{}|{}|{}|{}|{}|{}",
            row.get::<usize, String>(0)?,
            row.get::<usize, String>(1)?,
            row.get::<usize, String>(2)?,
            row.get::<usize, i64>(3)?,
            row.get::<usize, String>(4)?,
            row.get::<usize, String>(5)?,
            row.get::<usize, String>(6)?,
        ))
    })?;
    for row in journal_rows {
        rows.push(row?);
    }

    let mut postings = connection.prepare(
        "SELECT id, journal_entry_id, account_id, amount_minor, COALESCE(provider_txn_id, ''), COALESCE(provider_balance_minor, 0)\n         FROM postings\n         ORDER BY id ASC",
    )?;
    let posting_rows = postings.query_map([], |row| {
        Ok(format!(
            "posting|{}|{}|{}|{}|{}|{}",
            row.get::<usize, String>(0)?,
            row.get::<usize, String>(1)?,
            row.get::<usize, String>(2)?,
            row.get::<usize, i64>(3)?,
            row.get::<usize, String>(4)?,
            row.get::<usize, i64>(5)?,
        ))
    })?;
    for row in posting_rows {
        rows.push(row?);
    }

    Ok(rows)
}

pub fn fixture_group_asset_account_ids(home_dir: &Path, group_id: &str) -> Result<Vec<String>> {
    let paths = fixture_paths(home_dir);
    let loaded = load_config(Some(&paths.config_path))?;
    Ok(group_asset_account_ids(&loaded.config, group_id))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        FixtureBuildOptions, canonical_fixture_snapshot, fixture_paths, load_fixture_spec,
        materialize_fixture_home,
    };
    use crate::config::load_config;
    use crate::db::{OpenDatabaseOptions, open_database};
    use crate::queries::view_transactions;
    use crate::reports::{report_cashflow, report_summary};

    #[test]
    fn source_spec_is_valid() {
        let spec = load_fixture_spec(
            &FixtureBuildOptions::default()
                .source_dir
                .join("fixture-spec.json"),
        )
        .expect("fixture spec parses");
        assert_eq!(spec.seed, "qa-001-benchmark-fixture");
        assert_eq!(spec.months, 48);
        assert_eq!(spec.groups.len(), 3);
    }

    #[test]
    fn materialized_fixture_supports_core_reports() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        let loaded = load_config(Some(&fixture.paths.config_path)).expect("load config");
        let connection = open_database(OpenDatabaseOptions {
            path: Some(fixture.paths.db_path.clone()),
            config_dir: Some(fixture.paths.data_dir.clone()),
            create: false,
            readonly: true,
            migrate: true,
        })
        .expect("open db");

        let transactions = view_transactions(
            &connection,
            &crate::queries::TransactionQueryOptions {
                chart_account_ids: Some(crate::queries::group_asset_account_ids(
                    &loaded.config,
                    "personal",
                )),
                limit: 1_000,
                ..crate::queries::TransactionQueryOptions::default()
            },
        )
        .expect("view transactions");
        assert!(transactions.len() > 900);

        let (cashflow, totals) =
            report_cashflow(&connection, &loaded.config, "business", 24, None, None)
                .expect("cashflow");
        assert_eq!(cashflow.len(), 24);
        assert!(totals.income_minor > totals.expense_minor);

        let summary = report_summary(&connection, &loaded.config, 12, None).expect("summary");
        assert_eq!(summary.groups.len(), 3);
        assert!(summary.consolidated.net_worth_minor > 0);
    }

    #[test]
    fn fixture_generation_is_canonical() {
        let left = tempdir().expect("left tempdir");
        let right = tempdir().expect("right tempdir");

        let left_fixture = materialize_fixture_home(left.path(), &FixtureBuildOptions::default())
            .expect("left fixture");
        let right_fixture = materialize_fixture_home(right.path(), &FixtureBuildOptions::default())
            .expect("right fixture");

        assert_eq!(left_fixture.stats, right_fixture.stats);
        assert_eq!(
            canonical_fixture_snapshot(&left_fixture.paths.db_path).expect("left snapshot"),
            canonical_fixture_snapshot(&right_fixture.paths.db_path).expect("right snapshot"),
        );
    }

    #[test]
    fn fixture_paths_follow_fin_home_shape() {
        let root = tempdir().expect("tempdir");
        let paths = fixture_paths(root.path());
        assert_eq!(paths.config_path, root.path().join("data/fin.config.toml"));
        assert_eq!(paths.rules_path, root.path().join("data/fin.rules.json"));
        assert_eq!(paths.spec_path, root.path().join("fixture-spec.json"));
        assert_eq!(paths.db_path, root.path().join("data/fin.db"));
    }
}

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use toml::Table;

use crate::error::{FinError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinConfig {
    pub financial: Table,
    pub accounts: Vec<AccountConfig>,
    pub banks: Vec<BankPreset>,
    #[serde(default)]
    pub sanitization: Option<SanitizationConfig>,
    #[serde(default)]
    pub groups: Option<Vec<GroupMetadata>>,
    #[serde(default, alias = "reserve")]
    pub reserves: Option<ReserveConfig>,
}

impl FinConfig {
    #[must_use]
    pub fn rules_path(&self) -> Option<&str> {
        self.sanitization.as_ref()?.rules.as_deref()
    }

    pub fn validate(&self) -> Result<()> {
        if self.accounts.is_empty() {
            return Err(FinError::InvalidInput {
                code: "CONFIG_EMPTY_ACCOUNTS",
                message: "accounts must not be empty".to_owned(),
            });
        }
        if self.banks.is_empty() {
            return Err(FinError::InvalidInput {
                code: "CONFIG_EMPTY_BANKS",
                message: "banks must not be empty".to_owned(),
            });
        }
        self.validate_reserve_config()?;
        Ok(())
    }

    #[must_use]
    pub fn group_ids(&self) -> Vec<String> {
        let mut groups = Vec::new();
        let mut seen = BTreeSet::new();
        if let Some(configured_groups) = &self.groups {
            for group in configured_groups {
                if seen.insert(group.id.clone()) {
                    groups.push(group.id.clone());
                }
            }
        }
        for account in &self.accounts {
            if seen.insert(account.group.clone()) {
                groups.push(account.group.clone());
            }
        }
        groups
    }

    #[must_use]
    pub fn account_ids_by_group(&self, group_id: &str) -> Vec<String> {
        self.accounts
            .iter()
            .filter(|account| account.group == group_id)
            .map(|account| account.id.clone())
            .collect()
    }

    #[must_use]
    pub fn account_by_id(&self, account_id: &str) -> Option<&AccountConfig> {
        self.accounts
            .iter()
            .find(|account| account.id == account_id)
    }

    #[must_use]
    pub fn account_map_by_group(&self) -> BTreeMap<String, Vec<AccountConfig>> {
        let mut groups = BTreeMap::new();
        for account in &self.accounts {
            groups
                .entry(account.group.clone())
                .or_insert_with(Vec::new)
                .push(account.clone());
        }
        groups
    }

    #[must_use]
    pub fn provider_for_account(&self, account_id: &str) -> Option<&str> {
        self.account_by_id(account_id)
            .map(|account| account.provider.as_str())
    }

    #[must_use]
    pub fn bank_preset(&self, provider: &str) -> Option<&BankPreset> {
        self.banks.iter().find(|bank| bank.name == provider)
    }

    #[must_use]
    pub fn resolve_group_metadata(&self, group_id: &str) -> ResolvedGroupMetadata {
        let configured = self
            .groups
            .as_ref()
            .and_then(|groups| groups.iter().find(|group| group.id == group_id));
        if let Some(group) = configured {
            return ResolvedGroupMetadata {
                id: group.id.clone(),
                label: group.label.clone(),
                icon: group.icon.clone().unwrap_or_else(|| "wallet".to_owned()),
                tax_type: group.tax_type.clone().unwrap_or_else(|| "none".to_owned()),
                expense_reserve_months: group.expense_reserve_months.unwrap_or(3),
            };
        }

        let default = match group_id {
            "personal" => ("Personal", "user", "income", 3),
            "joint" => ("Joint", "heart", "none", 3),
            "business" => ("Business", "briefcase", "corp", 1),
            _ => (group_id, "wallet", "none", 3),
        };
        ResolvedGroupMetadata {
            id: group_id.to_owned(),
            label: default.0.to_owned(),
            icon: default.1.to_owned(),
            tax_type: default.2.to_owned(),
            expense_reserve_months: default.3,
        }
    }

    #[must_use]
    pub fn default_reserve_mode(&self) -> ReserveMode {
        self.reserves
            .as_ref()
            .and_then(|reserve| reserve.default_mode)
            .unwrap_or(ReserveMode::Conservative)
    }

    #[must_use]
    pub fn group_default_reserve_mode(&self, group_id: &str) -> ReserveMode {
        self.reserves
            .as_ref()
            .and_then(|reserve| reserve.group_config(group_id))
            .and_then(|group| group.default_mode)
            .unwrap_or_else(|| self.default_reserve_mode())
    }

    #[must_use]
    pub fn legacy_expense_reserve_months(&self, group_id: &str) -> f64 {
        if let Some(months) = self
            .groups
            .as_ref()
            .and_then(|groups| groups.iter().find(|group| group.id == group_id))
            .and_then(|group| group.expense_reserve_months)
        {
            return f64::from(months);
        }

        if let Some(months) = self.financial_i64("expense_reserve_months") {
            return months as f64;
        }

        f64::from(self.resolve_group_metadata(group_id).expense_reserve_months)
    }

    fn global_legacy_expense_reserve_months(&self) -> f64 {
        self.financial_i64("expense_reserve_months")
            .map(|value| value as f64)
            .unwrap_or(3.0)
    }

    #[must_use]
    pub fn resolve_reserve_mode(
        &self,
        group_id: &str,
        requested: Option<ReserveMode>,
    ) -> ReserveMode {
        requested.unwrap_or_else(|| self.group_default_reserve_mode(group_id))
    }

    #[must_use]
    pub fn resolve_reserve_policy(
        &self,
        group_id: &str,
        requested: Option<ReserveMode>,
    ) -> ResolvedReservePolicy {
        let reserve_mode = self.resolve_reserve_mode(group_id, requested);
        let global_mode_config = self
            .reserves
            .as_ref()
            .and_then(|reserve| reserve.mode_config(reserve_mode));
        let group_mode_config = self
            .reserves
            .as_ref()
            .and_then(|reserve| reserve.group_config(group_id))
            .and_then(|group| group.modes.as_ref())
            .and_then(|modes| match reserve_mode {
                ReserveMode::Conservative => modes.conservative.as_ref(),
                ReserveMode::Recurring => modes.recurring.as_ref(),
                ReserveMode::Aggressive => modes.aggressive.as_ref(),
            });
        let fallback_months = self.legacy_expense_reserve_months(group_id);
        let default_months = match reserve_mode {
            ReserveMode::Conservative => fallback_months,
            ReserveMode::Recurring => 6.0,
            ReserveMode::Aggressive => 3.0,
        };
        let expense_months = group_mode_config
            .and_then(|config| config.expense_months)
            .or_else(|| global_mode_config.and_then(|config| config.expense_months))
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(default_months);
        let factor = group_mode_config
            .and_then(|config| config.factor)
            .or_else(|| global_mode_config.and_then(|config| config.factor))
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(1.0);
        let lookback_months = group_mode_config
            .and_then(|config| config.lookback_months)
            .or_else(|| global_mode_config.and_then(|config| config.lookback_months))
            .filter(|value| *value > 0)
            .or(match reserve_mode {
                ReserveMode::Conservative => None,
                ReserveMode::Recurring | ReserveMode::Aggressive => Some(6),
            });
        let expense_basis = group_mode_config
            .and_then(|config| config.expense_basis)
            .or_else(|| global_mode_config.and_then(|config| config.expense_basis))
            .unwrap_or(match reserve_mode {
                ReserveMode::Conservative => ExpenseReserveBasis::HistoricalMedianExpense,
                ReserveMode::Recurring | ReserveMode::Aggressive => {
                    ExpenseReserveBasis::RecurringBaseline
                }
            });

        ResolvedReservePolicy {
            reserve_mode,
            expense_basis,
            expense_months,
            factor,
            lookback_months,
        }
    }

    #[must_use]
    pub fn resolved_reserve_config(&self) -> ResolvedReserveConfig {
        let default_mode = self.default_reserve_mode();
        let modes = ReserveMode::ALL
            .into_iter()
            .map(|mode| {
                let reserve_mode = mode;
                let global_mode_config = self
                    .reserves
                    .as_ref()
                    .and_then(|reserve| reserve.mode_config(reserve_mode));
                let default_months = match reserve_mode {
                    ReserveMode::Conservative => self.global_legacy_expense_reserve_months(),
                    ReserveMode::Recurring => 6.0,
                    ReserveMode::Aggressive => 3.0,
                };
                let policy = ResolvedReservePolicy {
                    reserve_mode,
                    expense_basis: global_mode_config
                        .and_then(|config| config.expense_basis)
                        .unwrap_or(match reserve_mode {
                            ReserveMode::Conservative => {
                                ExpenseReserveBasis::HistoricalMedianExpense
                            }
                            ReserveMode::Recurring | ReserveMode::Aggressive => {
                                ExpenseReserveBasis::RecurringBaseline
                            }
                        }),
                    expense_months: global_mode_config
                        .and_then(|config| config.expense_months)
                        .filter(|value| value.is_finite() && *value >= 0.0)
                        .unwrap_or(default_months),
                    factor: global_mode_config
                        .and_then(|config| config.factor)
                        .filter(|value| value.is_finite() && *value >= 0.0)
                        .unwrap_or(1.0),
                    lookback_months: global_mode_config
                        .and_then(|config| config.lookback_months)
                        .filter(|value| *value > 0)
                        .or(match reserve_mode {
                            ReserveMode::Conservative => None,
                            ReserveMode::Recurring | ReserveMode::Aggressive => Some(6),
                        }),
                };
                (mode.as_str().to_owned(), policy)
            })
            .collect::<BTreeMap<_, _>>();
        let groups = self
            .group_ids()
            .into_iter()
            .map(|group_id| {
                let modes = ReserveMode::ALL
                    .into_iter()
                    .map(|mode| {
                        (
                            mode.as_str().to_owned(),
                            self.resolve_reserve_policy(&group_id, Some(mode)),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                (
                    group_id.clone(),
                    ResolvedReserveGroupConfig {
                        default_mode: self.group_default_reserve_mode(&group_id),
                        modes,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        ResolvedReserveConfig {
            default_mode,
            modes,
            groups,
        }
    }

    fn validate_reserve_config(&self) -> Result<()> {
        let Some(reserves) = &self.reserves else {
            return Ok(());
        };

        if let Some(modes) = &reserves.modes {
            self.validate_reserve_modes("reserves.modes", modes)?;
        }

        for (group_id, group) in &reserves.groups {
            if let Some(modes) = &group.modes {
                self.validate_reserve_modes(&format!("reserves.groups.{group_id}.modes"), modes)?;
            }
        }

        Ok(())
    }

    fn validate_reserve_modes(&self, scope: &str, modes: &ReserveModesConfig) -> Result<()> {
        for mode in ReserveMode::ALL {
            let Some(config) = modes.mode_config(mode) else {
                continue;
            };
            if let Some(expense_months) = config.expense_months
                && (!expense_months.is_finite() || expense_months < 0.0)
            {
                return Err(FinError::InvalidInput {
                    code: "CONFIG_INVALID_RESERVE_MONTHS",
                    message: format!(
                        "{scope}.{} has invalid expense_months: {expense_months}",
                        mode.as_str()
                    ),
                });
            }
            if let Some(factor) = config.factor
                && (!factor.is_finite() || factor < 0.0)
            {
                return Err(FinError::InvalidInput {
                    code: "CONFIG_INVALID_RESERVE_FACTOR",
                    message: format!("{scope}.{} has invalid factor: {factor}", mode.as_str()),
                });
            }
            if let Some(lookback_months) = config.lookback_months
                && lookback_months == 0
            {
                return Err(FinError::InvalidInput {
                    code: "CONFIG_INVALID_RESERVE_LOOKBACK",
                    message: format!("{scope}.{} must use lookback_months > 0", mode.as_str()),
                });
            }
            if let Some(expense_basis) = config.expense_basis {
                let expected = match mode {
                    ReserveMode::Conservative => ExpenseReserveBasis::HistoricalMedianExpense,
                    ReserveMode::Recurring | ReserveMode::Aggressive => {
                        ExpenseReserveBasis::RecurringBaseline
                    }
                };
                if expense_basis != expected {
                    return Err(FinError::InvalidInput {
                        code: "CONFIG_INVALID_RESERVE_BASIS",
                        message: format!(
                            "{scope}.{} requires expense_basis `{}`",
                            mode.as_str(),
                            expected.as_str()
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn financial_i64(&self, key: &str) -> Option<i64> {
        match self.financial.get(key) {
            Some(toml::Value::Integer(value)) => Some(*value),
            Some(toml::Value::Float(value)) => Some(value.round() as i64),
            _ => None,
        }
    }

    #[must_use]
    pub fn financial_f64(&self, key: &str) -> Option<f64> {
        match self.financial.get(key) {
            Some(toml::Value::Float(value)) => Some(*value),
            Some(toml::Value::Integer(value)) => Some(*value as f64),
            _ => None,
        }
    }

    #[must_use]
    pub fn financial_bool(&self, key: &str) -> Option<bool> {
        self.financial.get(key).and_then(toml::Value::as_bool)
    }

    #[must_use]
    pub fn financial_str(&self, key: &str) -> Option<&str> {
        self.financial.get(key).and_then(toml::Value::as_str)
    }

    #[must_use]
    pub fn financial_table(&self, key: &str) -> Option<&Table> {
        self.financial.get(key).and_then(toml::Value::as_table)
    }

    #[must_use]
    pub fn financial_array_strings(&self, key: &str) -> Vec<String> {
        self.financial
            .get(key)
            .and_then(toml::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn joint_share_you(&self) -> f64 {
        self.financial_f64("joint_share_you")
            .unwrap_or(0.5)
            .clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn burn_rate_method(&self) -> String {
        self.financial_str("burn_rate_method")
            .map(|value| value.to_ascii_lowercase())
            .filter(|value| matches!(value.as_str(), "mean" | "median"))
            .unwrap_or_else(|| "median".to_owned())
    }

    #[must_use]
    pub fn burn_rate_exclude_accounts(&self) -> Vec<String> {
        self.financial_array_strings("burn_rate_exclude_accounts")
    }

    #[must_use]
    pub fn runway_balance_exclude_accounts(&self) -> Vec<String> {
        self.financial_array_strings("runway_balance_exclude_accounts")
    }

    #[must_use]
    pub fn tax_year_start_month(&self, tax_type: &str) -> u32 {
        let key = match tax_type {
            "corp" => "corp_tax_year_start_month",
            "income" => "income_tax_year_start_month",
            _ => return 1,
        };
        self.financial_i64(key)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| (1..=12).contains(value))
            .unwrap_or(4)
    }

    #[must_use]
    pub fn scenario_monthly_minor(&self, key: &str) -> Option<i64> {
        let table = self.financial_table("scenario")?;
        match table.get(key) {
            Some(toml::Value::Integer(value)) => Some(*value),
            Some(toml::Value::Float(value)) => Some(value.round() as i64),
            _ => None,
        }
    }

    #[must_use]
    pub fn scenario_bool(&self, key: &str) -> Option<bool> {
        self.financial_table("scenario")
            .and_then(|table| table.get(key))
            .and_then(toml::Value::as_bool)
    }

    #[must_use]
    pub fn scenario_toggle(&self, key: &str) -> bool {
        self.financial_table("scenario")
            .and_then(|table| table.get("toggles"))
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get(key))
            .and_then(toml::Value::as_bool)
            .unwrap_or(false)
    }
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ReserveMode {
    #[default]
    Conservative,
    Recurring,
    Aggressive,
}

impl ReserveMode {
    pub const ALL: [Self; 3] = [Self::Conservative, Self::Recurring, Self::Aggressive];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conservative => "conservative",
            Self::Recurring => "recurring",
            Self::Aggressive => "aggressive",
        }
    }
}

impl FromStr for ReserveMode {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "conservative" => Ok(Self::Conservative),
            "recurring" => Ok(Self::Recurring),
            "aggressive" => Ok(Self::Aggressive),
            _ => Err(format!("unsupported reserve mode: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpenseReserveBasis {
    HistoricalMedianExpense,
    RecurringBaseline,
}

impl ExpenseReserveBasis {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HistoricalMedianExpense => "historical_median_expense",
            Self::RecurringBaseline => "recurring_baseline",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReserveConfig {
    #[serde(default)]
    pub default_mode: Option<ReserveMode>,
    #[serde(default)]
    pub modes: Option<ReserveModesConfig>,
    #[serde(default)]
    pub groups: BTreeMap<String, ReserveGroupConfig>,
}

impl ReserveConfig {
    #[must_use]
    pub fn mode_config(&self, mode: ReserveMode) -> Option<&ReserveModeConfig> {
        let modes = self.modes.as_ref()?;
        match mode {
            ReserveMode::Conservative => modes.conservative.as_ref(),
            ReserveMode::Recurring => modes.recurring.as_ref(),
            ReserveMode::Aggressive => modes.aggressive.as_ref(),
        }
    }

    #[must_use]
    pub fn group_config(&self, group_id: &str) -> Option<&ReserveGroupConfig> {
        self.groups.get(group_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReserveModesConfig {
    #[serde(default)]
    pub conservative: Option<ReserveModeConfig>,
    #[serde(default)]
    pub recurring: Option<ReserveModeConfig>,
    #[serde(default)]
    pub aggressive: Option<ReserveModeConfig>,
}

impl ReserveModesConfig {
    #[must_use]
    pub fn mode_config(&self, mode: ReserveMode) -> Option<&ReserveModeConfig> {
        match mode {
            ReserveMode::Conservative => self.conservative.as_ref(),
            ReserveMode::Recurring => self.recurring.as_ref(),
            ReserveMode::Aggressive => self.aggressive.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReserveModeConfig {
    #[serde(default)]
    pub expense_basis: Option<ExpenseReserveBasis>,
    #[serde(default)]
    pub expense_months: Option<f64>,
    #[serde(default, alias = "expense_factor")]
    pub factor: Option<f64>,
    #[serde(default, alias = "burn_lookback_months")]
    pub lookback_months: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReserveGroupConfig {
    #[serde(default)]
    pub default_mode: Option<ReserveMode>,
    #[serde(default)]
    pub modes: Option<ReserveModesConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedReservePolicy {
    pub reserve_mode: ReserveMode,
    pub expense_basis: ExpenseReserveBasis,
    pub expense_months: f64,
    pub factor: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lookback_months: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedReserveGroupConfig {
    pub default_mode: ReserveMode,
    pub modes: BTreeMap<String, ResolvedReservePolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedReserveConfig {
    pub default_mode: ReserveMode,
    pub modes: BTreeMap<String, ResolvedReservePolicy>,
    pub groups: BTreeMap<String, ResolvedReserveGroupConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizationConfig {
    #[serde(default)]
    pub rules: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMetadata {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub tax_type: Option<String>,
    #[serde(default)]
    pub expense_reserve_months: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedGroupMetadata {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub tax_type: String,
    pub expense_reserve_months: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    pub id: String,
    pub group: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub provider: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub inbox_folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankPreset {
    pub name: String,
    pub columns: Table,
}

pub fn parse_fin_config(raw: &str) -> Result<FinConfig> {
    let parsed: FinConfig = toml::from_str(raw).map_err(|error| FinError::Parse {
        context: "fin.config.toml",
        message: error.to_string(),
    })?;
    parsed.validate()?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use crate::config::model::{ReserveMode, parse_fin_config};

    #[test]
    fn parses_config_template_surface() {
        let config = r#"
[financial]
corp_tax_rate = 0.25

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
"#;

        let parsed = parse_fin_config(config).expect("template config parses");
        assert_eq!(parsed.accounts.len(), 1);
        assert_eq!(parsed.banks.len(), 1);
    }

    #[test]
    fn group_ids_preserve_configured_group_order_and_append_missing_account_groups() {
        let config = r#"
[financial]
corp_tax_rate = 0.25

[[groups]]
id = "business"
label = "Business"

[[groups]]
id = "personal"
label = "Personal"

[[accounts]]
id = "Assets:Joint:Monzo"
group = "joint"
type = "asset"
provider = "monzo"

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

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"
"#;

        let parsed = parse_fin_config(config).expect("config parses");

        assert_eq!(parsed.group_ids(), vec!["business", "personal", "joint"]);
    }

    #[test]
    fn reserve_config_parses_group_defaults_and_overrides() {
        let config = r#"
[financial]
corp_tax_rate = 0.25
expense_reserve_months = 9

[reserves]
default_mode = "recurring"

[reserves.modes.recurring]
expense_basis = "recurring_baseline"
expense_months = 6
lookback_months = 6

[reserves.groups.personal]
default_mode = "conservative"

[reserves.groups.personal.modes.aggressive]
expense_months = 2

[[groups]]
id = "business"
label = "Business"
expense_reserve_months = 12

[[groups]]
id = "personal"
label = "Personal"
expense_reserve_months = 6

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

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"
"#;

        let parsed = parse_fin_config(config).expect("config parses");

        assert_eq!(parsed.default_reserve_mode(), ReserveMode::Recurring);
        assert_eq!(
            parsed.group_default_reserve_mode("business"),
            ReserveMode::Recurring
        );
        assert_eq!(
            parsed.group_default_reserve_mode("personal"),
            ReserveMode::Conservative
        );
        assert_eq!(
            parsed
                .resolve_reserve_policy("business", Some(ReserveMode::Conservative))
                .expense_months,
            12.0
        );
        assert_eq!(
            parsed
                .resolve_reserve_policy("personal", Some(ReserveMode::Aggressive))
                .expense_months,
            2.0
        );
        assert_eq!(
            parsed
                .resolved_reserve_config()
                .groups
                .get("personal")
                .expect("personal reserves")
                .default_mode,
            ReserveMode::Conservative
        );
    }
}

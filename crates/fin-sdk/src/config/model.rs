use std::collections::{BTreeMap, BTreeSet};

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
    use crate::config::model::parse_fin_config;

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
}

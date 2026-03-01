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
}

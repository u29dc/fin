use regex::RegexBuilder;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::categories::map_to_expense_account;
use crate::error::{FinError, Result};
use crate::rules::{MatchMode, NameMappingConfig, NameMappingRule};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizeMatch {
    pub clean_description: String,
    pub category: Option<String>,
    pub matched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptionSummary {
    pub raw_description: String,
    pub occurrences: i64,
    pub total_amount_minor: i64,
    pub chart_account_ids: Vec<String>,
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationCandidate {
    pub id: String,
    pub raw_description: String,
    pub current_clean: String,
    pub proposed_clean: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPlan {
    pub to_update: Vec<MigrationCandidate>,
    pub already_clean: usize,
    pub no_match: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationError {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<MigrationError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecategorizeCandidate {
    pub posting_id: String,
    pub journal_entry_id: String,
    pub description: String,
    pub current_account_id: String,
    pub proposed_account_id: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecategorizePlan {
    pub to_update: Vec<RecategorizeCandidate>,
    pub already_categorized: usize,
    pub no_match: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecategorizeResult {
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<MigrationError>,
}

fn matches_rule(value: &str, rule: &NameMappingRule) -> bool {
    let normalized = value.trim();
    let upper = normalized.to_ascii_uppercase();
    rule.patterns.iter().any(|pattern| {
        let mode = rule.match_mode;
        let case_sensitive = rule.case_sensitive;
        match mode {
            MatchMode::Exact => {
                if case_sensitive {
                    normalized == pattern
                } else {
                    upper == pattern.to_ascii_uppercase()
                }
            }
            MatchMode::Regex => RegexBuilder::new(pattern)
                .case_insensitive(!case_sensitive)
                .build()
                .map(|regex| regex.is_match(normalized))
                .unwrap_or(false),
            MatchMode::Contains => {
                if case_sensitive {
                    normalized.contains(pattern)
                } else {
                    upper.contains(&pattern.to_ascii_uppercase())
                }
            }
        }
    })
}

pub fn sanitize_description(raw: &str, config: &NameMappingConfig) -> SanitizeMatch {
    let trimmed = raw.trim();
    for rule in &config.rules {
        if matches_rule(trimmed, rule) {
            return SanitizeMatch {
                clean_description: rule.target.clone(),
                category: rule.category.clone(),
                matched: true,
            };
        }
    }
    SanitizeMatch {
        clean_description: if config.fallback_to_raw {
            trimmed.to_owned()
        } else {
            raw.to_owned()
        },
        category: None,
        matched: false,
    }
}

pub fn discover_descriptions(
    connection: &Connection,
    min_occurrences: usize,
    chart_account_id: Option<&str>,
    limit: usize,
) -> Result<Vec<DescriptionSummary>> {
    let min_occurrences_i64 = i64::try_from(min_occurrences).unwrap_or(i64::MAX);
    let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
    let mut summaries = Vec::new();
    if let Some(account_id) = chart_account_id {
        let mut statement = connection.prepare(
            "SELECT je.raw_description,\n                    COUNT(DISTINCT je.id) AS occurrences,\n                    COALESCE(SUM(p.amount_minor), 0) AS total_amount,\n                    GROUP_CONCAT(DISTINCT p.account_id) AS chart_account_ids,\n                    MIN(je.posted_at) AS first_seen,\n                    MAX(je.posted_at) AS last_seen\n             FROM journal_entries je\n             JOIN postings p ON p.journal_entry_id = je.id\n             WHERE je.raw_description IS NOT NULL\n               AND p.account_id = ?1\n             GROUP BY je.raw_description\n             HAVING occurrences >= ?2\n             ORDER BY occurrences DESC\n             LIMIT ?3",
        )?;
        let mut rows = statement.query(params![account_id, min_occurrences_i64, limit_i64])?;
        while let Some(row) = rows.next()? {
            let chart_ids = row
                .get::<usize, Option<String>>(3)?
                .unwrap_or_default()
                .split(',')
                .filter(|value| !value.is_empty())
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>();
            summaries.push(DescriptionSummary {
                raw_description: row.get(0)?,
                occurrences: row.get(1)?,
                total_amount_minor: row.get(2)?,
                chart_account_ids: chart_ids,
                first_seen: row.get(4)?,
                last_seen: row.get(5)?,
            });
        }
        return Ok(summaries);
    }

    let mut statement = connection.prepare(
        "SELECT je.raw_description,\n                COUNT(DISTINCT je.id) AS occurrences,\n                COALESCE(SUM(p.amount_minor), 0) AS total_amount,\n                GROUP_CONCAT(DISTINCT p.account_id) AS chart_account_ids,\n                MIN(je.posted_at) AS first_seen,\n                MAX(je.posted_at) AS last_seen\n         FROM journal_entries je\n         JOIN postings p ON p.journal_entry_id = je.id\n         WHERE je.raw_description IS NOT NULL\n         GROUP BY je.raw_description\n         HAVING occurrences >= ?1\n         ORDER BY occurrences DESC\n         LIMIT ?2",
    )?;
    let mut rows = statement.query(params![min_occurrences_i64, limit_i64])?;
    while let Some(row) = rows.next()? {
        let chart_ids = row
            .get::<usize, Option<String>>(3)?
            .unwrap_or_default()
            .split(',')
            .filter(|value| !value.is_empty())
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>();
        summaries.push(DescriptionSummary {
            raw_description: row.get(0)?,
            occurrences: row.get(1)?,
            total_amount_minor: row.get(2)?,
            chart_account_ids: chart_ids,
            first_seen: row.get(4)?,
            last_seen: row.get(5)?,
        });
    }
    Ok(summaries)
}

pub fn discover_unmapped_descriptions(
    connection: &Connection,
    config: &NameMappingConfig,
    min_occurrences: usize,
    chart_account_id: Option<&str>,
    limit: usize,
) -> Result<Vec<DescriptionSummary>> {
    Ok(
        discover_descriptions(connection, min_occurrences, chart_account_id, limit)?
            .into_iter()
            .filter(|summary| !sanitize_description(&summary.raw_description, config).matched)
            .collect(),
    )
}

pub fn plan_migration(
    connection: &Connection,
    config: &NameMappingConfig,
) -> Result<MigrationPlan> {
    let mut statement = connection.prepare(
        "SELECT id,\n                raw_description,\n                COALESCE(clean_description, raw_description),\n                counterparty\n         FROM journal_entries\n         WHERE raw_description IS NOT NULL",
    )?;
    let mut rows = statement.query([])?;
    let mut to_update = Vec::new();
    let mut already_clean = 0usize;
    let mut no_match = 0usize;

    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let raw_description: String = row.get(1)?;
        let current_clean: String = row.get(2)?;
        let counterparty: Option<String> = row.get(3)?;
        let mut matched = sanitize_description(&raw_description, config);
        if !matched.matched
            && let Some(counterparty) = counterparty.as_deref()
        {
            let from_counterparty = sanitize_description(counterparty, config);
            if from_counterparty.matched {
                matched = from_counterparty;
            }
        }

        if !matched.matched {
            no_match += 1;
            continue;
        }
        let name_needs_update =
            current_clean != matched.clean_description && current_clean == raw_description;
        if !name_needs_update {
            already_clean += 1;
            continue;
        }
        to_update.push(MigrationCandidate {
            id,
            raw_description,
            current_clean,
            proposed_clean: matched.clean_description,
        });
    }

    Ok(MigrationPlan {
        to_update,
        already_clean,
        no_match,
    })
}

pub fn execute_migration(
    connection: &Connection,
    plan: &MigrationPlan,
    dry_run: bool,
) -> Result<MigrationResult> {
    if dry_run {
        return Ok(MigrationResult {
            updated: plan.to_update.len(),
            skipped: plan.already_clean + plan.no_match,
            errors: vec![],
        });
    }

    let mut statement = connection.prepare(
        "UPDATE journal_entries\n         SET description = ?1,\n             clean_description = ?1,\n             updated_at = datetime('now')\n         WHERE id = ?2",
    )?;
    let mut updated = 0usize;
    let mut errors = Vec::new();
    let tx = connection.unchecked_transaction()?;
    {
        for candidate in &plan.to_update {
            match statement.execute(params![candidate.proposed_clean, candidate.id]) {
                Ok(_) => updated += 1,
                Err(error) => errors.push(MigrationError {
                    id: candidate.id.clone(),
                    error: error.to_string(),
                }),
            }
        }
    }
    tx.commit()?;

    Ok(MigrationResult {
        updated,
        skipped: plan.already_clean + plan.no_match,
        errors,
    })
}

pub fn plan_recategorize(
    connection: &Connection,
    config: &NameMappingConfig,
) -> Result<RecategorizePlan> {
    let mut statement = connection.prepare(
        "SELECT p.id,\n                p.journal_entry_id,\n                je.description,\n                je.raw_description,\n                je.counterparty,\n                p.account_id\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         WHERE p.account_id IN ('Expenses:Uncategorized', 'Expenses:Bills:DirectDebits')",
    )?;
    let mut rows = statement.query([])?;
    let mut to_update = Vec::new();
    let mut already_categorized = 0usize;
    let mut no_match = 0usize;
    while let Some(row) = rows.next()? {
        let posting_id: String = row.get(0)?;
        let journal_entry_id: String = row.get(1)?;
        let description: String = row.get(2)?;
        let raw_description: Option<String> = row.get(3)?;
        let counterparty: Option<String> = row.get(4)?;
        let current_account_id: String = row.get(5)?;
        let mut sanitized =
            sanitize_description(raw_description.as_deref().unwrap_or(&description), config);
        if !sanitized.matched
            && let Some(counterparty) = counterparty.as_deref()
        {
            let from_counterparty = sanitize_description(counterparty, config);
            if from_counterparty.matched {
                sanitized = from_counterparty;
            }
        }
        let proposed_account_id = map_to_expense_account(sanitized.category.as_deref());
        if proposed_account_id == "Expenses:Uncategorized"
            || proposed_account_id == "Expenses:Bills:DirectDebits"
        {
            no_match += 1;
            continue;
        }
        if proposed_account_id == current_account_id {
            already_categorized += 1;
            continue;
        }
        to_update.push(RecategorizeCandidate {
            posting_id,
            journal_entry_id,
            description,
            current_account_id,
            proposed_account_id,
            category: sanitized.category,
        });
    }
    Ok(RecategorizePlan {
        to_update,
        already_categorized,
        no_match,
    })
}

pub fn execute_recategorize(
    connection: &Connection,
    plan: &RecategorizePlan,
    dry_run: bool,
) -> Result<RecategorizeResult> {
    if dry_run {
        return Ok(RecategorizeResult {
            updated: plan.to_update.len(),
            skipped: plan.already_categorized + plan.no_match,
            errors: vec![],
        });
    }

    let mut statement = connection.prepare("UPDATE postings SET account_id = ?1 WHERE id = ?2")?;
    let mut updated = 0usize;
    let mut errors = Vec::new();
    let tx = connection.unchecked_transaction()?;
    {
        for candidate in &plan.to_update {
            match statement.execute(params![candidate.proposed_account_id, candidate.posting_id]) {
                Ok(_) => updated += 1,
                Err(error) => errors.push(MigrationError {
                    id: candidate.posting_id.clone(),
                    error: error.to_string(),
                }),
            }
        }
    }
    tx.commit()?;

    Ok(RecategorizeResult {
        updated,
        skipped: plan.already_categorized + plan.no_match,
        errors,
    })
}

pub fn ensure_account_exists(connection: &Connection, account_id: &str) -> Result<bool> {
    let exists = connection
        .query_row(
            "SELECT id FROM chart_of_accounts WHERE id = ?1",
            [account_id],
            |row| row.get::<usize, String>(0),
        )
        .ok()
        .is_some();
    if exists {
        return Ok(false);
    }

    let parts = account_id.split(':').collect::<Vec<_>>();
    if parts.len() < 2 {
        return Err(FinError::InvalidInput {
            code: "INVALID_ACCOUNT",
            message: format!("Invalid account format: {account_id}"),
        });
    }
    let parent_id = parts[0..parts.len() - 1].join(":");
    let parent_exists = connection
        .query_row(
            "SELECT id FROM chart_of_accounts WHERE id = ?1",
            [&parent_id],
            |row| row.get::<usize, String>(0),
        )
        .ok()
        .is_some();
    if !parent_exists {
        return Err(FinError::InvalidInput {
            code: "INVALID_ACCOUNT",
            message: format!("Parent account not found: {parent_id}"),
        });
    }

    let account_type = match parts[0].to_ascii_lowercase().as_str() {
        "assets" => "asset",
        "liabilities" => "liability",
        "equity" => "equity",
        "income" => "income",
        _ => "expense",
    };
    let name = parts.last().unwrap_or(&account_id).to_string();
    connection.execute(
        "INSERT INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder)\n         VALUES (?1, ?2, ?3, ?4, 0)",
        params![account_id, name, account_type, parent_id],
    )?;
    Ok(true)
}

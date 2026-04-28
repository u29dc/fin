use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use csv::ReaderBuilder;
use regex::Regex;
use rusqlite::{Connection, params, params_from_iter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::categories::map_category_to_account;
use crate::config::{FinConfig, load_config, resolve_fin_paths};
use crate::db::{OpenDatabaseOptions, ensure_chart_of_accounts_seeded, open_database};
use crate::error::{FinError, Result};
use crate::rules::{NameMappingConfig, load_rules};
use crate::sanitize::sanitize_description;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectedProvider {
    Monzo,
    Wise,
    Vanguard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTransaction {
    pub chart_account_id: String,
    pub posted_at: String,
    pub amount_minor: i64,
    pub currency: String,
    pub raw_description: String,
    pub counterparty: Option<String>,
    pub provider_category: Option<String>,
    pub provider_txn_id: Option<String>,
    pub balance_minor: Option<i64>,
    pub source_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalTransaction {
    pub id: String,
    pub chart_account_id: String,
    pub posted_at: String,
    pub amount_minor: i64,
    pub currency: String,
    pub raw_description: String,
    pub clean_description: String,
    pub counterparty: Option<String>,
    pub category: Option<String>,
    pub provider_txn_id: Option<String>,
    pub balance_minor: Option<i64>,
    pub source_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub mode: ImportMode,
    pub processed_files: Vec<String>,
    pub archived_files: Vec<String>,
    pub skipped_files: Vec<SkippedFile>,
    pub total_transactions: usize,
    pub unique_transactions: usize,
    pub duplicate_transactions: usize,
    pub journal_entries_attempted: usize,
    pub journal_entries_created: usize,
    pub transfer_pairs_created: usize,
    pub replaced_provider_transactions: usize,
    pub replaced_journal_entries: usize,
    pub entry_errors: Vec<String>,
    pub accounts_touched: Vec<String>,
    pub unmapped_descriptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportMode {
    #[default]
    Append,
    FullExport,
}

#[derive(Debug, Clone, Default)]
pub struct ImportInboxOptions {
    pub inbox_dir: Option<PathBuf>,
    pub archive_dir: Option<PathBuf>,
    pub db_path: Option<PathBuf>,
    pub migrate: bool,
    pub mode: ImportMode,
}

#[derive(Debug, Clone)]
struct DetectedFile {
    path: PathBuf,
    provider: DetectedProvider,
    chart_account_id: String,
}

#[derive(Debug, Clone)]
struct TransferPair {
    from: CanonicalTransaction,
    to: CanonicalTransaction,
}

#[derive(Debug, Clone, Copy, Default)]
struct FullExportReconciliation {
    replaced_provider_transactions: usize,
    replaced_journal_entries: usize,
}

fn parse_amount_minor(raw: &str) -> Result<i64> {
    let cleaned = raw.trim().replace(',', "");
    if cleaned.is_empty() {
        return Err(FinError::Parse {
            context: "amount",
            message: "empty amount".to_owned(),
        });
    }
    let amount = cleaned.parse::<f64>().map_err(|error| FinError::Parse {
        context: "amount",
        message: error.to_string(),
    })?;
    Ok((amount * 100.0).round() as i64)
}

fn day_month_year_to_iso(value: &str) -> Result<String> {
    let parts = value.trim().split(['/', '-']).collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(FinError::Parse {
            context: "date",
            message: format!("invalid date: {value}"),
        });
    }
    let day = parts[0].parse::<u32>().map_err(|error| FinError::Parse {
        context: "date",
        message: error.to_string(),
    })?;
    let month = parts[1].parse::<u32>().map_err(|error| FinError::Parse {
        context: "date",
        message: error.to_string(),
    })?;
    let year = parts[2].parse::<u32>().map_err(|error| FinError::Parse {
        context: "date",
        message: error.to_string(),
    })?;
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

fn day_month_year_time_to_iso(date_part: &str, time_part: &str) -> Result<String> {
    let date = day_month_year_to_iso(date_part)?;
    let time = time_part.trim().split('.').next().unwrap_or("00:00:00");
    let segments = time.split(':').collect::<Vec<_>>();
    let hour = segments.first().copied().unwrap_or("00");
    let minute = segments.get(1).copied().unwrap_or("00");
    let second = segments.get(2).copied().unwrap_or("00");
    Ok(format!("{date}T{hour:0>2}:{minute:0>2}:{second:0>2}"))
}

fn first_line(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|error| FinError::Io {
        message: format!("{}: {error}", path.display()),
    })?;
    let mut line = String::new();
    for byte in bytes {
        if byte == b'\n' || byte == b'\r' {
            break;
        }
        line.push(byte as char);
    }
    Ok(line)
}

fn detect_provider_from_header(header: &str) -> Option<DetectedProvider> {
    if header.contains("TransferWise ID") {
        return Some(DetectedProvider::Wise);
    }
    if header.contains("Transaction ID") && header.contains("Amount") {
        return Some(DetectedProvider::Monzo);
    }
    if header.contains("Trade Date") && header.contains("Transaction Description") {
        return Some(DetectedProvider::Vanguard);
    }
    None
}

fn provider_name(provider: DetectedProvider) -> &'static str {
    match provider {
        DetectedProvider::Monzo => "monzo",
        DetectedProvider::Wise => "wise",
        DetectedProvider::Vanguard => "vanguard",
    }
}

fn provider_from_string(value: &str) -> Option<DetectedProvider> {
    match value {
        "monzo" => Some(DetectedProvider::Monzo),
        "wise" => Some(DetectedProvider::Wise),
        "vanguard" => Some(DetectedProvider::Vanguard),
        _ => None,
    }
}

fn folder_to_account_map(config: &FinConfig) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for account in &config.accounts {
        if let Some(folder) = &account.inbox_folder {
            map.insert(folder.clone(), account.id.clone());
        }
    }
    map
}

fn scan_inbox(inbox_dir: &Path, config: &FinConfig) -> Result<Vec<DetectedFile>> {
    let mut detected = Vec::new();
    let folder_mapping = folder_to_account_map(config);
    if !inbox_dir.exists() {
        return Err(FinError::InvalidInput {
            code: "NO_INBOX",
            message: format!("Inbox directory not found: {}", inbox_dir.display()),
        });
    }

    for entry in fs::read_dir(inbox_dir).map_err(|error| FinError::Io {
        message: format!("{}: {error}", inbox_dir.display()),
    })? {
        let entry = entry.map_err(|error| FinError::Io {
            message: error.to_string(),
        })?;
        if !entry
            .file_type()
            .map_err(|error| FinError::Io {
                message: error.to_string(),
            })?
            .is_dir()
        {
            continue;
        }
        let folder_name = entry.file_name().to_string_lossy().to_string();
        let Some(chart_account_id) = folder_mapping.get(&folder_name) else {
            continue;
        };
        let expected_provider = config
            .provider_for_account(chart_account_id)
            .and_then(provider_from_string);
        for file in fs::read_dir(entry.path()).map_err(|error| FinError::Io {
            message: error.to_string(),
        })? {
            let file = file.map_err(|error| FinError::Io {
                message: error.to_string(),
            })?;
            if !file
                .file_type()
                .map_err(|error| FinError::Io {
                    message: error.to_string(),
                })?
                .is_file()
            {
                continue;
            }
            let path = file.path();
            let ext = path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            if ext == "pdf" {
                if expected_provider == Some(DetectedProvider::Vanguard) {
                    detected.push(DetectedFile {
                        path,
                        provider: DetectedProvider::Vanguard,
                        chart_account_id: chart_account_id.clone(),
                    });
                }
                continue;
            }
            if ext != "csv" {
                continue;
            }
            let header = first_line(&path)?;
            let inferred_provider = detect_provider_from_header(&header);
            let provider = inferred_provider.or(expected_provider);
            if let Some(provider) = provider
                && Some(provider) == expected_provider
            {
                detected.push(DetectedFile {
                    path,
                    provider,
                    chart_account_id: chart_account_id.clone(),
                });
            }
        }
    }

    Ok(detected)
}

fn get_column(config: &FinConfig, provider: &str, key: &str, fallback: &str) -> String {
    config
        .bank_preset(provider)
        .and_then(|preset| preset.columns.get(key))
        .and_then(toml::Value::as_str)
        .unwrap_or(fallback)
        .to_owned()
}

fn parse_monzo_csv(
    path: &Path,
    chart_account_id: &str,
    config: &FinConfig,
) -> Result<Vec<ParsedTransaction>> {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .from_path(path)
        .map_err(|error| FinError::Parse {
            context: "monzo.csv",
            message: error.to_string(),
        })?;
    let headers = reader
        .headers()
        .map_err(|error| FinError::Parse {
            context: "monzo.csv.headers",
            message: error.to_string(),
        })?
        .clone();
    let date_col = get_column(config, "monzo", "date", "Date");
    let time_col = get_column(config, "monzo", "time", "Time");
    let amount_col = get_column(config, "monzo", "amount", "Amount");
    let description_col = get_column(config, "monzo", "description", "Description");
    let name_col = get_column(config, "monzo", "name", "Name");
    let category_col = get_column(config, "monzo", "category", "Category");
    let txn_id_col = get_column(config, "monzo", "transaction_id", "Transaction ID");
    let balance_col = get_column(config, "monzo", "balance", "Balance");
    let currency_col = "Currency".to_owned();
    let mut records = Vec::new();

    for row in reader.records() {
        let row = row.map_err(|error| FinError::Parse {
            context: "monzo.csv.row",
            message: error.to_string(),
        })?;
        let value = |column: &str| {
            headers
                .iter()
                .position(|header| header == column)
                .and_then(|index| row.get(index))
                .unwrap_or("")
                .trim()
        };
        let date = value(&date_col);
        let time = value(&time_col);
        if date.is_empty() || time.is_empty() {
            continue;
        }
        let posted_at = day_month_year_time_to_iso(date, time)?;
        let amount = parse_amount_minor(value(&amount_col)).or_else(|_| {
            let money_in = parse_amount_minor(value("Money In")).unwrap_or(0);
            let money_out = parse_amount_minor(value("Money Out")).unwrap_or(0);
            Ok::<i64, FinError>(money_in - money_out)
        })?;
        let name = value(&name_col);
        let description = value(&description_col);
        let raw_description = if !description.is_empty() {
            description.to_owned()
        } else {
            name.to_owned()
        };
        let provider_txn_id = value(&txn_id_col);
        let balance = value(&balance_col);
        let currency = value(&currency_col);
        let category = value(&category_col);
        records.push(ParsedTransaction {
            chart_account_id: chart_account_id.to_owned(),
            posted_at,
            amount_minor: amount,
            currency: if currency.is_empty() {
                "GBP".to_owned()
            } else {
                currency.to_owned()
            },
            raw_description,
            counterparty: if name.is_empty() {
                None
            } else {
                Some(name.to_owned())
            },
            provider_category: if category.is_empty() {
                None
            } else {
                Some(category.to_owned())
            },
            provider_txn_id: if provider_txn_id.is_empty() {
                None
            } else {
                Some(provider_txn_id.to_owned())
            },
            balance_minor: if balance.is_empty() {
                None
            } else {
                Some(parse_amount_minor(balance)?)
            },
            source_file: path.display().to_string(),
        });
    }

    Ok(records)
}

fn parse_wise_csv(
    path: &Path,
    chart_account_id: &str,
    config: &FinConfig,
) -> Result<Vec<ParsedTransaction>> {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .from_path(path)
        .map_err(|error| FinError::Parse {
            context: "wise.csv",
            message: error.to_string(),
        })?;
    let headers = reader
        .headers()
        .map_err(|error| FinError::Parse {
            context: "wise.csv.headers",
            message: error.to_string(),
        })?
        .clone();
    let date_col = get_column(config, "wise", "date", "Date");
    let date_time_col = "Date Time".to_owned();
    let amount_col = get_column(config, "wise", "amount", "Amount");
    let description_col = get_column(config, "wise", "description", "Description");
    let txn_id_col = get_column(config, "wise", "transaction_id", "TransferWise ID");
    let balance_col = get_column(config, "wise", "balance", "Running Balance");
    let mut records = Vec::new();

    for row in reader.records() {
        let row = row.map_err(|error| FinError::Parse {
            context: "wise.csv.row",
            message: error.to_string(),
        })?;
        let value = |column: &str| {
            headers
                .iter()
                .position(|header| header == column)
                .and_then(|index| row.get(index))
                .unwrap_or("")
                .trim()
        };
        let date_time = value(&date_time_col);
        let date_only = value(&date_col);
        let posted_at = if !date_time.is_empty() {
            let parts = date_time.split_whitespace().collect::<Vec<_>>();
            let date = parts.first().copied().unwrap_or("");
            let time = parts.get(1).copied().unwrap_or("00:00:00");
            day_month_year_time_to_iso(date, time)?
        } else {
            let date = day_month_year_to_iso(date_only)?;
            format!("{date}T00:00:00")
        };
        let amount = parse_amount_minor(value(&amount_col))?;
        let description = value(&description_col);
        let payment_reference = value("Payment Reference");
        let raw_description = if payment_reference.is_empty() {
            description.to_owned()
        } else {
            format!("{payment_reference} - {description}")
        };
        let category = value("Transaction Type");
        let counterparty = if !value("Payee Name").is_empty() {
            Some(value("Payee Name").to_owned())
        } else if !value("Payer Name").is_empty() {
            Some(value("Payer Name").to_owned())
        } else {
            None
        };
        let balance = value(&balance_col);
        let txn_id = value(&txn_id_col);
        records.push(ParsedTransaction {
            chart_account_id: chart_account_id.to_owned(),
            posted_at,
            amount_minor: amount,
            currency: {
                let currency = value("Currency");
                if currency.is_empty() {
                    "GBP".to_owned()
                } else {
                    currency.to_owned()
                }
            },
            raw_description,
            counterparty,
            provider_category: if category.is_empty() {
                None
            } else {
                Some(category.to_owned())
            },
            provider_txn_id: if txn_id.is_empty() {
                None
            } else {
                Some(txn_id.to_owned())
            },
            balance_minor: if balance.is_empty() {
                None
            } else {
                Some(parse_amount_minor(balance)?)
            },
            source_file: path.display().to_string(),
        });
    }

    Ok(records)
}

fn parse_vanguard_csv(
    path: &Path,
    chart_account_id: &str,
    config: &FinConfig,
) -> Result<Vec<ParsedTransaction>> {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .from_path(path)
        .map_err(|error| FinError::Parse {
            context: "vanguard.csv",
            message: error.to_string(),
        })?;
    let headers = reader
        .headers()
        .map_err(|error| FinError::Parse {
            context: "vanguard.csv.headers",
            message: error.to_string(),
        })?
        .clone();
    let date_col = get_column(config, "vanguard", "date", "Trade Date");
    let description_col = get_column(config, "vanguard", "description", "Transaction Description");
    let amount_col = get_column(config, "vanguard", "amount", "Net Amount");
    let mut records = Vec::new();

    for (index, row) in reader.records().enumerate() {
        let row = row.map_err(|error| FinError::Parse {
            context: "vanguard.csv.row",
            message: error.to_string(),
        })?;
        let value = |column: &str| {
            headers
                .iter()
                .position(|header| header == column)
                .and_then(|offset| row.get(offset))
                .unwrap_or("")
                .trim()
        };
        let date = value(&date_col);
        let description = value(&description_col);
        let amount_raw = value(&amount_col);
        if date.is_empty() || amount_raw.is_empty() {
            continue;
        }
        let posted_at = format!("{date}T00:00:00");
        let amount_minor = parse_amount_minor(amount_raw)?;
        let lowered = description.to_ascii_lowercase();
        let is_external = lowered.contains("deposit")
            || lowered.starts_with("funds transferred")
            || lowered.contains("withdraw");
        if !is_external {
            continue;
        }
        let provider_txn_id = format!(
            "vanguard-csv-{}-{}-{}-{}",
            index + 1,
            date,
            lowered.replace(' ', "-"),
            amount_raw
        );
        records.push(ParsedTransaction {
            chart_account_id: chart_account_id.to_owned(),
            posted_at,
            amount_minor,
            currency: "GBP".to_owned(),
            raw_description: description.to_owned(),
            counterparty: None,
            provider_category: None,
            provider_txn_id: Some(provider_txn_id),
            balance_minor: None,
            source_file: path.display().to_string(),
        });
    }

    Ok(records)
}

fn parse_english_date_to_iso(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    let parts = trimmed.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(FinError::Parse {
            context: "vanguard.pdf.date",
            message: format!("invalid date: {raw}"),
        });
    }
    let day = parts[0].parse::<u32>().map_err(|error| FinError::Parse {
        context: "vanguard.pdf.date",
        message: error.to_string(),
    })?;
    let month = match parts[1].to_ascii_lowercase().as_str() {
        "january" => 1,
        "february" => 2,
        "march" => 3,
        "april" => 4,
        "may" => 5,
        "june" => 6,
        "july" => 7,
        "august" => 8,
        "september" => 9,
        "october" => 10,
        "november" => 11,
        "december" => 12,
        other => {
            return Err(FinError::Parse {
                context: "vanguard.pdf.date",
                message: format!("invalid month: {other}"),
            });
        }
    };
    let year = parts[2].parse::<u32>().map_err(|error| FinError::Parse {
        context: "vanguard.pdf.date",
        message: error.to_string(),
    })?;
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

fn parse_vanguard_pdf(path: &Path, chart_account_id: &str) -> Result<Vec<ParsedTransaction>> {
    let output = Command::new("pdftotext")
        .arg(path)
        .arg("-")
        .output()
        .map_err(|error| FinError::Parse {
            context: "vanguard.pdf.exec",
            message: format!("failed to run pdftotext: {error}"),
        })?;
    if !output.status.success() {
        return Err(FinError::Parse {
            context: "vanguard.pdf.exec",
            message: format!(
                "pdftotext exited with {}: {}",
                output.status.code().unwrap_or(1),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();

    let date_re =
        Regex::new(r"Portfolio Value by Product Wrapper as at ([0-9]{1,2} [A-Za-z]+ [0-9]{4})")
            .map_err(|error| FinError::Parse {
                context: "vanguard.pdf.regex",
                message: error.to_string(),
            })?;
    let fallback_date_re =
        Regex::new(r"\n([0-9]{1,2} [A-Za-z]+ [0-9]{4})\n").map_err(|error| FinError::Parse {
            context: "vanguard.pdf.regex",
            message: error.to_string(),
        })?;

    let date_capture = date_re
        .captures(&text)
        .or_else(|| fallback_date_re.captures(&text))
        .ok_or_else(|| FinError::Parse {
            context: "vanguard.pdf.date",
            message: "valuation date not found".to_owned(),
        })?;
    let valuation_date = parse_english_date_to_iso(
        date_capture
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or(""),
    )?;

    let anchor = "Total Portfolio Value";
    let anchor_index = text.find(anchor).ok_or_else(|| FinError::Parse {
        context: "vanguard.pdf.value",
        message: "Total Portfolio Value section not found".to_owned(),
    })?;
    let window_end = usize::min(anchor_index + 1_000, text.len());
    let window = &text[anchor_index..window_end];
    let value_re = Regex::new(r"£\s*([0-9,]+\.[0-9]{2})").map_err(|error| FinError::Parse {
        context: "vanguard.pdf.regex",
        message: error.to_string(),
    })?;
    let value_raw = value_re
        .captures(window)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str())
        .ok_or_else(|| FinError::Parse {
            context: "vanguard.pdf.value",
            message: "portfolio value not found near anchor".to_owned(),
        })?;
    let value_minor = parse_amount_minor(&value_raw.replace(',', ""))?;

    Ok(vec![ParsedTransaction {
        chart_account_id: chart_account_id.to_owned(),
        posted_at: format!("{valuation_date}T00:00:00"),
        amount_minor: 0,
        currency: "GBP".to_owned(),
        raw_description: format!("Vanguard portfolio valuation ({valuation_date})"),
        counterparty: None,
        provider_category: Some("portfolio_valuation".to_owned()),
        provider_txn_id: Some(format!("vanguard-valuation-{valuation_date}")),
        balance_minor: Some(value_minor),
        source_file: path.display().to_string(),
    }])
}

fn parse_file(file: &DetectedFile, config: &FinConfig) -> Result<Vec<ParsedTransaction>> {
    match file.provider {
        DetectedProvider::Monzo => parse_monzo_csv(&file.path, &file.chart_account_id, config),
        DetectedProvider::Wise => parse_wise_csv(&file.path, &file.chart_account_id, config),
        DetectedProvider::Vanguard => {
            let extension = file
                .path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if extension == "csv" {
                parse_vanguard_csv(&file.path, &file.chart_account_id, config)
            } else if extension == "pdf" {
                parse_vanguard_pdf(&file.path, &file.chart_account_id)
            } else {
                Err(FinError::InvalidInput {
                    code: "UNSUPPORTED_FILE",
                    message: format!("Unsupported Vanguard file: {}", file.path.display()),
                })
            }
        }
    }
}

fn canonicalize_transactions(
    transactions: Vec<ParsedTransaction>,
    rules: &NameMappingConfig,
) -> (Vec<CanonicalTransaction>, Vec<String>) {
    let mut canonical = Vec::with_capacity(transactions.len());
    let mut unmapped = BTreeSet::new();
    for transaction in transactions {
        let mut sanitized = sanitize_description(&transaction.raw_description, rules);
        if !sanitized.matched
            && let Some(counterparty) = transaction.counterparty.as_deref()
        {
            let from_counterparty = sanitize_description(counterparty, rules);
            if from_counterparty.matched {
                sanitized = from_counterparty;
            }
        }
        if !sanitized.matched && rules.warn_on_unmapped {
            unmapped.insert(transaction.raw_description.clone());
        }
        canonical.push(CanonicalTransaction {
            id: Uuid::new_v4().to_string(),
            chart_account_id: transaction.chart_account_id,
            posted_at: transaction.posted_at,
            amount_minor: transaction.amount_minor,
            currency: transaction.currency,
            raw_description: transaction.raw_description,
            clean_description: sanitized.clean_description,
            counterparty: transaction.counterparty,
            category: sanitized.category,
            provider_txn_id: transaction.provider_txn_id,
            balance_minor: transaction.balance_minor,
            source_file: transaction.source_file,
        });
    }
    (canonical, unmapped.into_iter().collect())
}

fn load_existing_provider_txn_pairs(connection: &Connection) -> Result<HashSet<String>> {
    let mut statement = connection.prepare(
        "SELECT provider_txn_id, account_id\n         FROM postings\n         WHERE provider_txn_id IS NOT NULL",
    )?;
    let mut rows = statement.query([])?;
    let mut set = HashSet::new();
    while let Some(row) = rows.next()? {
        let provider_txn_id: String = row.get(0)?;
        let account_id: String = row.get(1)?;
        set.insert(format!("{provider_txn_id}::{account_id}"));
    }
    Ok(set)
}

fn filter_duplicate_transactions(
    transactions: Vec<CanonicalTransaction>,
    existing_pairs: &HashSet<String>,
) -> Result<(Vec<CanonicalTransaction>, usize)> {
    let mut seen_batch = HashSet::new();
    let mut new_transactions = Vec::new();
    let mut duplicates = 0usize;
    for transaction in transactions {
        if let Some(provider_txn_id) = &transaction.provider_txn_id {
            let key = format!("{provider_txn_id}::{}", transaction.chart_account_id);
            if seen_batch.contains(&key) || existing_pairs.contains(&key) {
                duplicates += 1;
                continue;
            }
            seen_batch.insert(key);
        }
        new_transactions.push(transaction);
    }
    Ok((new_transactions, duplicates))
}

fn filter_new_transactions(
    connection: &Connection,
    transactions: Vec<CanonicalTransaction>,
) -> Result<(Vec<CanonicalTransaction>, usize)> {
    let existing_pairs = load_existing_provider_txn_pairs(connection)?;
    filter_duplicate_transactions(transactions, &existing_pairs)
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

fn provider_journal_entry_ids_for_accounts(
    connection: &Connection,
    account_ids: &[String],
) -> Result<Vec<String>> {
    if account_ids.is_empty() {
        return Ok(Vec::new());
    }
    let sql = format!(
        "SELECT DISTINCT journal_entry_id\n         FROM postings\n         WHERE provider_txn_id IS NOT NULL\n           AND account_id IN ({})",
        placeholders(account_ids.len())
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(account_ids.iter()), |row| row.get(0))?;
    rows.collect::<std::result::Result<Vec<String>, _>>()
        .map_err(Into::into)
}

fn validate_full_export_journal_scope(
    connection: &Connection,
    journal_entry_ids: &[String],
    touched_accounts: &BTreeSet<String>,
) -> Result<()> {
    if journal_entry_ids.is_empty() {
        return Ok(());
    }
    let sql = format!(
        "SELECT DISTINCT journal_entry_id, account_id\n         FROM postings\n         WHERE provider_txn_id IS NOT NULL\n           AND journal_entry_id IN ({})",
        placeholders(journal_entry_ids.len())
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(params_from_iter(journal_entry_ids.iter()))?;
    while let Some(row) = rows.next()? {
        let journal_entry_id: String = row.get(0)?;
        let account_id: String = row.get(1)?;
        if !touched_accounts.contains(&account_id) {
            return Err(FinError::InvalidInput {
                code: "FULL_EXPORT_PARTIAL_TRANSFER",
                message: format!(
                    "Full export reconciliation for touched accounts would delete linked provider transaction in {account_id} from journal entry {journal_entry_id}. Include that account in the same import batch."
                ),
            });
        }
    }
    Ok(())
}

fn provider_transaction_count_for_journal_entries(
    connection: &Connection,
    journal_entry_ids: &[String],
) -> Result<usize> {
    if journal_entry_ids.is_empty() {
        return Ok(0);
    }
    let sql = format!(
        "SELECT COUNT(*)\n         FROM postings\n         WHERE provider_txn_id IS NOT NULL\n           AND journal_entry_id IN ({})",
        placeholders(journal_entry_ids.len())
    );
    let count = connection.query_row(&sql, params_from_iter(journal_entry_ids.iter()), |row| {
        row.get::<_, i64>(0)
    })?;
    Ok(count as usize)
}

fn delete_journal_entries(connection: &Connection, journal_entry_ids: &[String]) -> Result<usize> {
    if journal_entry_ids.is_empty() {
        return Ok(0);
    }
    let sql = format!(
        "DELETE FROM journal_entries WHERE id IN ({})",
        placeholders(journal_entry_ids.len())
    );
    connection
        .execute(&sql, params_from_iter(journal_entry_ids.iter()))
        .map_err(Into::into)
}

fn reconcile_full_export_accounts(
    connection: &Connection,
    accounts_touched: &[String],
) -> Result<FullExportReconciliation> {
    let touched_accounts = accounts_touched.iter().cloned().collect::<BTreeSet<_>>();
    let journal_entry_ids = provider_journal_entry_ids_for_accounts(connection, accounts_touched)?;
    validate_full_export_journal_scope(connection, &journal_entry_ids, &touched_accounts)?;
    let replaced_provider_transactions =
        provider_transaction_count_for_journal_entries(connection, &journal_entry_ids)?;
    let replaced_journal_entries = delete_journal_entries(connection, &journal_entry_ids)?;

    Ok(FullExportReconciliation {
        replaced_provider_transactions,
        replaced_journal_entries,
    })
}

fn transfer_days_delta(a: &str, b: &str) -> Option<i64> {
    let day_a = a.get(0..10)?;
    let day_b = b.get(0..10)?;
    let parse = |value: &str| {
        let parts = value.split('-').collect::<Vec<_>>();
        if parts.len() != 3 {
            return None;
        }
        let year = parts[0].parse::<i64>().ok()?;
        let month = parts[1].parse::<i64>().ok()?;
        let day = parts[2].parse::<i64>().ok()?;
        Some(year * 372 + month * 31 + day)
    };
    Some((parse(day_a)? - parse(day_b)?).abs())
}

fn detect_transfer_pairs(
    transactions: &[CanonicalTransaction],
) -> (Vec<TransferPair>, HashSet<String>) {
    let mut pairs = Vec::new();
    let mut matched_ids = HashSet::new();
    let mut by_amount = HashMap::<i64, Vec<&CanonicalTransaction>>::new();
    for transaction in transactions {
        if transaction.amount_minor.abs() < 100 {
            continue;
        }
        by_amount
            .entry(transaction.amount_minor)
            .or_default()
            .push(transaction);
    }

    for transaction in transactions {
        if matched_ids.contains(&transaction.id) || transaction.amount_minor.abs() < 100 {
            continue;
        }
        let opposite = -transaction.amount_minor;
        let Some(candidates) = by_amount.get(&opposite) else {
            continue;
        };
        let mut found = None;
        for candidate in candidates {
            if matched_ids.contains(&candidate.id) {
                continue;
            }
            if candidate.chart_account_id == transaction.chart_account_id {
                continue;
            }
            if transfer_days_delta(&candidate.posted_at, &transaction.posted_at)
                .map(|days| days <= 5)
                .unwrap_or(false)
            {
                found = Some((*candidate).clone());
                break;
            }
        }
        if let Some(candidate) = found {
            let (from, to) = if transaction.amount_minor < 0 {
                (transaction.clone(), candidate)
            } else {
                (candidate, transaction.clone())
            };
            matched_ids.insert(from.id.clone());
            matched_ids.insert(to.id.clone());
            pairs.push(TransferPair { from, to });
        }
    }

    for transaction in transactions {
        if matched_ids.contains(&transaction.id) || transaction.amount_minor.abs() < 100 {
            continue;
        }
        let is_transfer_category = transaction
            .category
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case("transfer"))
            .unwrap_or(false);
        if !is_transfer_category {
            continue;
        }
        let opposite = -transaction.amount_minor;
        let Some(candidates) = by_amount.get(&opposite) else {
            continue;
        };
        let mut found = None;
        for candidate in candidates {
            if matched_ids.contains(&candidate.id) {
                continue;
            }
            if candidate.chart_account_id == transaction.chart_account_id {
                continue;
            }
            found = Some((*candidate).clone());
            break;
        }
        if let Some(candidate) = found {
            let (from, to) = if transaction.amount_minor < 0 {
                (transaction.clone(), candidate)
            } else {
                (candidate, transaction.clone())
            };
            matched_ids.insert(from.id.clone());
            matched_ids.insert(to.id.clone());
            pairs.push(TransferPair { from, to });
        }
    }

    (pairs, matched_ids)
}

fn insert_transfer_pair(connection: &Connection, pair: &TransferPair) -> Result<()> {
    let journal_id = format!("je_{}", Uuid::new_v4().simple());
    let from_date = pair.from.posted_at.as_str();
    let to_date = pair.to.posted_at.as_str();
    let posted_at = if from_date < to_date {
        from_date
    } else {
        to_date
    };
    let posted_date = &posted_at[0..10];
    let description = if pair.from.clean_description.is_empty() {
        pair.from.raw_description.clone()
    } else {
        pair.from.clean_description.clone()
    };
    connection.execute(
        "INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, raw_description, clean_description, counterparty, source_file)\n         VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8)",
        params![
            journal_id,
            posted_at,
            posted_date,
            description,
            pair.from.raw_description,
            pair.from.clean_description,
            pair.from.counterparty,
            pair.from.source_file,
        ],
    )?;

    connection.execute(
        "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor)\n         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
        params![
            format!("p_{}", Uuid::new_v4().simple()),
            journal_id,
            pair.from.chart_account_id,
            pair.from.amount_minor,
            pair.from.currency,
            pair.from.provider_txn_id,
            pair.from.balance_minor,
        ],
    )?;
    connection.execute(
        "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor)\n         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
        params![
            format!("p_{}", Uuid::new_v4().simple()),
            journal_id,
            pair.to.chart_account_id,
            pair.to.amount_minor,
            pair.to.currency,
            pair.to.provider_txn_id,
            pair.to.balance_minor,
        ],
    )?;
    Ok(())
}

fn insert_non_transfer(connection: &Connection, transaction: &CanonicalTransaction) -> Result<()> {
    let journal_id = format!("je_{}", Uuid::new_v4().simple());
    let posted_date = &transaction.posted_at[0..10];
    let description = if transaction.clean_description.is_empty() {
        transaction.raw_description.clone()
    } else {
        transaction.clean_description.clone()
    };
    let counter_account = map_category_to_account(
        transaction.category.as_deref(),
        &description,
        transaction.amount_minor > 0,
        Some(&transaction.chart_account_id),
    );

    connection.execute(
        "INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, raw_description, clean_description, counterparty, source_file)\n         VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, ?8)",
        params![
            journal_id,
            transaction.posted_at,
            posted_date,
            description,
            transaction.raw_description,
            transaction.clean_description,
            transaction.counterparty,
            transaction.source_file,
        ],
    )?;
    connection.execute(
        "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor)\n         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
        params![
            format!("p_{}", Uuid::new_v4().simple()),
            journal_id,
            transaction.chart_account_id,
            transaction.amount_minor,
            transaction.currency,
            transaction.provider_txn_id,
            transaction.balance_minor,
        ],
    )?;
    connection.execute(
        "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor)\n         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL)",
        params![
            format!("p_{}", Uuid::new_v4().simple()),
            journal_id,
            counter_account,
            -transaction.amount_minor,
            transaction.currency,
        ],
    )?;
    Ok(())
}

fn archive_target_path(
    archive_dir: &Path,
    file_path: &Path,
    provider: DetectedProvider,
    chart_account_id: &str,
    order: usize,
) -> PathBuf {
    let now = chrono_like_now();
    let date_folder = format!("{:04}-{:02}-{:02}", now.year, now.month, now.day);
    let timestamp = format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        now.year, now.month, now.day, now.hour, now.minute, now.second
    );
    let ext = file_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let stem = file_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file")
        .to_ascii_lowercase()
        .replace(|ch: char| !ch.is_ascii_alphanumeric(), "-");
    let account_slug = chart_account_id.replace(':', "-").to_ascii_lowercase();
    let name = format!(
        "{timestamp}_{}_{}_{:02}_{}{ext}",
        provider_name(provider),
        account_slug,
        order,
        stem
    );
    archive_dir.join(date_folder).join(name)
}

struct DateTimeParts {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

fn chrono_like_now() -> DateTimeParts {
    use std::time::{SystemTime, UNIX_EPOCH};
    let unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Coarse conversion sufficient for deterministic archive naming in this tool.
    // 86400 seconds/day approximation is acceptable for folder naming.
    let days = unix / 86_400;
    let year = 1970 + (days / 365) as i32;
    let day_of_year = (days % 365) as u32;
    let month = (day_of_year / 30).saturating_add(1).min(12);
    let day = (day_of_year % 30).saturating_add(1).min(31);
    let secs = unix % 86_400;
    let hour = (secs / 3600) as u32;
    let minute = ((secs % 3600) / 60) as u32;
    let second = (secs % 60) as u32;
    DateTimeParts {
        year,
        month,
        day,
        hour,
        minute,
        second,
    }
}

fn process_files(
    detected: &[DetectedFile],
    config: &FinConfig,
    imported_sources: &HashSet<String>,
    skip_imported_sources: bool,
) -> (
    Vec<DetectedFile>,
    Vec<SkippedFile>,
    Vec<ParsedTransaction>,
    Vec<String>,
) {
    let mut processed = Vec::new();
    let mut skipped = Vec::new();
    let mut parsed = Vec::new();
    let mut accounts_touched = BTreeSet::new();

    for file in detected {
        let file_path = file.path.display().to_string();
        if skip_imported_sources && imported_sources.contains(&file_path) {
            skipped.push(SkippedFile {
                path: file_path,
                reason: "File already imported.".to_owned(),
            });
            continue;
        }
        match parse_file(file, config) {
            Ok(mut transactions) => {
                parsed.append(&mut transactions);
                processed.push(file.clone());
                accounts_touched.insert(file.chart_account_id.clone());
            }
            Err(error) => skipped.push(SkippedFile {
                path: file.path.display().to_string(),
                reason: error.to_string(),
            }),
        }
    }

    (
        processed,
        skipped,
        parsed,
        accounts_touched.into_iter().collect(),
    )
}

fn load_imported_source_files(connection: &Connection) -> Result<HashSet<String>> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT source_file FROM journal_entries WHERE source_file IS NOT NULL",
    )?;
    let mut rows = statement.query([])?;
    let mut set = HashSet::new();
    while let Some(row) = rows.next()? {
        let source_file: String = row.get(0)?;
        set.insert(source_file);
    }
    Ok(set)
}

fn archive_processed_files(files: &[DetectedFile], archive_dir: &Path) -> Result<Vec<String>> {
    let mut archived = Vec::new();
    for (offset, file) in files.iter().enumerate() {
        let target = archive_target_path(
            archive_dir,
            &file.path,
            file.provider,
            &file.chart_account_id,
            offset + 1,
        );
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| FinError::Io {
                message: format!("{}: {error}", parent.display()),
            })?;
        }
        fs::rename(&file.path, &target).map_err(|error| FinError::Io {
            message: format!("{} -> {}: {error}", file.path.display(), target.display()),
        })?;
        archived.push(target.display().to_string());
    }
    Ok(archived)
}

pub fn import_inbox(options: ImportInboxOptions) -> Result<ImportResult> {
    let loaded_config = load_config(None)?;
    let paths = resolve_fin_paths();
    let inbox_dir = options.inbox_dir.unwrap_or(paths.inbox_dir);
    let archive_dir = options.archive_dir.unwrap_or(paths.archive_dir);
    let db_path = options.db_path.unwrap_or(paths.db_file);
    let mut connection = open_database(OpenDatabaseOptions {
        path: Some(db_path),
        config_dir: Some(loaded_config.config_dir()),
        migrate: options.migrate,
        ..OpenDatabaseOptions::default()
    })?;
    ensure_chart_of_accounts_seeded(&connection, &loaded_config.config)?;

    let loaded_rules = load_rules(None, Some(&loaded_config), None)?;
    let detected = scan_inbox(&inbox_dir, &loaded_config.config)?;
    let imported_sources = load_imported_source_files(&connection)?;
    let (processed_files, mut skipped_files, parsed_transactions, accounts_touched) = process_files(
        &detected,
        &loaded_config.config,
        &imported_sources,
        options.mode == ImportMode::Append,
    );
    let (canonical, unmapped_descriptions) =
        canonicalize_transactions(parsed_transactions, &loaded_rules.config);
    let total_transactions = canonical.len();

    if options.mode == ImportMode::FullExport && !skipped_files.is_empty() {
        let entry_errors = skipped_files
            .iter()
            .map(|file| format!("{}: {}", file.path, file.reason))
            .collect::<Vec<_>>();
        return Ok(ImportResult {
            mode: options.mode,
            processed_files: processed_files
                .iter()
                .map(|file| file.path.display().to_string())
                .collect(),
            archived_files: Vec::new(),
            skipped_files,
            total_transactions,
            unique_transactions: 0,
            duplicate_transactions: 0,
            journal_entries_attempted: 0,
            journal_entries_created: 0,
            transfer_pairs_created: 0,
            replaced_provider_transactions: 0,
            replaced_journal_entries: 0,
            entry_errors,
            accounts_touched,
            unmapped_descriptions,
        });
    }

    let (new_transactions, duplicate_transactions) = if options.mode == ImportMode::FullExport {
        filter_duplicate_transactions(canonical, &HashSet::new())?
    } else {
        filter_new_transactions(&connection, canonical)?
    };
    let unique_transactions = new_transactions.len();
    let (transfer_pairs, transfer_matched) = detect_transfer_pairs(&new_transactions);
    let non_transfers = new_transactions
        .iter()
        .filter(|transaction| !transfer_matched.contains(&transaction.id))
        .cloned()
        .collect::<Vec<_>>();
    let mut journal_entries_created = 0usize;
    let mut transfer_pairs_created = 0usize;
    let mut entry_errors = Vec::new();
    let journal_entries_attempted = transfer_pairs.len() + non_transfers.len();
    let mut reconciliation = FullExportReconciliation::default();

    let tx = connection.transaction()?;
    if options.mode == ImportMode::FullExport {
        reconciliation = reconcile_full_export_accounts(&tx, &accounts_touched)?;
    }
    for pair in &transfer_pairs {
        match insert_transfer_pair(&tx, pair) {
            Ok(_) => {
                journal_entries_created += 1;
                transfer_pairs_created += 1;
            }
            Err(error) => entry_errors.push(format!(
                "Transfer {} <-> {}: {error}",
                pair.from.id, pair.to.id
            )),
        }
    }
    for transaction in &non_transfers {
        match insert_non_transfer(&tx, transaction) {
            Ok(_) => journal_entries_created += 1,
            Err(error) => entry_errors.push(format!("Transaction {}: {error}", transaction.id)),
        }
    }
    if options.mode == ImportMode::FullExport && !entry_errors.is_empty() {
        reconciliation = FullExportReconciliation::default();
    } else {
        tx.commit()?;
    }

    let archived_files = if entry_errors.is_empty() {
        archive_processed_files(&processed_files, &archive_dir)?
    } else {
        skipped_files.extend(processed_files.iter().map(|file| SkippedFile {
            path: file.path.display().to_string(),
            reason: "Not archived because journal entry errors were detected.".to_owned(),
        }));
        Vec::new()
    };

    Ok(ImportResult {
        mode: options.mode,
        processed_files: processed_files
            .iter()
            .map(|file| file.path.display().to_string())
            .collect(),
        archived_files,
        skipped_files,
        total_transactions,
        unique_transactions,
        duplicate_transactions,
        journal_entries_attempted,
        journal_entries_created,
        transfer_pairs_created,
        replaced_provider_transactions: reconciliation.replaced_provider_transactions,
        replaced_journal_entries: reconciliation.replaced_journal_entries,
        entry_errors,
        accounts_touched,
        unmapped_descriptions,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, params};

    use super::*;
    use crate::db::schema::SCHEMA_SQL;

    fn test_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory db");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable foreign keys");
        connection.execute_batch(SCHEMA_SQL).expect("create schema");
        for (id, account_type) in [
            ("Assets:Personal:Monzo", "asset"),
            ("Assets:Personal:Savings", "asset"),
            ("Assets:Business:Wise", "asset"),
            ("Expenses:Other", "expense"),
            ("Income:Other", "income"),
        ] {
            connection
                .execute(
                    "INSERT INTO chart_of_accounts (id, name, account_type) VALUES (?1, ?1, ?2)",
                    params![id, account_type],
                )
                .expect("insert account");
        }
        connection
    }

    fn insert_provider_entry(
        connection: &Connection,
        journal_id: &str,
        account_id: &str,
        provider_txn_id: &str,
        amount_minor: i64,
    ) {
        connection
            .execute(
                "INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, source_file)
                 VALUES (?1, '2026-04-01T00:00:00', '2026-04-01', 0, ?2, 'test.csv')",
                params![journal_id, provider_txn_id],
            )
            .expect("insert journal entry");
        connection
            .execute(
                "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, provider_txn_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    format!("p-{journal_id}-provider"),
                    journal_id,
                    account_id,
                    amount_minor,
                    provider_txn_id
                ],
            )
            .expect("insert provider posting");
        connection
            .execute(
                "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor)
                 VALUES (?1, ?2, 'Expenses:Other', ?3)",
                params![format!("p-{journal_id}-counter"), journal_id, -amount_minor],
            )
            .expect("insert counter posting");
    }

    fn insert_manual_entry(connection: &Connection, journal_id: &str, account_id: &str) {
        connection
            .execute(
                "INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, source_file)
                 VALUES (?1, '2026-04-01T00:00:00', '2026-04-01', 0, 'manual', NULL)",
                params![journal_id],
            )
            .expect("insert manual journal entry");
        connection
            .execute(
                "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor)
                 VALUES (?1, ?2, ?3, 1000)",
                params![format!("p-{journal_id}-asset"), journal_id, account_id],
            )
            .expect("insert manual asset posting");
        connection
            .execute(
                "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor)
                 VALUES (?1, ?2, 'Income:Other', -1000)",
                params![format!("p-{journal_id}-counter"), journal_id],
            )
            .expect("insert manual counter posting");
    }

    fn insert_transfer_entry(connection: &Connection, journal_id: &str) {
        connection
            .execute(
                "INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, source_file)
                 VALUES (?1, '2026-04-01T00:00:00', '2026-04-01', 1, 'transfer', 'test.csv')",
                params![journal_id],
            )
            .expect("insert transfer journal entry");
        for (posting_id, account_id, amount_minor, provider_txn_id) in [
            (
                "p-transfer-from",
                "Assets:Business:Wise",
                -5000,
                "wise-transfer",
            ),
            (
                "p-transfer-to",
                "Assets:Personal:Monzo",
                5000,
                "monzo-transfer",
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, provider_txn_id)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        posting_id,
                        journal_id,
                        account_id,
                        amount_minor,
                        provider_txn_id
                    ],
                )
                .expect("insert transfer posting");
        }
    }

    fn journal_entry_count(connection: &Connection) -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM journal_entries", [], |row| row.get(0))
            .expect("count journal entries")
    }

    fn provider_posting_count(connection: &Connection, account_id: &str) -> i64 {
        connection
            .query_row(
                "SELECT COUNT(*) FROM postings WHERE account_id = ?1 AND provider_txn_id IS NOT NULL",
                params![account_id],
                |row| row.get(0),
            )
            .expect("count provider postings")
    }

    #[test]
    fn full_export_reconciliation_deletes_provider_rows_for_touched_account_only() {
        let connection = test_connection();
        insert_provider_entry(
            &connection,
            "je-provider",
            "Assets:Personal:Monzo",
            "txn-old",
            1500,
        );
        insert_manual_entry(&connection, "je-manual", "Assets:Personal:Monzo");

        let summary =
            reconcile_full_export_accounts(&connection, &["Assets:Personal:Monzo".to_owned()])
                .expect("reconcile");

        assert_eq!(summary.replaced_provider_transactions, 1);
        assert_eq!(summary.replaced_journal_entries, 1);
        assert_eq!(journal_entry_count(&connection), 1);
        assert_eq!(
            provider_posting_count(&connection, "Assets:Personal:Monzo"),
            0
        );
    }

    #[test]
    fn full_export_reconciliation_blocks_partial_linked_transfer() {
        let connection = test_connection();
        insert_transfer_entry(&connection, "je-transfer");

        let error =
            reconcile_full_export_accounts(&connection, &["Assets:Personal:Monzo".to_owned()])
                .expect_err("partial linked transfer should fail");

        assert!(error.to_string().contains("FULL_EXPORT_PARTIAL_TRANSFER"));
        assert_eq!(journal_entry_count(&connection), 1);
        assert_eq!(
            provider_posting_count(&connection, "Assets:Business:Wise"),
            1
        );
        assert_eq!(
            provider_posting_count(&connection, "Assets:Personal:Monzo"),
            1
        );
    }

    #[test]
    fn full_export_reconciliation_deletes_linked_transfer_when_all_accounts_are_touched() {
        let connection = test_connection();
        insert_transfer_entry(&connection, "je-transfer");

        let summary = reconcile_full_export_accounts(
            &connection,
            &[
                "Assets:Business:Wise".to_owned(),
                "Assets:Personal:Monzo".to_owned(),
            ],
        )
        .expect("reconcile linked transfer");

        assert_eq!(summary.replaced_provider_transactions, 2);
        assert_eq!(summary.replaced_journal_entries, 1);
        assert_eq!(journal_entry_count(&connection), 0);
    }

    #[test]
    fn duplicate_filter_can_ignore_existing_rows_for_full_export_reinsert() {
        let transactions = vec![
            CanonicalTransaction {
                id: "a".to_owned(),
                chart_account_id: "Assets:Personal:Monzo".to_owned(),
                posted_at: "2026-04-01T00:00:00".to_owned(),
                amount_minor: 100,
                currency: "GBP".to_owned(),
                raw_description: "one".to_owned(),
                clean_description: "one".to_owned(),
                counterparty: None,
                category: None,
                provider_txn_id: Some("txn-1".to_owned()),
                balance_minor: None,
                source_file: "test.csv".to_owned(),
            },
            CanonicalTransaction {
                id: "b".to_owned(),
                chart_account_id: "Assets:Personal:Monzo".to_owned(),
                posted_at: "2026-04-01T00:00:00".to_owned(),
                amount_minor: 100,
                currency: "GBP".to_owned(),
                raw_description: "one duplicate".to_owned(),
                clean_description: "one duplicate".to_owned(),
                counterparty: None,
                category: None,
                provider_txn_id: Some("txn-1".to_owned()),
                balance_minor: None,
                source_file: "test.csv".to_owned(),
            },
        ];

        let (filtered, duplicates) =
            filter_duplicate_transactions(transactions, &HashSet::new()).expect("filter");

        assert_eq!(filtered.len(), 1);
        assert_eq!(duplicates, 1);
    }
}

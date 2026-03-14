use std::collections::HashSet;

use rusqlite::{Connection, params};

use crate::config::FinConfig;
use crate::error::Result;

#[derive(Debug, Clone)]
struct ChartAccountSeed {
    id: String,
    name: String,
    account_type: String,
    parent_id: Option<String>,
    is_placeholder: bool,
}

fn static_seeds() -> Vec<ChartAccountSeed> {
    [
        ("Liabilities", "Liabilities", "liability", None, true),
        (
            "Liabilities:Business",
            "Business",
            "liability",
            Some("Liabilities"),
            true,
        ),
        (
            "Liabilities:Business:CorpTaxPayable",
            "Corp Tax Payable",
            "liability",
            Some("Liabilities:Business"),
            false,
        ),
        (
            "Liabilities:Business:VATPayable",
            "VAT Payable",
            "liability",
            Some("Liabilities:Business"),
            false,
        ),
        ("Equity", "Equity", "equity", None, true),
        (
            "Equity:OpeningBalances",
            "Opening Balances",
            "equity",
            Some("Equity"),
            false,
        ),
        (
            "Equity:RetainedEarnings",
            "Retained Earnings",
            "equity",
            Some("Equity"),
            false,
        ),
        (
            "Equity:Transfers",
            "Internal Transfers",
            "equity",
            Some("Equity"),
            false,
        ),
        (
            "Equity:Investments",
            "Investments",
            "equity",
            Some("Equity"),
            false,
        ),
        ("Income", "Income", "income", None, true),
        ("Income:Salary", "Salary", "income", Some("Income"), false),
        (
            "Income:Dividends",
            "Dividends",
            "income",
            Some("Income"),
            false,
        ),
        (
            "Income:Interest",
            "Interest",
            "income",
            Some("Income"),
            false,
        ),
        ("Income:Refunds", "Refunds", "income", Some("Income"), false),
        ("Income:Other", "Other", "income", Some("Income"), false),
        ("Expenses", "Expenses", "expense", None, true),
        ("Expenses:Food", "Food", "expense", Some("Expenses"), true),
        (
            "Expenses:Food:Groceries",
            "Groceries",
            "expense",
            Some("Expenses:Food"),
            false,
        ),
        (
            "Expenses:Food:Restaurants",
            "Restaurants",
            "expense",
            Some("Expenses:Food"),
            false,
        ),
        (
            "Expenses:Food:Supplements",
            "Supplements",
            "expense",
            Some("Expenses:Food"),
            false,
        ),
        (
            "Expenses:Transport",
            "Transport",
            "expense",
            Some("Expenses"),
            true,
        ),
        (
            "Expenses:Transport:PublicTransport",
            "Public Transport",
            "expense",
            Some("Expenses:Transport"),
            false,
        ),
        (
            "Expenses:Transport:Travel",
            "Travel",
            "expense",
            Some("Expenses:Transport"),
            false,
        ),
        (
            "Expenses:Transport:Vehicle",
            "Vehicle",
            "expense",
            Some("Expenses:Transport"),
            false,
        ),
        (
            "Expenses:Transport:Parking",
            "Parking",
            "expense",
            Some("Expenses:Transport"),
            false,
        ),
        (
            "Expenses:Housing",
            "Housing",
            "expense",
            Some("Expenses"),
            true,
        ),
        (
            "Expenses:Housing:Rent",
            "Rent",
            "expense",
            Some("Expenses:Housing"),
            false,
        ),
        (
            "Expenses:Housing:Utilities",
            "Utilities",
            "expense",
            Some("Expenses:Housing"),
            false,
        ),
        (
            "Expenses:Business",
            "Business",
            "expense",
            Some("Expenses"),
            true,
        ),
        (
            "Expenses:Business:Software",
            "Software",
            "expense",
            Some("Expenses:Business"),
            false,
        ),
        (
            "Expenses:Business:Equipment",
            "Equipment",
            "expense",
            Some("Expenses:Business"),
            false,
        ),
        (
            "Expenses:Business:Services",
            "Services",
            "expense",
            Some("Expenses:Business"),
            false,
        ),
        (
            "Expenses:Business:Contractors",
            "Contractors",
            "expense",
            Some("Expenses:Business"),
            false,
        ),
        (
            "Expenses:Business:Insurance",
            "Insurance",
            "expense",
            Some("Expenses:Business"),
            false,
        ),
        (
            "Expenses:Entertainment",
            "Entertainment",
            "expense",
            Some("Expenses"),
            true,
        ),
        (
            "Expenses:Entertainment:Subscriptions",
            "Subscriptions",
            "expense",
            Some("Expenses:Entertainment"),
            false,
        ),
        (
            "Expenses:Entertainment:Leisure",
            "Leisure",
            "expense",
            Some("Expenses:Entertainment"),
            false,
        ),
        (
            "Expenses:Health",
            "Health",
            "expense",
            Some("Expenses"),
            true,
        ),
        (
            "Expenses:Health:Medical",
            "Medical",
            "expense",
            Some("Expenses:Health"),
            false,
        ),
        (
            "Expenses:Health:Fitness",
            "Fitness",
            "expense",
            Some("Expenses:Health"),
            false,
        ),
        (
            "Expenses:Health:Insurance",
            "Insurance",
            "expense",
            Some("Expenses:Health"),
            false,
        ),
        (
            "Expenses:Shopping",
            "Shopping",
            "expense",
            Some("Expenses"),
            true,
        ),
        (
            "Expenses:Shopping:Home",
            "Home",
            "expense",
            Some("Expenses:Shopping"),
            false,
        ),
        (
            "Expenses:Shopping:Charity",
            "Charity",
            "expense",
            Some("Expenses:Shopping"),
            false,
        ),
        (
            "Expenses:Personal",
            "Personal",
            "expense",
            Some("Expenses"),
            true,
        ),
        (
            "Expenses:Personal:Immigration",
            "Immigration",
            "expense",
            Some("Expenses:Personal"),
            false,
        ),
        ("Expenses:Taxes", "Taxes", "expense", Some("Expenses"), true),
        (
            "Expenses:Taxes:VAT",
            "VAT",
            "expense",
            Some("Expenses:Taxes"),
            false,
        ),
        (
            "Expenses:Taxes:HMRC",
            "HMRC",
            "expense",
            Some("Expenses:Taxes"),
            false,
        ),
        (
            "Expenses:Taxes:CorporationTax",
            "Corporation Tax",
            "expense",
            Some("Expenses:Taxes"),
            false,
        ),
        (
            "Expenses:Taxes:SelfAssessment",
            "Self Assessment",
            "expense",
            Some("Expenses:Taxes"),
            false,
        ),
        (
            "Expenses:Taxes:PAYE",
            "PAYE",
            "expense",
            Some("Expenses:Taxes"),
            false,
        ),
        ("Expenses:Bills", "Bills", "expense", Some("Expenses"), true),
        (
            "Expenses:Bills:Energy",
            "Energy",
            "expense",
            Some("Expenses:Bills"),
            false,
        ),
        (
            "Expenses:Bills:Water",
            "Water",
            "expense",
            Some("Expenses:Bills"),
            false,
        ),
        (
            "Expenses:Bills:CouncilTax",
            "Council Tax",
            "expense",
            Some("Expenses:Bills"),
            false,
        ),
        (
            "Expenses:Bills:Internet",
            "Internet",
            "expense",
            Some("Expenses:Bills"),
            false,
        ),
        (
            "Expenses:Bills:Insurance",
            "Insurance",
            "expense",
            Some("Expenses:Bills"),
            false,
        ),
        (
            "Expenses:Bills:DirectDebits",
            "Direct Debits",
            "expense",
            Some("Expenses:Bills"),
            false,
        ),
        (
            "Expenses:Uncategorized",
            "Uncategorized",
            "expense",
            Some("Expenses"),
            false,
        ),
        (
            "Expenses:Other",
            "Other",
            "expense",
            Some("Expenses"),
            false,
        ),
    ]
    .into_iter()
    .map(
        |(id, name, account_type, parent_id, is_placeholder)| ChartAccountSeed {
            id: id.to_owned(),
            name: name.to_owned(),
            account_type: account_type.to_owned(),
            parent_id: parent_id.map(std::string::ToString::to_string),
            is_placeholder,
        },
    )
    .collect()
}

fn dynamic_asset_seeds(config: &FinConfig) -> Vec<ChartAccountSeed> {
    let mut seeds = vec![ChartAccountSeed {
        id: "Assets".to_owned(),
        name: "Assets".to_owned(),
        account_type: "asset".to_owned(),
        parent_id: None,
        is_placeholder: true,
    }];
    let mut seen = HashSet::from(["Assets".to_owned()]);

    for account in config
        .accounts
        .iter()
        .filter(|account| account.account_type == "asset")
    {
        let parts = account.id.split(':').collect::<Vec<_>>();
        for index in 1..parts.len() {
            let parent_id = parts[0..=index].join(":");
            if parent_id == account.id || seen.contains(&parent_id) {
                continue;
            }
            let grand_parent = if index == 1 {
                Some("Assets".to_owned())
            } else {
                Some(parts[0..index].join(":"))
            };
            let name = parts[index].to_owned();
            seeds.push(ChartAccountSeed {
                id: parent_id.clone(),
                name,
                account_type: "asset".to_owned(),
                parent_id: grand_parent,
                is_placeholder: true,
            });
            seen.insert(parent_id);
        }

        let leaf_name = account
            .label
            .clone()
            .unwrap_or_else(|| parts.last().unwrap_or(&account.id.as_str()).to_string());
        let parent_id = if parts.len() > 1 {
            Some(parts[0..parts.len() - 1].join(":"))
        } else {
            None
        };
        seeds.push(ChartAccountSeed {
            id: account.id.clone(),
            name: leaf_name,
            account_type: "asset".to_owned(),
            parent_id,
            is_placeholder: false,
        });
    }

    seeds
}

pub fn ensure_chart_of_accounts_seeded(connection: &Connection, config: &FinConfig) -> Result<()> {
    let mut seeds = dynamic_asset_seeds(config);
    seeds.extend(static_seeds());

    let mut statement = connection.prepare(
        "INSERT OR IGNORE INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder)\n         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;

    for seed in seeds {
        statement.execute(params![
            seed.id,
            seed.name,
            seed.account_type,
            seed.parent_id,
            i32::from(seed.is_placeholder),
        ])?;
    }
    Ok(())
}

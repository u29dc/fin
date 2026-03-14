fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn map_exact_category_to_account(category: &str) -> Option<&'static str> {
    match category {
        "transfer" => Some("Equity:Transfers"),
        "salary" => Some("Income:Salary"),
        "dividends" => Some("Income:Dividends"),
        "interest" => Some("Income:Interest"),
        "refund" => Some("Income:Refunds"),
        "food" | "groceries" => Some("Expenses:Food:Groceries"),
        "restaurants" | "cafe" => Some("Expenses:Food:Restaurants"),
        "transport" => Some("Expenses:Transport:PublicTransport"),
        "utilities" => Some("Expenses:Housing:Utilities"),
        "rent" => Some("Expenses:Housing:Rent"),
        "subscriptions" => Some("Expenses:Entertainment:Subscriptions"),
        "businesssubs" => Some("Expenses:Business:Subscriptions"),
        "software" => Some("Expenses:Business:Software"),
        "tax" | "government" => Some("Expenses:Taxes:VAT"),
        "hmrctax" => Some("Expenses:Taxes:HMRC"),
        "corporationtax" => Some("Expenses:Taxes:CorporationTax"),
        "selfassessment" => Some("Expenses:Taxes:SelfAssessment"),
        "paye" => Some("Expenses:Taxes:PAYE"),
        "insurance" => Some("Expenses:Business:Insurance"),
        "office" => Some("Expenses:Business:Equipment"),
        "vehicle" => Some("Expenses:Transport:Vehicle"),
        "professional" | "services" => Some("Expenses:Business:Services"),
        "contractors" => Some("Expenses:Business:Contractors"),
        "immigration" => Some("Expenses:Personal:Immigration"),
        "fitness" => Some("Expenses:Health:Fitness"),
        "healthinsurance" => Some("Expenses:Health:Insurance"),
        "supplements" => Some("Expenses:Food:Supplements"),
        "health" => Some("Expenses:Health:Medical"),
        "shopping" => Some("Expenses:Shopping:Home"),
        "entertainment" => Some("Expenses:Entertainment:Leisure"),
        "travel" => Some("Expenses:Transport:Travel"),
        "charity" => Some("Expenses:Shopping:Charity"),
        "parking" => Some("Expenses:Transport:Parking"),
        "fuel" => Some("Expenses:Transport:Vehicle"),
        "energy" => Some("Expenses:Bills:Energy"),
        "water" => Some("Expenses:Bills:Water"),
        "counciltax" => Some("Expenses:Bills:CouncilTax"),
        "internet" | "broadband" => Some("Expenses:Bills:Internet"),
        "bills" | "directdebit" => Some("Expenses:Bills:DirectDebits"),
        "cardcheck" | "card check" => Some("Equity:Transfers"),
        "investment" => Some("Equity:Investments"),
        "unclear" | "other" => Some("Expenses:Other"),
        _ => None,
    }
}

pub fn map_to_expense_account(category: Option<&str>) -> String {
    let Some(category) = category else {
        return "Expenses:Uncategorized".to_owned();
    };
    let normalized = normalize(category);
    map_exact_category_to_account(&normalized)
        .unwrap_or("Expenses:Uncategorized")
        .to_owned()
}

pub fn map_to_income_account(category: Option<&str>) -> String {
    let Some(category) = category else {
        return "Income:Other".to_owned();
    };
    let normalized = normalize(category);
    let account = map_exact_category_to_account(&normalized).unwrap_or("Income:Other");
    if account.starts_with("Income:") {
        account.to_owned()
    } else {
        "Income:Other".to_owned()
    }
}

fn transfer_keywords() -> [&'static str; 10] {
    [
        "pot",
        "roundup",
        "round-up",
        "savings",
        "vault",
        "flex",
        "topped up",
        "money transfer",
        "internal",
        "transfer",
    ]
}

fn is_internal_transfer(description: &str) -> bool {
    let lowered = normalize(description);
    transfer_keywords()
        .iter()
        .any(|keyword| lowered.contains(keyword))
}

fn is_expense_category(category: &str) -> bool {
    matches!(
        category,
        "groceries"
            | "shopping"
            | "food"
            | "transport"
            | "subscriptions"
            | "businesssubs"
            | "software"
            | "utilities"
            | "health"
            | "personal"
            | "entertainment"
            | "travel"
            | "bills"
            | "directdebit"
            | "energy"
            | "water"
            | "counciltax"
            | "internet"
            | "broadband"
            | "fitness"
            | "healthinsurance"
            | "supplements"
            | "insurance"
            | "vehicle"
            | "tax"
            | "government"
            | "hmrctax"
            | "corporationtax"
            | "selfassessment"
            | "paye"
            | "professional"
            | "contractors"
            | "charity"
            | "immigration"
            | "cafe"
            | "parking"
            | "fuel"
    )
}

pub fn map_category_to_account(
    category: Option<&str>,
    description: &str,
    is_inflow: bool,
    source_account_id: Option<&str>,
) -> String {
    let normalized_description = normalize(description);
    if let Some(category) = category
        && normalize(category) == "transfer"
    {
        return "Equity:Transfers".to_owned();
    }

    if is_internal_transfer(description) {
        return "Equity:Transfers".to_owned();
    }

    if is_inflow {
        if let Some(category) = category {
            let normalized = normalize(category);
            if is_expense_category(normalized.as_str()) {
                return "Income:Refunds".to_owned();
            }
        }
        return map_to_income_account(category);
    }

    if normalized_description.contains("hmrc vat") || normalized_description.contains(" vat ") {
        return "Expenses:Taxes:VAT".to_owned();
    }
    if normalized_description.contains("hmrc")
        || normalized_description.contains("cumbernauld")
        || normalized_description.contains("hm revenue")
    {
        if normalized_description.contains("paye")
            || normalized_description.contains(" nic")
            || normalized_description.contains(" ni ")
        {
            return "Expenses:Taxes:PAYE".to_owned();
        }
        if let Some(source_account_id) = source_account_id {
            if source_account_id.starts_with("Assets:Business:") {
                return "Expenses:Taxes:CorporationTax".to_owned();
            }
            if source_account_id.starts_with("Assets:Personal:")
                || source_account_id.starts_with("Assets:Joint:")
            {
                return "Expenses:Taxes:SelfAssessment".to_owned();
            }
        }
        return "Expenses:Taxes:HMRC".to_owned();
    }

    map_to_expense_account(category)
}

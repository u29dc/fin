use std::collections::BTreeMap;

use rusqlite::{Connection, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::config::FinConfig;
use crate::error::Result;
use crate::queries::group_asset_account_ids;
use crate::stats::round_ratio;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RollupMode {
    Total,
    MonthlyAverage,
}

#[derive(Debug, Clone)]
pub struct HierarchyQueryOptions {
    pub months: usize,
    pub mode: RollupMode,
    pub to: Option<String>,
}

impl Default for HierarchyQueryOptions {
    fn default() -> Self {
        Self {
            months: 6,
            mode: RollupMode::MonthlyAverage,
            to: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlowQueryOptions {
    pub months: usize,
    pub mode: RollupMode,
    pub to: Option<String>,
}

impl Default for FlowQueryOptions {
    fn default() -> Self {
        Self {
            months: 6,
            mode: RollupMode::MonthlyAverage,
            to: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpenseHierarchyNodeKind {
    Expense,
    Transfer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExpenseHierarchyNode {
    pub account_id: String,
    pub name: String,
    pub kind: ExpenseHierarchyNodeKind,
    pub total_minor: i64,
    pub share_of_parent_pct: f64,
    pub share_of_root_pct: f64,
    pub children: Vec<ExpenseHierarchyNode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FlowNodeKind {
    Income,
    Asset,
    Expense,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub kind: FlowNodeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlowEdge {
    pub source_id: String,
    pub target_id: String,
    pub amount_minor: i64,
    pub share_of_total_pct: f64,
    pub share_of_source_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlowGraph {
    pub total_minor: i64,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
}

#[derive(Debug)]
struct ExpenseAccountRow {
    id: String,
    name: String,
    parent_id: Option<String>,
}

#[derive(Debug)]
struct ExpenseTotalRow {
    account_id: String,
    total_minor: i64,
}

#[derive(Debug)]
struct ExpenseMonthlyTotalRow {
    account_id: String,
    month: String,
    month_total: i64,
}

#[derive(Debug)]
struct TransferOutflowRow {
    label: String,
    total_minor: i64,
}

#[derive(Debug)]
struct FlowRow {
    source_id: String,
    source_label: String,
    source_kind: FlowNodeKind,
    target_id: String,
    target_label: String,
    target_kind: FlowNodeKind,
    amount_minor: i64,
}

#[derive(Debug)]
struct MonthlyFlowRow {
    source_id: String,
    source_label: String,
    source_kind: FlowNodeKind,
    target_id: String,
    target_label: String,
    target_kind: FlowNodeKind,
    month: String,
    month_amount_minor: i64,
}

/// Build a hierarchical outflow tree for one group.
///
/// In `monthly_average` mode each leaf is `(window total / months)` to stabilize infrequent costs.
pub fn group_expense_hierarchy(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    options: &HierarchyQueryOptions,
) -> Result<Vec<ExpenseHierarchyNode>> {
    let account_ids = group_asset_account_ids(config, group_id);
    if account_ids.is_empty() {
        return Ok(Vec::new());
    }

    let totals = match options.mode {
        RollupMode::Total => expense_totals(connection, &account_ids, options)?,
        RollupMode::MonthlyAverage => expense_average_totals(connection, &account_ids, options)?,
    };

    let transfer_root = match options.mode {
        RollupMode::Total => transfer_outflow_root(connection, &account_ids, options)?,
        RollupMode::MonthlyAverage => {
            transfer_outflow_average_root(connection, &account_ids, options)?
        }
    };

    let accounts = expense_accounts(connection)?;
    let mut roots = build_expense_tree(&accounts, &totals);
    if let Some(transfer_root) = transfer_root {
        roots.push(transfer_root);
    }

    prune_zero_nodes(&mut roots);
    sort_tree(&mut roots);
    annotate_tree_shares(&mut roots);
    Ok(roots)
}

/// Build a flow graph for one group.
///
/// In `monthly_average` mode each edge is `(window total / months)` to stabilize infrequent flows.
pub fn group_flow_graph(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    options: &FlowQueryOptions,
) -> Result<FlowGraph> {
    let account_ids = group_asset_account_ids(config, group_id);
    if account_ids.is_empty() {
        return Ok(FlowGraph {
            total_minor: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
        });
    }

    let mut flow_rows = match options.mode {
        RollupMode::Total => total_flow_rows(connection, &account_ids, options)?,
        RollupMode::MonthlyAverage => average_flow_rows(connection, &account_ids, options)?,
    };
    flow_rows.retain(|row| row.amount_minor > 0);
    flow_rows.sort_by(|left, right| {
        right
            .amount_minor
            .cmp(&left.amount_minor)
            .then(left.source_label.cmp(&right.source_label))
            .then(left.target_label.cmp(&right.target_label))
    });

    let total_minor = flow_rows.iter().map(|row| row.amount_minor).sum::<i64>();
    let mut source_totals = BTreeMap::<String, i64>::new();
    let mut nodes = BTreeMap::<String, FlowNode>::new();
    for row in &flow_rows {
        *source_totals.entry(row.source_id.clone()).or_insert(0) += row.amount_minor;
        nodes
            .entry(row.source_id.clone())
            .or_insert_with(|| FlowNode {
                id: row.source_id.clone(),
                label: row.source_label.clone(),
                kind: row.source_kind,
            });
        nodes
            .entry(row.target_id.clone())
            .or_insert_with(|| FlowNode {
                id: row.target_id.clone(),
                label: row.target_label.clone(),
                kind: row.target_kind,
            });
    }

    let edges = flow_rows
        .into_iter()
        .map(|row| {
            let source_total = source_totals.get(&row.source_id).copied().unwrap_or(0);
            FlowEdge {
                source_id: row.source_id,
                target_id: row.target_id,
                amount_minor: row.amount_minor,
                share_of_total_pct: share_pct(row.amount_minor, total_minor),
                share_of_source_pct: share_pct(row.amount_minor, source_total),
            }
        })
        .collect::<Vec<_>>();

    Ok(FlowGraph {
        total_minor,
        nodes: nodes.into_values().collect(),
        edges,
    })
}

fn expense_totals(
    connection: &Connection,
    account_ids: &[String],
    options: &HierarchyQueryOptions,
) -> Result<BTreeMap<String, i64>> {
    let (asset_clause, mut params) = asset_match_clause("asset_posting", account_ids);
    let mut clauses = vec!["coa.account_type = 'expense'".to_owned()];
    apply_window_clause(
        &mut clauses,
        &mut params,
        options.months,
        options.to.as_deref(),
    );

    let sql = format!(
        "SELECT p.account_id,\n                COALESCE(SUM(p.amount_minor), 0) AS total_minor\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON p.account_id = coa.id\n         WHERE {}\n           AND EXISTS (\n                SELECT 1\n                FROM postings asset_posting\n                WHERE asset_posting.journal_entry_id = p.journal_entry_id\n                  AND {}\n           )\n         GROUP BY p.account_id",
        clauses.join(" AND "),
        asset_clause
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(ExpenseTotalRow {
            account_id: row.get(0)?,
            total_minor: row.get(1)?,
        })
    })?;

    let mut totals = BTreeMap::new();
    for row in rows {
        let row = row?;
        totals.insert(row.account_id, row.total_minor);
    }
    Ok(totals)
}

fn expense_average_totals(
    connection: &Connection,
    account_ids: &[String],
    options: &HierarchyQueryOptions,
) -> Result<BTreeMap<String, i64>> {
    let months = normalize_months(options.months);
    let (asset_clause, mut params) = asset_match_clause("asset_posting", account_ids);
    let mut clauses = vec!["coa.account_type = 'expense'".to_owned()];
    apply_window_clause(&mut clauses, &mut params, months, options.to.as_deref());

    let sql = format!(
        "SELECT p.account_id,\n                strftime('%Y-%m', je.posted_date) AS month,\n                COALESCE(SUM(p.amount_minor), 0) AS month_total\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON p.account_id = coa.id\n         WHERE {}\n           AND EXISTS (\n                SELECT 1\n                FROM postings asset_posting\n                WHERE asset_posting.journal_entry_id = p.journal_entry_id\n                  AND {}\n           )\n         GROUP BY p.account_id, strftime('%Y-%m', je.posted_date)",
        clauses.join(" AND "),
        asset_clause
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(ExpenseMonthlyTotalRow {
            account_id: row.get(0)?,
            month: row.get(1)?,
            month_total: row.get(2)?,
        })
    })?;

    let mut totals = BTreeMap::<String, i64>::new();
    let months_i64 = i64::try_from(months).unwrap_or(1);
    for row in rows {
        let row = row?;
        let _month = row.month;
        *totals.entry(row.account_id).or_insert(0) += row.month_total;
    }
    for total in totals.values_mut() {
        *total /= months_i64.max(1);
    }
    Ok(totals)
}

fn transfer_outflow_root(
    connection: &Connection,
    account_ids: &[String],
    options: &HierarchyQueryOptions,
) -> Result<Option<ExpenseHierarchyNode>> {
    let rows = transfer_outflow_rows(connection, account_ids, options)?;
    Ok(build_transfer_root(rows))
}

fn transfer_outflow_average_root(
    connection: &Connection,
    account_ids: &[String],
    options: &HierarchyQueryOptions,
) -> Result<Option<ExpenseHierarchyNode>> {
    let months = normalize_months(options.months);
    let rows = transfer_outflow_average_rows(connection, account_ids, options)?;
    let months_i64 = i64::try_from(months).unwrap_or(1);
    let averaged = rows
        .into_iter()
        .map(|(label, total_minor)| TransferOutflowRow {
            label,
            total_minor: total_minor / months_i64.max(1),
        })
        .collect::<Vec<_>>();
    Ok(build_transfer_root(averaged))
}

fn transfer_outflow_rows(
    connection: &Connection,
    account_ids: &[String],
    options: &HierarchyQueryOptions,
) -> Result<Vec<TransferOutflowRow>> {
    let (from_clause, mut params) = asset_match_clause("p_from", account_ids);
    let mut clauses = vec![
        "coa_from.account_type = 'asset'".to_owned(),
        "coa_to.account_type = 'asset'".to_owned(),
        "p_from.amount_minor < 0".to_owned(),
        "p_to.amount_minor > 0".to_owned(),
        "p_from.account_id != p_to.account_id".to_owned(),
        from_clause,
    ];
    apply_window_clause(
        &mut clauses,
        &mut params,
        options.months,
        options.to.as_deref(),
    );

    let sql = format!(
        "SELECT CASE\n                    WHEN p_to.account_id LIKE 'Assets:%:%' THEN\n                        SUBSTR(p_to.account_id, 8, INSTR(SUBSTR(p_to.account_id, 8), ':') - 1) || ' Account'\n                    ELSE coa_to.name\n                END AS label,\n                COALESCE(SUM(ABS(p_from.amount_minor)), 0) AS total_minor\n         FROM postings p_from\n         JOIN postings p_to ON p_from.journal_entry_id = p_to.journal_entry_id\n         JOIN journal_entries je ON p_from.journal_entry_id = je.id\n         JOIN chart_of_accounts coa_from ON p_from.account_id = coa_from.id\n         JOIN chart_of_accounts coa_to ON p_to.account_id = coa_to.id\n         WHERE {}\n         GROUP BY label",
        clauses.join(" AND ")
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(TransferOutflowRow {
            label: row.get(0)?,
            total_minor: row.get(1)?,
        })
    })?;

    let mut transfers = Vec::new();
    for row in rows {
        transfers.push(row?);
    }
    Ok(transfers)
}

fn transfer_outflow_average_rows(
    connection: &Connection,
    account_ids: &[String],
    options: &HierarchyQueryOptions,
) -> Result<Vec<(String, i64)>> {
    let (from_clause, mut params) = asset_match_clause("p_from", account_ids);
    let mut clauses = vec![
        "coa_from.account_type = 'asset'".to_owned(),
        "coa_to.account_type = 'asset'".to_owned(),
        "p_from.amount_minor < 0".to_owned(),
        "p_to.amount_minor > 0".to_owned(),
        "p_from.account_id != p_to.account_id".to_owned(),
        from_clause,
    ];
    apply_window_clause(
        &mut clauses,
        &mut params,
        normalize_months(options.months),
        options.to.as_deref(),
    );

    let sql = format!(
        "SELECT CASE\n                    WHEN p_to.account_id LIKE 'Assets:%:%' THEN\n                        SUBSTR(p_to.account_id, 8, INSTR(SUBSTR(p_to.account_id, 8), ':') - 1) || ' Account'\n                    ELSE coa_to.name\n                END AS label,\n                strftime('%Y-%m', je.posted_date) AS month,\n                COALESCE(SUM(ABS(p_from.amount_minor)), 0) AS month_total\n         FROM postings p_from\n         JOIN postings p_to ON p_from.journal_entry_id = p_to.journal_entry_id\n         JOIN journal_entries je ON p_from.journal_entry_id = je.id\n         JOIN chart_of_accounts coa_from ON p_from.account_id = coa_from.id\n         JOIN chart_of_accounts coa_to ON p_to.account_id = coa_to.id\n         WHERE {}\n         GROUP BY label, strftime('%Y-%m', je.posted_date)",
        clauses.join(" AND ")
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok((row.get::<usize, String>(0)?, row.get::<usize, i64>(2)?))
    })?;

    let mut aggregated = BTreeMap::<String, i64>::new();
    for row in rows {
        let (label, month_total) = row?;
        *aggregated.entry(label).or_insert(0) += month_total;
    }
    Ok(aggregated.into_iter().collect())
}

fn build_transfer_root(children: Vec<TransferOutflowRow>) -> Option<ExpenseHierarchyNode> {
    let mut transfer_children = children
        .into_iter()
        .filter(|row| row.total_minor > 0)
        .map(|row| ExpenseHierarchyNode {
            account_id: format!("Outflows:Transfers:{}", slugify_label(&row.label)),
            name: row.label,
            kind: ExpenseHierarchyNodeKind::Transfer,
            total_minor: row.total_minor,
            share_of_parent_pct: 0.0,
            share_of_root_pct: 0.0,
            children: Vec::new(),
        })
        .collect::<Vec<_>>();
    if transfer_children.is_empty() {
        return None;
    }
    transfer_children.sort_by(|left, right| {
        right
            .total_minor
            .cmp(&left.total_minor)
            .then(left.name.cmp(&right.name))
    });
    let total_minor = transfer_children.iter().map(|node| node.total_minor).sum();
    Some(ExpenseHierarchyNode {
        account_id: "Outflows:Transfers".to_owned(),
        name: "Transfers".to_owned(),
        kind: ExpenseHierarchyNodeKind::Transfer,
        total_minor,
        share_of_parent_pct: 0.0,
        share_of_root_pct: 0.0,
        children: transfer_children,
    })
}

fn expense_accounts(connection: &Connection) -> Result<Vec<ExpenseAccountRow>> {
    let mut statement = connection.prepare(
        "SELECT id, name, parent_id\n         FROM chart_of_accounts\n         WHERE account_type = 'expense'\n         ORDER BY id ASC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExpenseAccountRow {
            id: row.get(0)?,
            name: row.get(1)?,
            parent_id: row.get(2)?,
        })
    })?;

    let mut accounts = Vec::new();
    for row in rows {
        accounts.push(row?);
    }
    Ok(accounts)
}

fn build_expense_tree(
    accounts: &[ExpenseAccountRow],
    totals: &BTreeMap<String, i64>,
) -> Vec<ExpenseHierarchyNode> {
    let account_rows = accounts
        .iter()
        .map(|account| (account.id.clone(), account))
        .collect::<BTreeMap<_, _>>();
    let mut children_by_parent = BTreeMap::<String, Vec<String>>::new();
    for account in accounts {
        if let Some(parent_id) = &account.parent_id {
            children_by_parent
                .entry(parent_id.clone())
                .or_default()
                .push(account.id.clone());
        }
    }

    let mut roots = Vec::new();
    if let Some(root) =
        build_expense_tree_node("Expenses", &account_rows, totals, &children_by_parent)
    {
        roots.push(root);
    }
    for root in &mut roots {
        roll_up_totals(root);
    }
    roots
}

fn build_expense_tree_node(
    account_id: &str,
    account_rows: &BTreeMap<String, &ExpenseAccountRow>,
    totals: &BTreeMap<String, i64>,
    children_by_parent: &BTreeMap<String, Vec<String>>,
) -> Option<ExpenseHierarchyNode> {
    let account = account_rows.get(account_id)?;
    let mut node = ExpenseHierarchyNode {
        account_id: account.id.clone(),
        name: account.name.clone(),
        kind: ExpenseHierarchyNodeKind::Expense,
        total_minor: totals.get(account_id).copied().unwrap_or(0),
        share_of_parent_pct: 0.0,
        share_of_root_pct: 0.0,
        children: Vec::new(),
    };

    if let Some(child_ids) = children_by_parent.get(account_id) {
        node.children = child_ids
            .iter()
            .filter_map(|child_id| {
                build_expense_tree_node(child_id, account_rows, totals, children_by_parent)
            })
            .collect();
    }

    Some(node)
}

fn roll_up_totals(node: &mut ExpenseHierarchyNode) -> i64 {
    if node.children.is_empty() {
        return node.total_minor;
    }
    let total = node.children.iter_mut().map(roll_up_totals).sum::<i64>();
    node.total_minor = total;
    total
}

fn prune_zero_nodes(nodes: &mut Vec<ExpenseHierarchyNode>) {
    nodes.retain(|node| node.total_minor > 0);
    for node in nodes {
        prune_zero_nodes(&mut node.children);
    }
}

fn sort_tree(nodes: &mut Vec<ExpenseHierarchyNode>) {
    nodes.sort_by(|left, right| {
        right
            .total_minor
            .cmp(&left.total_minor)
            .then(left.name.cmp(&right.name))
    });
    for node in nodes {
        sort_tree(&mut node.children);
    }
}

fn annotate_tree_shares(nodes: &mut [ExpenseHierarchyNode]) {
    let root_total = nodes.iter().map(|node| node.total_minor).sum::<i64>();
    annotate_tree_shares_inner(nodes, root_total, root_total);
}

fn annotate_tree_shares_inner(
    nodes: &mut [ExpenseHierarchyNode],
    parent_total: i64,
    root_total: i64,
) {
    for node in nodes {
        node.share_of_parent_pct = share_pct(node.total_minor, parent_total);
        node.share_of_root_pct = share_pct(node.total_minor, root_total);
        let child_total = node
            .children
            .iter()
            .map(|child| child.total_minor)
            .sum::<i64>();
        let next_parent_total = if child_total > 0 {
            child_total
        } else {
            node.total_minor
        };
        annotate_tree_shares_inner(&mut node.children, next_parent_total, root_total);
    }
}

fn total_flow_rows(
    connection: &Connection,
    account_ids: &[String],
    options: &FlowQueryOptions,
) -> Result<Vec<FlowRow>> {
    let mut rows = income_to_asset_flows(connection, account_ids, options)?;
    rows.extend(asset_to_expense_flows(connection, account_ids, options)?);
    rows.extend(asset_to_asset_flows(connection, account_ids, options)?);
    Ok(rows)
}

fn average_flow_rows(
    connection: &Connection,
    account_ids: &[String],
    options: &FlowQueryOptions,
) -> Result<Vec<FlowRow>> {
    let months = i64::try_from(normalize_months(options.months)).unwrap_or(1);
    let mut totals = BTreeMap::<(String, String), MonthlyFlowRow>::new();
    for row in income_to_asset_monthly_flows(connection, account_ids, options)?
        .into_iter()
        .chain(asset_to_expense_monthly_flows(
            connection,
            account_ids,
            options,
        )?)
        .chain(asset_to_asset_monthly_flows(
            connection,
            account_ids,
            options,
        )?)
    {
        let key = (row.source_id.clone(), row.target_id.clone());
        let entry = totals.entry(key).or_insert(MonthlyFlowRow {
            source_id: row.source_id.clone(),
            source_label: row.source_label.clone(),
            source_kind: row.source_kind,
            target_id: row.target_id.clone(),
            target_label: row.target_label.clone(),
            target_kind: row.target_kind,
            month: String::new(),
            month_amount_minor: 0,
        });
        let _month = row.month;
        entry.month_amount_minor += row.month_amount_minor;
    }

    Ok(totals
        .into_values()
        .filter_map(|row| {
            let amount_minor = row.month_amount_minor / months.max(1);
            (amount_minor > 0).then_some(FlowRow {
                source_id: row.source_id,
                source_label: row.source_label,
                source_kind: row.source_kind,
                target_id: row.target_id,
                target_label: row.target_label,
                target_kind: row.target_kind,
                amount_minor,
            })
        })
        .collect())
}

fn income_to_asset_flows(
    connection: &Connection,
    account_ids: &[String],
    options: &FlowQueryOptions,
) -> Result<Vec<FlowRow>> {
    let (asset_clause, mut params) = asset_match_clause("p_asset", account_ids);
    let mut clauses = vec![
        "coa_income.account_type = 'income'".to_owned(),
        "coa_asset.account_type = 'asset'".to_owned(),
        "p_asset.amount_minor > 0".to_owned(),
        asset_clause,
    ];
    apply_window_clause(
        &mut clauses,
        &mut params,
        options.months,
        options.to.as_deref(),
    );
    query_flow_rows(
        connection,
        format!(
            "SELECT p_income.account_id AS source_id,\n                    coa_income.name AS source_label,\n                    'income' AS source_kind,\n                    p_asset.account_id AS target_id,\n                    coa_asset.name AS target_label,\n                    'asset' AS target_kind,\n                    COALESCE(SUM(p_asset.amount_minor), 0) AS amount_minor\n             FROM postings p_income\n             JOIN postings p_asset ON p_income.journal_entry_id = p_asset.journal_entry_id\n             JOIN journal_entries je ON p_income.journal_entry_id = je.id\n             JOIN chart_of_accounts coa_income ON p_income.account_id = coa_income.id\n             JOIN chart_of_accounts coa_asset ON p_asset.account_id = coa_asset.id\n             WHERE {}\n             GROUP BY p_income.account_id, p_asset.account_id\n             HAVING COALESCE(SUM(p_asset.amount_minor), 0) > 0",
            clauses.join(" AND ")
        ),
        params,
    )
}

fn asset_to_expense_flows(
    connection: &Connection,
    account_ids: &[String],
    options: &FlowQueryOptions,
) -> Result<Vec<FlowRow>> {
    let (asset_clause, mut params) = asset_match_clause("p_asset", account_ids);
    let mut clauses = vec![
        "coa_expense.account_type = 'expense'".to_owned(),
        "coa_asset.account_type = 'asset'".to_owned(),
        "p_expense.amount_minor > 0".to_owned(),
        asset_clause,
    ];
    apply_window_clause(
        &mut clauses,
        &mut params,
        options.months,
        options.to.as_deref(),
    );
    query_flow_rows(
        connection,
        format!(
            "SELECT p_asset.account_id AS source_id,\n                    coa_asset.name AS source_label,\n                    'asset' AS source_kind,\n                    p_expense.account_id AS target_id,\n                    coa_expense.name AS target_label,\n                    'expense' AS target_kind,\n                    COALESCE(SUM(p_expense.amount_minor), 0) AS amount_minor\n             FROM postings p_expense\n             JOIN postings p_asset ON p_expense.journal_entry_id = p_asset.journal_entry_id\n             JOIN journal_entries je ON p_expense.journal_entry_id = je.id\n             JOIN chart_of_accounts coa_expense ON p_expense.account_id = coa_expense.id\n             JOIN chart_of_accounts coa_asset ON p_asset.account_id = coa_asset.id\n             WHERE {}\n             GROUP BY p_asset.account_id, p_expense.account_id\n             HAVING COALESCE(SUM(p_expense.amount_minor), 0) > 0",
            clauses.join(" AND ")
        ),
        params,
    )
}

fn asset_to_asset_flows(
    connection: &Connection,
    account_ids: &[String],
    options: &FlowQueryOptions,
) -> Result<Vec<FlowRow>> {
    let (asset_clause, mut params) = asset_match_clause("p_from", account_ids);
    let mut clauses = vec![
        "coa_from.account_type = 'asset'".to_owned(),
        "coa_to.account_type = 'asset'".to_owned(),
        "p_from.amount_minor < 0".to_owned(),
        "p_to.amount_minor > 0".to_owned(),
        "p_from.account_id != p_to.account_id".to_owned(),
        asset_clause,
    ];
    apply_window_clause(
        &mut clauses,
        &mut params,
        options.months,
        options.to.as_deref(),
    );
    query_flow_rows(
        connection,
        format!(
            "SELECT p_from.account_id AS source_id,\n                    coa_from.name AS source_label,\n                    'asset' AS source_kind,\n                    p_to.account_id AS target_id,\n                    coa_to.name AS target_label,\n                    'asset' AS target_kind,\n                    COALESCE(SUM(p_to.amount_minor), 0) AS amount_minor\n             FROM postings p_from\n             JOIN postings p_to ON p_from.journal_entry_id = p_to.journal_entry_id\n             JOIN journal_entries je ON p_from.journal_entry_id = je.id\n             JOIN chart_of_accounts coa_from ON p_from.account_id = coa_from.id\n             JOIN chart_of_accounts coa_to ON p_to.account_id = coa_to.id\n             WHERE {}\n             GROUP BY p_from.account_id, p_to.account_id\n             HAVING COALESCE(SUM(p_to.amount_minor), 0) > 0",
            clauses.join(" AND ")
        ),
        params,
    )
}

fn income_to_asset_monthly_flows(
    connection: &Connection,
    account_ids: &[String],
    options: &FlowQueryOptions,
) -> Result<Vec<MonthlyFlowRow>> {
    let (asset_clause, mut params) = asset_match_clause("p_asset", account_ids);
    let mut clauses = vec![
        "coa_income.account_type = 'income'".to_owned(),
        "coa_asset.account_type = 'asset'".to_owned(),
        "p_asset.amount_minor > 0".to_owned(),
        asset_clause,
    ];
    apply_window_clause(
        &mut clauses,
        &mut params,
        normalize_months(options.months),
        options.to.as_deref(),
    );
    query_monthly_flow_rows(
        connection,
        format!(
            "SELECT p_income.account_id AS source_id,\n                    coa_income.name AS source_label,\n                    'income' AS source_kind,\n                    p_asset.account_id AS target_id,\n                    coa_asset.name AS target_label,\n                    'asset' AS target_kind,\n                    strftime('%Y-%m', je.posted_date) AS month,\n                    COALESCE(SUM(p_asset.amount_minor), 0) AS amount_minor\n             FROM postings p_income\n             JOIN postings p_asset ON p_income.journal_entry_id = p_asset.journal_entry_id\n             JOIN journal_entries je ON p_income.journal_entry_id = je.id\n             JOIN chart_of_accounts coa_income ON p_income.account_id = coa_income.id\n             JOIN chart_of_accounts coa_asset ON p_asset.account_id = coa_asset.id\n             WHERE {}\n             GROUP BY p_income.account_id, p_asset.account_id, strftime('%Y-%m', je.posted_date)\n             HAVING COALESCE(SUM(p_asset.amount_minor), 0) > 0",
            clauses.join(" AND ")
        ),
        params,
    )
}

fn asset_to_expense_monthly_flows(
    connection: &Connection,
    account_ids: &[String],
    options: &FlowQueryOptions,
) -> Result<Vec<MonthlyFlowRow>> {
    let (asset_clause, mut params) = asset_match_clause("p_asset", account_ids);
    let mut clauses = vec![
        "coa_expense.account_type = 'expense'".to_owned(),
        "coa_asset.account_type = 'asset'".to_owned(),
        "p_expense.amount_minor > 0".to_owned(),
        asset_clause,
    ];
    apply_window_clause(
        &mut clauses,
        &mut params,
        normalize_months(options.months),
        options.to.as_deref(),
    );
    query_monthly_flow_rows(
        connection,
        format!(
            "SELECT p_asset.account_id AS source_id,\n                    coa_asset.name AS source_label,\n                    'asset' AS source_kind,\n                    p_expense.account_id AS target_id,\n                    coa_expense.name AS target_label,\n                    'expense' AS target_kind,\n                    strftime('%Y-%m', je.posted_date) AS month,\n                    COALESCE(SUM(p_expense.amount_minor), 0) AS amount_minor\n             FROM postings p_expense\n             JOIN postings p_asset ON p_expense.journal_entry_id = p_asset.journal_entry_id\n             JOIN journal_entries je ON p_expense.journal_entry_id = je.id\n             JOIN chart_of_accounts coa_expense ON p_expense.account_id = coa_expense.id\n             JOIN chart_of_accounts coa_asset ON p_asset.account_id = coa_asset.id\n             WHERE {}\n             GROUP BY p_asset.account_id, p_expense.account_id, strftime('%Y-%m', je.posted_date)\n             HAVING COALESCE(SUM(p_expense.amount_minor), 0) > 0",
            clauses.join(" AND ")
        ),
        params,
    )
}

fn asset_to_asset_monthly_flows(
    connection: &Connection,
    account_ids: &[String],
    options: &FlowQueryOptions,
) -> Result<Vec<MonthlyFlowRow>> {
    let (asset_clause, mut params) = asset_match_clause("p_from", account_ids);
    let mut clauses = vec![
        "coa_from.account_type = 'asset'".to_owned(),
        "coa_to.account_type = 'asset'".to_owned(),
        "p_from.amount_minor < 0".to_owned(),
        "p_to.amount_minor > 0".to_owned(),
        "p_from.account_id != p_to.account_id".to_owned(),
        asset_clause,
    ];
    apply_window_clause(
        &mut clauses,
        &mut params,
        normalize_months(options.months),
        options.to.as_deref(),
    );
    query_monthly_flow_rows(
        connection,
        format!(
            "SELECT p_from.account_id AS source_id,\n                    coa_from.name AS source_label,\n                    'asset' AS source_kind,\n                    p_to.account_id AS target_id,\n                    coa_to.name AS target_label,\n                    'asset' AS target_kind,\n                    strftime('%Y-%m', je.posted_date) AS month,\n                    COALESCE(SUM(p_to.amount_minor), 0) AS amount_minor\n             FROM postings p_from\n             JOIN postings p_to ON p_from.journal_entry_id = p_to.journal_entry_id\n             JOIN journal_entries je ON p_from.journal_entry_id = je.id\n             JOIN chart_of_accounts coa_from ON p_from.account_id = coa_from.id\n             JOIN chart_of_accounts coa_to ON p_to.account_id = coa_to.id\n             WHERE {}\n             GROUP BY p_from.account_id, p_to.account_id, strftime('%Y-%m', je.posted_date)\n             HAVING COALESCE(SUM(p_to.amount_minor), 0) > 0",
            clauses.join(" AND ")
        ),
        params,
    )
}

fn query_flow_rows(
    connection: &Connection,
    sql: String,
    params: Vec<String>,
) -> Result<Vec<FlowRow>> {
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(FlowRow {
            source_id: row.get(0)?,
            source_label: row.get(1)?,
            source_kind: parse_flow_kind(row.get::<usize, String>(2)?),
            target_id: row.get(3)?,
            target_label: row.get(4)?,
            target_kind: parse_flow_kind(row.get::<usize, String>(5)?),
            amount_minor: row.get(6)?,
        })
    })?;

    let mut flows = Vec::new();
    for row in rows {
        flows.push(row?);
    }
    Ok(flows)
}

fn query_monthly_flow_rows(
    connection: &Connection,
    sql: String,
    params: Vec<String>,
) -> Result<Vec<MonthlyFlowRow>> {
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(MonthlyFlowRow {
            source_id: row.get(0)?,
            source_label: row.get(1)?,
            source_kind: parse_flow_kind(row.get::<usize, String>(2)?),
            target_id: row.get(3)?,
            target_label: row.get(4)?,
            target_kind: parse_flow_kind(row.get::<usize, String>(5)?),
            month: row.get(6)?,
            month_amount_minor: row.get(7)?,
        })
    })?;

    let mut flows = Vec::new();
    for row in rows {
        flows.push(row?);
    }
    Ok(flows)
}

fn parse_flow_kind(raw: String) -> FlowNodeKind {
    match raw.as_str() {
        "income" => FlowNodeKind::Income,
        "expense" => FlowNodeKind::Expense,
        _ => FlowNodeKind::Asset,
    }
}

fn asset_match_clause(alias: &str, account_ids: &[String]) -> (String, Vec<String>) {
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

fn apply_window_clause(
    clauses: &mut Vec<String>,
    params: &mut Vec<String>,
    months: usize,
    to: Option<&str>,
) {
    let months = normalize_months(months);
    if let Some(to) = to {
        clauses.push("je.posted_date <= ?".to_owned());
        params.push(to.to_owned());
        clauses.push("je.posted_date >= date(?, '-' || ? || ' months')".to_owned());
        params.push(to.to_owned());
        params.push(months.to_string());
    } else {
        clauses.push("je.posted_date >= date('now', '-' || ? || ' months')".to_owned());
        params.push(months.to_string());
    }
}

fn normalize_months(months: usize) -> usize {
    months.max(1)
}

fn share_pct(amount_minor: i64, total_minor: i64) -> f64 {
    if amount_minor <= 0 || total_minor <= 0 {
        return 0.0;
    }
    round_ratio((amount_minor as f64 / total_minor as f64) * 100.0)
}

fn slugify_label(label: &str) -> String {
    let slug = label
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() {
                Some(character.to_ascii_lowercase())
            } else if character.is_ascii_whitespace() || matches!(character, '-' | '_') {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>();
    slug.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        ExpenseAccountRow, ExpenseHierarchyNode, ExpenseHierarchyNodeKind, FlowNodeKind,
        FlowQueryOptions, HierarchyQueryOptions, RollupMode, annotate_tree_shares,
        build_expense_tree, group_expense_hierarchy, group_flow_graph,
    };
    use crate::runtime::{RuntimeContext, RuntimeContextOptions};
    use crate::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    #[test]
    fn share_annotation_uses_parent_and_root_totals() {
        let mut nodes = vec![
            ExpenseHierarchyNode {
                account_id: "Expenses".to_owned(),
                name: "Expenses".to_owned(),
                kind: super::ExpenseHierarchyNodeKind::Expense,
                total_minor: 300,
                share_of_parent_pct: 0.0,
                share_of_root_pct: 0.0,
                children: vec![
                    ExpenseHierarchyNode {
                        account_id: "Expenses:Food".to_owned(),
                        name: "Food".to_owned(),
                        kind: super::ExpenseHierarchyNodeKind::Expense,
                        total_minor: 120,
                        share_of_parent_pct: 0.0,
                        share_of_root_pct: 0.0,
                        children: Vec::new(),
                    },
                    ExpenseHierarchyNode {
                        account_id: "Expenses:Transport".to_owned(),
                        name: "Transport".to_owned(),
                        kind: super::ExpenseHierarchyNodeKind::Expense,
                        total_minor: 180,
                        share_of_parent_pct: 0.0,
                        share_of_root_pct: 0.0,
                        children: Vec::new(),
                    },
                ],
            },
            ExpenseHierarchyNode {
                account_id: "Outflows:Transfers".to_owned(),
                name: "Transfers".to_owned(),
                kind: super::ExpenseHierarchyNodeKind::Transfer,
                total_minor: 100,
                share_of_parent_pct: 0.0,
                share_of_root_pct: 0.0,
                children: Vec::new(),
            },
        ];

        annotate_tree_shares(&mut nodes);

        assert_eq!(nodes[0].share_of_root_pct, 75.0);
        assert_eq!(nodes[0].children[0].share_of_parent_pct, 40.0);
        assert_eq!(nodes[0].children[1].share_of_root_pct, 45.0);
        assert_eq!(nodes[1].share_of_root_pct, 25.0);
    }

    #[test]
    fn build_expense_tree_keeps_nested_descendants_attached() {
        let accounts = vec![
            ExpenseAccountRow {
                id: "Expenses".to_owned(),
                name: "Expenses".to_owned(),
                parent_id: None,
            },
            ExpenseAccountRow {
                id: "Expenses:Housing".to_owned(),
                name: "Housing".to_owned(),
                parent_id: Some("Expenses".to_owned()),
            },
            ExpenseAccountRow {
                id: "Expenses:Housing:Rent".to_owned(),
                name: "Rent".to_owned(),
                parent_id: Some("Expenses:Housing".to_owned()),
            },
        ];

        let totals = [
            ("Expenses:Housing:Rent".to_owned(), 120_000_i64),
            ("Expenses:Housing".to_owned(), 0_i64),
            ("Expenses".to_owned(), 0_i64),
        ]
        .into_iter()
        .collect();

        let roots = build_expense_tree(&accounts, &totals);

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].account_id, "Expenses");
        assert_eq!(roots[0].total_minor, 120_000);
        assert_eq!(roots[0].children.len(), 1);
        assert_eq!(roots[0].children[0].account_id, "Expenses:Housing");
        assert_eq!(roots[0].children[0].kind, ExpenseHierarchyNodeKind::Expense);
        assert_eq!(roots[0].children[0].children.len(), 1);
        assert_eq!(
            roots[0].children[0].children[0].account_id,
            "Expenses:Housing:Rent"
        );
    }

    #[test]
    fn fixture_average_hierarchy_includes_transfers_root_and_positive_totals() {
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

        let hierarchy = group_expense_hierarchy(
            runtime.connection(),
            runtime.config(),
            "business",
            &HierarchyQueryOptions {
                months: 6,
                mode: RollupMode::MonthlyAverage,
                to: Some("2026-03-31".to_owned()),
            },
        )
        .expect("expense hierarchy");

        assert!(!hierarchy.is_empty());
        assert!(hierarchy.iter().all(|node| node.total_minor > 0));
        assert!(hierarchy.iter().any(|node| node.name == "Transfers"));
    }

    #[test]
    fn fixture_total_hierarchy_prunes_zero_nodes_and_orders_children() {
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

        let hierarchy = group_expense_hierarchy(
            runtime.connection(),
            runtime.config(),
            "personal",
            &HierarchyQueryOptions {
                months: 3,
                mode: RollupMode::Total,
                to: Some("2026-03-31".to_owned()),
            },
        )
        .expect("expense hierarchy");

        let root = hierarchy.first().expect("root node");
        assert!(root.children.windows(2).all(|window| {
            let [left, right] = window else {
                return true;
            };
            left.total_minor >= right.total_minor
        }));
        assert!(root.children.iter().all(|node| node.total_minor > 0));
    }

    #[test]
    fn fixture_flow_graph_contains_income_asset_and_expense_edges() {
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

        let graph = group_flow_graph(
            runtime.connection(),
            runtime.config(),
            "business",
            &FlowQueryOptions {
                months: 6,
                mode: RollupMode::MonthlyAverage,
                to: Some("2026-03-31".to_owned()),
            },
        )
        .expect("flow graph");

        assert!(graph.total_minor > 0);
        assert!(graph.edges.iter().all(|edge| edge.amount_minor > 0));
        assert!(
            graph
                .nodes
                .iter()
                .any(|node| node.kind == FlowNodeKind::Income)
        );
        assert!(
            graph
                .nodes
                .iter()
                .any(|node| node.kind == FlowNodeKind::Asset)
        );
        assert!(
            graph
                .nodes
                .iter()
                .any(|node| node.kind == FlowNodeKind::Expense)
        );
        let total_share = graph
            .edges
            .iter()
            .map(|edge| edge.share_of_total_pct)
            .sum::<f64>();
        assert!((total_share - 100.0).abs() <= 0.25);
    }

    #[test]
    fn fixture_flow_graph_includes_asset_to_asset_transfers() {
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

        let graph = group_flow_graph(
            runtime.connection(),
            runtime.config(),
            "personal",
            &FlowQueryOptions {
                months: 6,
                mode: RollupMode::MonthlyAverage,
                to: Some("2026-03-31".to_owned()),
            },
        )
        .expect("flow graph");

        assert!(graph.edges.iter().any(|edge| {
            let source = graph
                .nodes
                .iter()
                .find(|node| node.id == edge.source_id)
                .map(|node| node.kind);
            let target = graph
                .nodes
                .iter()
                .find(|node| node.id == edge.target_id)
                .map(|node| node.kind);
            matches!(source, Some(FlowNodeKind::Asset))
                && matches!(target, Some(FlowNodeKind::Asset))
        }));
    }
}

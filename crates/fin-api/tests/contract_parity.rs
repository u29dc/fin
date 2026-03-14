use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use fin_sdk::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};
use serde_json::{Map, Value};
use tempfile::{TempDir, tempdir};

struct FixtureRuntime {
    _temp: TempDir,
    home: PathBuf,
    config_path: PathBuf,
    db_path: PathBuf,
}

struct ApiProcess {
    child: Child,
    address: SocketAddr,
}

impl Drop for ApiProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fixture_runtime() -> FixtureRuntime {
    let temp = tempdir().expect("tempdir");
    let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
        .expect("materialize fixture");
    FixtureRuntime {
        _temp: temp,
        home: fixture.paths.home_dir,
        config_path: fixture.paths.config_path,
        db_path: fixture.paths.db_path,
    }
}

fn reserve_tcp_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve tcp address");
    let address = listener.local_addr().expect("read reserved tcp address");
    drop(listener);
    address
}

fn spawn_api(runtime: &FixtureRuntime) -> Result<ApiProcess> {
    let address = reserve_tcp_addr();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fin-api"))
        .env("FIN_HOME", &runtime.home)
        .arg("start")
        .arg("--transport")
        .arg("tcp")
        .arg("--tcp-addr")
        .arg(address.to_string())
        .arg("--config-path")
        .arg(&runtime.config_path)
        .arg("--db-path")
        .arg(&runtime.db_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn fin-api binary")?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match request_json(address, "/__probe") {
            Ok((200, _)) => return Ok(ApiProcess { child, address }),
            Ok(_) => thread::sleep(Duration::from_millis(50)),
            Err(_) => {
                if let Some(status) = child.try_wait().context("poll fin-api child")? {
                    let mut stderr = String::new();
                    if let Some(mut handle) = child.stderr.take() {
                        let _ = handle.read_to_string(&mut stderr);
                    }
                    bail!("fin-api exited before readiness: {status}\nstderr:\n{stderr}",);
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    let mut stderr = String::new();
    if let Some(mut handle) = child.stderr.take() {
        let _ = handle.read_to_string(&mut stderr);
    }
    bail!("timed out waiting for fin-api readiness\nstderr:\n{stderr}")
}

fn request_json(address: SocketAddr, path: &str) -> Result<(u16, Value)> {
    let mut stream = TcpStream::connect(address).with_context(|| format!("connect {path}"))?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .with_context(|| format!("write request for {path}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .with_context(|| format!("read response for {path}"))?;
    parse_http_json(&response)
}

fn parse_http_json(response: &str) -> Result<(u16, Value)> {
    let (head, body) = response
        .split_once("\r\n\r\n")
        .context("split http response")?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .context("extract http status")?
        .parse::<u16>()
        .context("parse http status")?;
    let json = serde_json::from_str(body).context("parse json response body")?;
    Ok((status, json))
}

fn normalize_snapshot(value: &mut Value, home: &Path) {
    match value {
        Value::Object(map) => {
            if let Some(meta) = map.get_mut("meta").and_then(Value::as_object_mut) {
                meta.insert("elapsed".to_owned(), Value::from(0));
            }
            for child in map.values_mut() {
                normalize_snapshot(child, home);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_snapshot(item, home);
            }
        }
        Value::String(text) => {
            let home_str = home.display().to_string();
            if text.contains(&home_str) {
                *text = text.replace(&home_str, "$FIN_HOME");
            } else if text.starts_with("SystemTime {") {
                *text = "<SYSTEM_TIME>".to_owned();
            }
        }
        _ => {}
    }
}

fn snapshot_document(address: SocketAddr, home: &Path) -> Result<Value> {
    let mut snapshot = Map::<String, Value>::new();

    let (version_status, version_body) = request_json(address, "/v1/version")?;
    snapshot.insert("version_status".to_owned(), Value::from(version_status));
    snapshot.insert("version".to_owned(), version_body);

    let (tools_status, tools_body) = request_json(address, "/v1/tools")?;
    snapshot.insert("tools_status".to_owned(), Value::from(tools_status));
    snapshot.insert("tools".to_owned(), tools_body);

    let (health_status, health_body) = request_json(address, "/v1/health")?;
    snapshot.insert("health_status".to_owned(), Value::from(health_status));
    snapshot.insert("health".to_owned(), health_body);

    let (config_status, config_body) = request_json(address, "/v1/config/show")?;
    snapshot.insert("config_show_status".to_owned(), Value::from(config_status));
    snapshot.insert("config_show".to_owned(), config_body);

    let (rules_status, rules_body) = request_json(address, "/v1/rules/show")?;
    snapshot.insert("rules_show_status".to_owned(), Value::from(rules_status));
    snapshot.insert("rules_show".to_owned(), rules_body);

    let (sanitize_status, sanitize_body) = request_json(address, "/v1/sanitize/discover?min=2")?;
    snapshot.insert(
        "sanitize_discover_status".to_owned(),
        Value::from(sanitize_status),
    );
    snapshot.insert("sanitize_discover".to_owned(), sanitize_body);

    let (accounts_status, accounts_body) =
        request_json(address, "/v1/view/accounts?group=personal")?;
    snapshot.insert(
        "view_accounts_status".to_owned(),
        Value::from(accounts_status),
    );
    snapshot.insert("view_accounts".to_owned(), accounts_body);

    let (transactions_status, transactions_body) =
        request_json(address, "/v1/view/transactions?group=personal&limit=1")?;
    let posting_id = transactions_body["data"]["items"][0]["posting_id"]
        .as_str()
        .context("transaction detail posting id")?;
    snapshot.insert(
        "view_transactions_status".to_owned(),
        Value::from(transactions_status),
    );
    snapshot.insert("view_transactions".to_owned(), transactions_body.clone());

    let detail_path = format!("/v1/view/transactions/{posting_id}");
    let (detail_status, detail_body) = request_json(address, &detail_path)?;
    snapshot.insert(
        "view_transaction_detail_status".to_owned(),
        Value::from(detail_status),
    );
    snapshot.insert("view_transaction_detail".to_owned(), detail_body);

    let (ledger_status, ledger_body) = request_json(address, "/v1/view/ledger?limit=1")?;
    snapshot.insert("view_ledger_status".to_owned(), Value::from(ledger_status));
    snapshot.insert("view_ledger".to_owned(), ledger_body);

    let (balance_status, balance_body) = request_json(address, "/v1/view/balance")?;
    snapshot.insert(
        "view_balance_status".to_owned(),
        Value::from(balance_status),
    );
    snapshot.insert("view_balance".to_owned(), balance_body);

    let (cashflow_status, cashflow_body) =
        request_json(address, "/v1/report/cashflow?group=business&months=12")?;
    snapshot.insert(
        "report_cashflow_status".to_owned(),
        Value::from(cashflow_status),
    );
    snapshot.insert("report_cashflow".to_owned(), cashflow_body);

    let (report_health_status, report_health_body) =
        request_json(address, "/v1/report/health?group=business")?;
    snapshot.insert(
        "report_health_status".to_owned(),
        Value::from(report_health_status),
    );
    snapshot.insert("report_health".to_owned(), report_health_body);

    let (runway_status, runway_body) = request_json(address, "/v1/report/runway?group=personal")?;
    snapshot.insert(
        "report_runway_status".to_owned(),
        Value::from(runway_status),
    );
    snapshot.insert("report_runway".to_owned(), runway_body);

    let (reserves_status, reserves_body) =
        request_json(address, "/v1/report/reserves?group=business")?;
    snapshot.insert(
        "report_reserves_status".to_owned(),
        Value::from(reserves_status),
    );
    snapshot.insert("report_reserves".to_owned(), reserves_body);

    let (categories_status, categories_body) = request_json(
        address,
        "/v1/report/categories?group=business&mode=breakdown&months=6&limit=5&to=2026-03-31",
    )?;
    snapshot.insert(
        "report_categories_status".to_owned(),
        Value::from(categories_status),
    );
    snapshot.insert("report_categories".to_owned(), categories_body);

    let (audit_status, audit_body) = request_json(
        address,
        "/v1/report/audit?account=Expenses%3ABusiness%3ASoftware&months=6&limit=5&to=2026-03-31",
    )?;
    snapshot.insert("report_audit_status".to_owned(), Value::from(audit_status));
    snapshot.insert("report_audit".to_owned(), audit_body);

    let (summary_status, summary_body) =
        request_json(address, "/v1/report/summary?months=12&to=2026-03-31")?;
    snapshot.insert(
        "report_summary_status".to_owned(),
        Value::from(summary_status),
    );
    snapshot.insert("report_summary".to_owned(), summary_body);

    let (allocation_status, allocation_body) = request_json(
        address,
        "/v1/dashboard/allocation?group=personal&month=2026-03",
    )?;
    snapshot.insert(
        "dashboard_allocation_status".to_owned(),
        Value::from(allocation_status),
    );
    snapshot.insert("dashboard_allocation".to_owned(), allocation_body);

    let (hierarchy_status, hierarchy_body) = request_json(
        address,
        "/v1/dashboard/hierarchy?group=business&months=6&mode=monthly_average&to=2026-03-31",
    )?;
    snapshot.insert(
        "dashboard_hierarchy_status".to_owned(),
        Value::from(hierarchy_status),
    );
    snapshot.insert("dashboard_hierarchy".to_owned(), hierarchy_body);

    let (flow_status, flow_body) = request_json(
        address,
        "/v1/dashboard/flow?group=business&months=6&mode=monthly_average&to=2026-03-31",
    )?;
    snapshot.insert("dashboard_flow_status".to_owned(), Value::from(flow_status));
    snapshot.insert("dashboard_flow".to_owned(), flow_body);

    let (balances_status, balances_body) = request_json(
        address,
        "/v1/dashboard/balances?account=Assets%3APersonal%3AChecking&downsampleMinStepDays=30",
    )?;
    snapshot.insert(
        "dashboard_balances_status".to_owned(),
        Value::from(balances_status),
    );
    snapshot.insert("dashboard_balances".to_owned(), balances_body);

    let (contrib_status, contrib_body) = request_json(
        address,
        "/v1/dashboard/contributions?account=Assets%3APersonal%3AInvestments&downsampleMinStepDays=30",
    )?;
    snapshot.insert(
        "dashboard_contributions_status".to_owned(),
        Value::from(contrib_status),
    );
    snapshot.insert("dashboard_contributions".to_owned(), contrib_body);

    let (projection_status, projection_body) =
        request_json(address, "/v1/dashboard/projection?group=business&months=12")?;
    snapshot.insert(
        "dashboard_projection_status".to_owned(),
        Value::from(projection_status),
    );
    snapshot.insert("dashboard_projection".to_owned(), projection_body);

    let mut document = Value::Object(snapshot);
    normalize_snapshot(&mut document, home);
    Ok(document)
}

#[test]
fn contract_snapshot_matches_golden_file() {
    let runtime = fixture_runtime();
    let api = spawn_api(&runtime).expect("spawn fin-api");
    let actual = snapshot_document(api.address, &runtime.home).expect("collect snapshot");

    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/contract.json");
    if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
        fs::create_dir_all(golden_path.parent().expect("golden parent")).expect("golden dir");
        fs::write(
            &golden_path,
            serde_json::to_string_pretty(&actual).expect("serialize snapshot"),
        )
        .expect("write golden snapshot");
    }

    let expected = fs::read_to_string(&golden_path).expect("read golden snapshot");
    let expected: Value = serde_json::from_str(&expected).expect("parse golden snapshot");

    assert_eq!(actual, expected);
}

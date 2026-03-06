use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fin_sdk::ToolMeta;
use fin_sdk::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};
use serde_json::Value;
use tempfile::{TempDir, tempdir};

struct FixtureRuntime {
    _temp: TempDir,
    home: PathBuf,
}

fn fixture_home() -> FixtureRuntime {
    let temp = tempdir().expect("tempdir");
    let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
        .expect("materialize fixture");
    FixtureRuntime {
        _temp: temp,
        home: fixture.paths.home_dir,
    }
}

fn run_json(home: &Path, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_fin"))
        .env("FIN_HOME", home)
        .args(args)
        .output()
        .expect("run fin");

    assert!(
        output.status.success(),
        "command failed: fin {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    serde_json::from_slice(&output.stdout).expect("parse json envelope")
}

fn data_keys(document: &Value) -> BTreeSet<String> {
    document["data"]
        .as_object()
        .expect("data object")
        .keys()
        .cloned()
        .collect()
}

fn tools_registry(home: &Path) -> BTreeMap<String, ToolMeta> {
    let tools = run_json(home, &["--json", "tools"]);
    let entries: Vec<ToolMeta> =
        serde_json::from_value(tools["data"]["tools"].clone()).expect("tool registry");
    entries
        .into_iter()
        .map(|tool| (tool.name.clone(), tool))
        .collect()
}

fn write_legacy_rules_ts(home: &Path) -> (PathBuf, PathBuf) {
    let source = home.join("data/fin.rules.ts");
    let target = home.join("data/fin.rules.generated.json");
    fs::write(
        &source,
        r#"
export const NAME_MAPPING_CONFIG = {
  rules: [{ patterns: ["WISE"], target: "Wise" }],
  warnOnUnmapped: true,
  fallbackToRaw: false,
};
"#,
    )
    .expect("write legacy rules ts");
    (source, target)
}

#[test]
fn tools_snapshot_matches_golden_file() {
    let fixture = fixture_home();
    let mut tools = run_json(&fixture.home, &["--json", "tools"]);
    tools["meta"]["elapsed"] = Value::from(0);

    let expected =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/tools.json"))
            .expect("read golden snapshot");
    let expected: Value = serde_json::from_str(&expected).expect("parse golden snapshot");

    assert_eq!(tools, expected);
}

#[test]
fn registry_metadata_matches_real_command_payloads() {
    let fixture = fixture_home();
    let registry = tools_registry(&fixture.home);

    let orientation = ["version", "tools", "health"];
    for name in orientation {
        assert!(
            registry.contains_key(name),
            "missing orientation tool: {name}"
        );
    }

    let tools_entry = registry.get("tools").expect("tools entry");
    assert!(tools_entry.read_only);
    assert!(tools_entry.supports_json);
    assert!(!tools_entry.interactive_only);

    let tui_entry = registry.get("tui.start").expect("tui.start entry");
    assert!(!tui_entry.supports_json);
    assert!(tui_entry.interactive_only);

    let tx_list = run_json(
        &fixture.home,
        &[
            "--json",
            "view",
            "transactions",
            "--group",
            "personal",
            "--limit",
            "5",
        ],
    );
    let entry_id = tx_list["data"]["transactions"][0]["id"]
        .as_str()
        .expect("transaction id")
        .to_owned();

    let category_list = run_json(
        &fixture.home,
        &[
            "--json",
            "report",
            "categories",
            "--group",
            "personal",
            "--mode",
            "breakdown",
            "--months",
            "6",
            "--limit",
            "5",
        ],
    );
    let audit_account = category_list["data"]["categories"][0]["category"]
        .as_str()
        .expect("audit account")
        .to_owned();

    let (legacy_rules_source, legacy_rules_target) = write_legacy_rules_ts(&fixture.home);

    let samples: Vec<(&str, Vec<Vec<String>>)> = vec![
        ("version", vec![vec!["--json".into(), "version".into()]]),
        (
            "tools",
            vec![
                vec!["--json".into(), "tools".into()],
                vec!["--json".into(), "tools".into(), "version".into()],
            ],
        ),
        ("health", vec![vec!["--json".into(), "health".into()]]),
        (
            "config.show",
            vec![vec!["--json".into(), "config".into(), "show".into()]],
        ),
        (
            "config.validate",
            vec![vec!["--json".into(), "config".into(), "validate".into()]],
        ),
        (
            "rules.show",
            vec![vec!["--json".into(), "rules".into(), "show".into()]],
        ),
        (
            "rules.validate",
            vec![vec!["--json".into(), "rules".into(), "validate".into()]],
        ),
        (
            "rules.migrate_ts",
            vec![vec![
                "--json".into(),
                "rules".into(),
                "migrate-ts".into(),
                "--source".into(),
                legacy_rules_source.display().to_string(),
                "--target".into(),
                legacy_rules_target.display().to_string(),
            ]],
        ),
        ("import", vec![vec!["--json".into(), "import".into()]]),
        (
            "sanitize.discover",
            vec![vec![
                "--json".into(),
                "sanitize".into(),
                "discover".into(),
                "--min".into(),
                "2".into(),
            ]],
        ),
        (
            "sanitize.migrate",
            vec![vec![
                "--json".into(),
                "sanitize".into(),
                "migrate".into(),
                "--dry-run".into(),
            ]],
        ),
        (
            "sanitize.recategorize",
            vec![vec![
                "--json".into(),
                "sanitize".into(),
                "recategorize".into(),
                "--dry-run".into(),
            ]],
        ),
        (
            "view.accounts",
            vec![vec![
                "--json".into(),
                "view".into(),
                "accounts".into(),
                "--group".into(),
                "personal".into(),
            ]],
        ),
        (
            "view.transactions",
            vec![vec![
                "--json".into(),
                "view".into(),
                "transactions".into(),
                "--group".into(),
                "personal".into(),
                "--limit".into(),
                "5".into(),
            ]],
        ),
        (
            "view.ledger",
            vec![vec![
                "--json".into(),
                "view".into(),
                "ledger".into(),
                "--limit".into(),
                "5".into(),
            ]],
        ),
        (
            "view.balance",
            vec![vec!["--json".into(), "view".into(), "balance".into()]],
        ),
        (
            "view.void",
            vec![vec![
                "--json".into(),
                "view".into(),
                "void".into(),
                entry_id.clone(),
                "--dry-run".into(),
            ]],
        ),
        (
            "edit.transaction",
            vec![vec![
                "--json".into(),
                "edit".into(),
                "transaction".into(),
                entry_id,
                "--description".into(),
                "contract parity test".into(),
                "--dry-run".into(),
            ]],
        ),
        (
            "report.cashflow",
            vec![vec![
                "--json".into(),
                "report".into(),
                "cashflow".into(),
                "--group".into(),
                "personal".into(),
                "--months".into(),
                "6".into(),
            ]],
        ),
        (
            "report.health",
            vec![vec![
                "--json".into(),
                "report".into(),
                "health".into(),
                "--group".into(),
                "personal".into(),
            ]],
        ),
        (
            "report.runway",
            vec![
                vec![
                    "--json".into(),
                    "report".into(),
                    "runway".into(),
                    "--group".into(),
                    "personal".into(),
                ],
                vec![
                    "--json".into(),
                    "report".into(),
                    "runway".into(),
                    "--consolidated".into(),
                    "--include".into(),
                    "business,personal".into(),
                ],
            ],
        ),
        (
            "report.reserves",
            vec![vec![
                "--json".into(),
                "report".into(),
                "reserves".into(),
                "--group".into(),
                "business".into(),
            ]],
        ),
        (
            "report.categories",
            vec![
                vec![
                    "--json".into(),
                    "report".into(),
                    "categories".into(),
                    "--group".into(),
                    "personal".into(),
                    "--mode".into(),
                    "breakdown".into(),
                    "--months".into(),
                    "6".into(),
                    "--limit".into(),
                    "5".into(),
                ],
                vec![
                    "--json".into(),
                    "report".into(),
                    "categories".into(),
                    "--group".into(),
                    "personal".into(),
                    "--mode".into(),
                    "median".into(),
                    "--months".into(),
                    "6".into(),
                    "--limit".into(),
                    "5".into(),
                ],
            ],
        ),
        (
            "report.audit",
            vec![vec![
                "--json".into(),
                "report".into(),
                "audit".into(),
                "--account".into(),
                audit_account,
                "--months".into(),
                "6".into(),
                "--limit".into(),
                "5".into(),
            ]],
        ),
        (
            "report.summary",
            vec![vec![
                "--json".into(),
                "report".into(),
                "summary".into(),
                "--months".into(),
                "12".into(),
            ]],
        ),
    ];

    for (tool_name, invocations) in samples {
        let tool = registry
            .get(tool_name)
            .unwrap_or_else(|| panic!("missing {tool_name}"));
        assert!(
            tool.supports_json,
            "contract test only covers JSON-capable tools: {tool_name}"
        );

        let mut observed = BTreeSet::new();
        for invocation in invocations {
            let refs = invocation.iter().map(String::as_str).collect::<Vec<_>>();
            let output = run_json(&fixture.home, &refs);
            observed.extend(data_keys(&output));
        }

        let expected = tool.output_fields.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(observed, expected, "output field drift for {tool_name}");
    }
}

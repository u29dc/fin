use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::api::{ApiState, build_router};
use crate::runtime::{StartPlan, TransportBinding};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundEndpoint {
    Unix(PathBuf),
    Tcp(std::net::SocketAddr),
}

impl BoundEndpoint {
    #[must_use]
    pub fn log_line(&self) -> String {
        match self {
            Self::Unix(path) => format!("transport=unix | socket={}", path.display()),
            Self::Tcp(addr) => format!("transport=tcp | address={addr}"),
        }
    }

    #[must_use]
    pub fn transport_name(&self) -> &'static str {
        match self {
            Self::Unix(_) => "unix",
            Self::Tcp(_) => "tcp",
        }
    }

    #[must_use]
    pub fn endpoint_label(&self) -> String {
        match self {
            Self::Unix(path) => path.display().to_string(),
            Self::Tcp(addr) => addr.to_string(),
        }
    }
}

pub async fn serve_with_shutdown<F>(
    plan: StartPlan,
    shutdown: F,
    ready: Option<oneshot::Sender<BoundEndpoint>>,
) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let state = ApiState::new(
        match &plan.transport {
            TransportBinding::Unix { socket_path } => BoundEndpoint::Unix(socket_path.clone()),
            TransportBinding::Tcp { bind_addr } => BoundEndpoint::Tcp(*bind_addr),
        },
        plan.config_path_override.clone(),
        plan.db_path_override.clone(),
    );
    match plan.transport {
        TransportBinding::Unix { socket_path } => {
            #[cfg(unix)]
            {
                serve_unix(socket_path, state, shutdown, ready).await
            }
            #[cfg(not(unix))]
            {
                let _ = shutdown;
                let _ = ready;
                bail!("unix transport is unavailable on this platform")
            }
        }
        TransportBinding::Tcp { bind_addr } => serve_tcp(bind_addr, state, shutdown, ready).await,
    }
}

async fn serve_tcp<F>(
    bind_addr: std::net::SocketAddr,
    state: ApiState,
    shutdown: F,
    ready: Option<oneshot::Sender<BoundEndpoint>>,
) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("bind fin-api tcp listener at {bind_addr}"))?;
    let endpoint = BoundEndpoint::Tcp(
        listener
            .local_addr()
            .context("read bound fin-api tcp address")?,
    );
    let router = build_router(ApiState::new(
        endpoint.clone(),
        state.config_path_override,
        state.db_path_override,
    ));
    if let Some(sender) = ready {
        let _ = sender.send(endpoint);
    }
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
        .context("serve fin-api over tcp")?;
    Ok(())
}

#[cfg(unix)]
async fn serve_unix<F>(
    socket_path: PathBuf,
    state: ApiState,
    shutdown: F,
    ready: Option<oneshot::Sender<BoundEndpoint>>,
) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    use tokio::net::UnixListener;

    ensure_parent_dir(&socket_path)?;
    cleanup_stale_socket(&socket_path)?;

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("bind fin-api unix socket at {}", socket_path.display()))?;
    let endpoint = BoundEndpoint::Unix(socket_path.clone());
    let router = build_router(ApiState::new(
        endpoint.clone(),
        state.config_path_override,
        state.db_path_override,
    ));
    if let Some(sender) = ready {
        let _ = sender.send(endpoint);
    }
    let serve_result = axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await;
    cleanup_socket_file(&socket_path)?;
    serve_result.context("serve fin-api over unix socket")?;
    Ok(())
}

#[cfg(unix)]
fn ensure_parent_dir(socket_path: &Path) -> Result<()> {
    let Some(parent) = socket_path.parent() else {
        bail!(
            "socket path {} has no parent directory",
            socket_path.display()
        );
    };
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create fin-api socket directory {}", parent.display()))?;
    Ok(())
}

#[cfg(unix)]
fn cleanup_stale_socket(socket_path: &Path) -> Result<()> {
    use std::os::unix::fs::FileTypeExt;

    let Ok(metadata) = std::fs::symlink_metadata(socket_path) else {
        return Ok(());
    };
    if metadata.file_type().is_socket() {
        std::fs::remove_file(socket_path).with_context(|| {
            format!(
                "remove stale fin-api socket before bind {}",
                socket_path.display()
            )
        })?;
        return Ok(());
    }
    bail!(
        "refusing to remove non-socket path at {}",
        socket_path.display()
    )
}

#[cfg(unix)]
fn cleanup_socket_file(socket_path: &Path) -> Result<()> {
    match std::fs::remove_file(socket_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "remove fin-api socket during shutdown {}",
                socket_path.display()
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::{Context, Result};
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    use super::{BoundEndpoint, serve_with_shutdown};
    use crate::{
        cli::{StartArgs, TransportKind},
        runtime::prepare_start_plan,
    };

    #[tokio::test]
    async fn tcp_probe_endpoint_responds() -> Result<()> {
        let plan = prepare_start_plan(&StartArgs {
            config_path: None,
            db_path: None,
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            serve_with_shutdown(
                plan,
                async move {
                    let _ = shutdown_rx.await;
                },
                Some(ready_tx),
            )
            .await
        });

        let endpoint = timeout(Duration::from_secs(3), ready_rx)
            .await
            .expect("ready timeout")
            .expect("ready sender dropped");
        let BoundEndpoint::Tcp(address) = endpoint else {
            panic!("expected tcp endpoint");
        };
        let response = request_tcp(address, "/__probe").await?;
        assert!(response.contains("200 OK"));
        assert!(response.contains("\"transport\":\"tcp\""));

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_probe_endpoint_responds_and_cleans_up_socket() -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let socket_path = temp.path().join("fin-api.sock");
        let plan = prepare_start_plan(&StartArgs {
            config_path: None,
            db_path: None,
            socket_path: Some(socket_path.clone()),
            tcp_addr: None,
            transport: Some(TransportKind::Unix),
            check_runtime: false,
        })?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            serve_with_shutdown(
                plan,
                async move {
                    let _ = shutdown_rx.await;
                },
                Some(ready_tx),
            )
            .await
        });

        let endpoint = timeout(Duration::from_secs(3), ready_rx)
            .await
            .expect("ready timeout")
            .expect("ready sender dropped");
        let BoundEndpoint::Unix(bound_path) = endpoint else {
            panic!("expected unix endpoint");
        };
        let response = request_unix(&bound_path, "/__probe").await?;
        assert!(response.contains("200 OK"));
        assert!(response.contains("\"transport\":\"unix\""));
        assert!(socket_path.exists());

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        assert!(!socket_path.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_start_removes_stale_socket_before_bind() -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let socket_path = temp.path().join("stale.sock");
        let stale_listener =
            std::os::unix::net::UnixListener::bind(&socket_path).expect("create stale socket");
        drop(stale_listener);
        assert!(socket_path.exists());

        let plan = prepare_start_plan(&StartArgs {
            config_path: None,
            db_path: None,
            socket_path: Some(socket_path.clone()),
            tcp_addr: None,
            transport: Some(TransportKind::Unix),
            check_runtime: false,
        })?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            serve_with_shutdown(
                plan,
                async move {
                    let _ = shutdown_rx.await;
                },
                Some(ready_tx),
            )
            .await
        });

        let endpoint = timeout(Duration::from_secs(3), ready_rx)
            .await
            .expect("ready timeout")
            .expect("ready sender dropped");
        let BoundEndpoint::Unix(bound_path) = endpoint else {
            panic!("expected unix endpoint");
        };
        let response = request_unix(&bound_path, "/__probe").await?;
        assert!(response.contains("200 OK"));

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn version_endpoint_matches_cli_contract() -> Result<()> {
        let plan = prepare_start_plan(&StartArgs {
            config_path: None,
            db_path: None,
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            serve_with_shutdown(
                plan,
                async move {
                    let _ = shutdown_rx.await;
                },
                Some(ready_tx),
            )
            .await
        });

        let endpoint = timeout(Duration::from_secs(3), ready_rx)
            .await
            .expect("ready timeout")
            .expect("ready sender dropped");
        let BoundEndpoint::Tcp(address) = endpoint else {
            panic!("expected tcp endpoint");
        };
        let response = request_tcp(address, "/v1/version").await?;
        let (status, body) = parse_http_json(&response)?;
        assert_eq!(status, 200);
        assert_eq!(body["ok"], true);
        assert_eq!(body["data"]["tool"], "version");
        assert!(body["data"]["sdk"].as_str().is_some());
        assert_eq!(body["meta"]["tool"], "version");

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn tools_endpoints_expose_catalog_and_detail() -> Result<()> {
        let plan = prepare_start_plan(&StartArgs {
            config_path: None,
            db_path: None,
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            serve_with_shutdown(
                plan,
                async move {
                    let _ = shutdown_rx.await;
                },
                Some(ready_tx),
            )
            .await
        });

        let endpoint = timeout(Duration::from_secs(3), ready_rx)
            .await
            .expect("ready timeout")
            .expect("ready sender dropped");
        let BoundEndpoint::Tcp(address) = endpoint else {
            panic!("expected tcp endpoint");
        };

        let catalog = request_tcp(address, "/v1/tools").await?;
        let (status, body) = parse_http_json(&catalog)?;
        assert_eq!(status, 200);
        assert_eq!(body["ok"], true);
        assert_eq!(body["meta"]["tool"], "tools");
        assert!(body["meta"]["count"].as_u64().is_some());
        assert!(body["data"]["tools"].as_array().is_some());
        assert!(body["data"]["globalFlags"].as_array().is_some());

        let detail = request_tcp(address, "/v1/tools/view.transactions").await?;
        let (detail_status, detail_body) = parse_http_json(&detail)?;
        assert_eq!(detail_status, 200);
        assert_eq!(detail_body["ok"], true);
        assert_eq!(detail_body["data"]["tool"]["name"], "view.transactions");

        let missing = request_tcp(address, "/v1/tools/nope").await?;
        let (missing_status, missing_body) = parse_http_json(&missing)?;
        assert_eq!(missing_status, 404);
        assert_eq!(missing_body["ok"], false);
        assert_eq!(missing_body["error"]["code"], "NOT_FOUND");
        assert_eq!(missing_body["meta"]["tool"], "tools");

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn health_endpoint_uses_runtime_overrides() -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let fixture = fin_sdk::testing::fixture::materialize_fixture_home(
            temp.path(),
            &fin_sdk::testing::fixture::FixtureBuildOptions::default(),
        )?;
        let plan = prepare_start_plan(&StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            serve_with_shutdown(
                plan,
                async move {
                    let _ = shutdown_rx.await;
                },
                Some(ready_tx),
            )
            .await
        });

        let endpoint = timeout(Duration::from_secs(3), ready_rx)
            .await
            .expect("ready timeout")
            .expect("ready sender dropped");
        let BoundEndpoint::Tcp(address) = endpoint else {
            panic!("expected tcp endpoint");
        };

        let response = request_tcp(address, "/v1/health").await?;
        let (status, body) = parse_http_json(&response)?;
        assert_eq!(status, 200);
        assert_eq!(body["ok"], true);
        assert_eq!(body["meta"]["tool"], "health");
        assert!(body["data"]["checks"].as_array().is_some());
        assert!(body["data"]["status"].as_str().is_some());
        assert!(body["data"]["summary"]["ok"].as_u64().is_some());

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn config_rules_and_sanitize_endpoints_use_fixture_runtime() -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let fixture = fin_sdk::testing::fixture::materialize_fixture_home(
            temp.path(),
            &fin_sdk::testing::fixture::FixtureBuildOptions::default(),
        )?;
        let (address, shutdown_tx, server) = spawn_tcp_server(StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .await?;

        let (config_status, config_body) = request_json(address, "/v1/config/show").await?;
        assert_eq!(config_status, 200);
        assert_eq!(config_body["ok"], true);
        assert_eq!(config_body["meta"]["tool"], "config.show");
        assert!(
            config_body["data"]["groups"]
                .as_array()
                .is_some_and(|groups| !groups.is_empty())
        );
        assert!(config_body["data"]["configPath"].as_str().is_some());

        let (validate_status, validate_body) = request_json(address, "/v1/config/validate").await?;
        assert_eq!(validate_status, 200);
        assert_eq!(validate_body["ok"], true);
        assert_eq!(validate_body["data"]["valid"], true);

        let (rules_status, rules_body) = request_json(address, "/v1/rules/show").await?;
        assert_eq!(rules_status, 200);
        assert_eq!(rules_body["ok"], true);
        assert_eq!(rules_body["meta"]["tool"], "rules.show");
        assert!(
            rules_body["data"]["ruleCount"]
                .as_u64()
                .is_some_and(|count| count > 0)
        );
        assert_eq!(rules_body["data"]["externalLoaded"], true);

        let (rules_validate_status, rules_validate_body) =
            request_json(address, "/v1/rules/validate").await?;
        assert_eq!(rules_validate_status, 200);
        assert_eq!(rules_validate_body["ok"], true);
        assert_eq!(rules_validate_body["data"]["valid"], true);

        let (sanitize_status, sanitize_body) =
            request_json(address, "/v1/sanitize/discover?min=2").await?;
        assert_eq!(sanitize_status, 200);
        assert_eq!(sanitize_body["ok"], true);
        assert_eq!(sanitize_body["meta"]["tool"], "sanitize.discover");
        let descriptions = sanitize_body["data"]["descriptions"]
            .as_array()
            .context("sanitize descriptions array")?;
        assert!(!descriptions.is_empty());
        assert_eq!(
            sanitize_body["data"]["count"].as_u64(),
            Some(descriptions.len() as u64)
        );

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn view_endpoints_support_pagination_detail_and_balance() -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let fixture = fin_sdk::testing::fixture::materialize_fixture_home(
            temp.path(),
            &fin_sdk::testing::fixture::FixtureBuildOptions::default(),
        )?;
        let (address, shutdown_tx, server) = spawn_tcp_server(StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .await?;

        let (accounts_status, accounts_body) =
            request_json(address, "/v1/view/accounts?group=personal").await?;
        assert_eq!(accounts_status, 200);
        assert_eq!(accounts_body["ok"], true);
        assert_eq!(accounts_body["meta"]["tool"], "view.accounts");
        assert!(
            accounts_body["meta"]["count"]
                .as_u64()
                .is_some_and(|count| count > 0)
        );
        assert!(
            accounts_body["data"]["accounts"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        );

        let (page_status, first_page) =
            request_json(address, "/v1/view/transactions?group=personal&limit=1").await?;
        assert_eq!(page_status, 200);
        assert_eq!(first_page["ok"], true);
        assert_eq!(first_page["meta"]["tool"], "view.transactions");
        assert_eq!(first_page["meta"]["count"], 1);
        assert!(
            first_page["meta"]["total"]
                .as_u64()
                .is_some_and(|count| count > 1)
        );
        assert_eq!(first_page["meta"]["hasMore"], true);
        let items = first_page["data"]["items"]
            .as_array()
            .context("first page items")?;
        assert_eq!(items.len(), 1);
        let first_posting_id = items[0]["posting_id"]
            .as_str()
            .context("first posting id")?
            .to_owned();
        let cursor_token = first_page["data"]["nextCursorToken"]
            .as_str()
            .context("next cursor token")?;
        let second_path = format!(
            "/v1/view/transactions?group=personal&limit=1&after={}",
            percent_encode(cursor_token)
        );
        let (second_status, second_page) = request_json(address, &second_path).await?;
        assert_eq!(second_status, 200);
        assert_eq!(second_page["ok"], true);
        let second_items = second_page["data"]["items"]
            .as_array()
            .context("second page items")?;
        assert_eq!(second_items.len(), 1);
        let second_posting_id = second_items[0]["posting_id"]
            .as_str()
            .context("second posting id")?;
        assert_ne!(second_posting_id, first_posting_id);

        let detail_path = format!("/v1/view/transactions/{first_posting_id}");
        let (detail_status, detail_body) = request_json(address, &detail_path).await?;
        assert_eq!(detail_status, 200);
        assert_eq!(detail_body["ok"], true);
        assert_eq!(detail_body["data"]["posting_id"], first_posting_id);

        let (ledger_status, ledger_body) = request_json(address, "/v1/view/ledger?limit=2").await?;
        assert_eq!(ledger_status, 200);
        assert_eq!(ledger_body["ok"], true);
        assert_eq!(ledger_body["meta"]["tool"], "view.ledger");
        assert_eq!(ledger_body["meta"]["count"], 2);
        assert!(
            ledger_body["meta"]["total"]
                .as_u64()
                .is_some_and(|count| count >= 2)
        );

        let (balance_status, balance_body) = request_json(address, "/v1/view/balance").await?;
        assert_eq!(balance_status, 200);
        assert_eq!(balance_body["ok"], true);
        assert_eq!(balance_body["meta"]["tool"], "view.balance");
        assert!(balance_body["data"]["netWorth"].as_i64().is_some());

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn blocked_and_invalid_requests_return_envelope_errors() -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let fixture = fin_sdk::testing::fixture::materialize_fixture_home(
            temp.path(),
            &fin_sdk::testing::fixture::FixtureBuildOptions::default(),
        )?;
        let (address, shutdown_tx, server) = spawn_tcp_server(StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .await?;

        let (invalid_status, invalid_body) = request_json(
            address,
            "/v1/view/transactions?group=personal&account=Assets:Personal:Monzo",
        )
        .await?;
        assert_eq!(invalid_status, 400);
        assert_eq!(invalid_body["ok"], false);
        assert_eq!(invalid_body["error"]["code"], "INVALID_INPUT");
        assert_eq!(invalid_body["meta"]["tool"], "view.transactions");

        let (cursor_status, cursor_body) =
            request_json(address, "/v1/view/transactions?after=not-json").await?;
        assert_eq!(cursor_status, 400);
        assert_eq!(cursor_body["ok"], false);
        assert_eq!(cursor_body["error"]["code"], "INVALID_INPUT");

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;

        let missing_config = temp.path().join("missing.config.toml");
        let (blocked_address, blocked_shutdown_tx, blocked_server) = spawn_tcp_server(StartArgs {
            config_path: Some(missing_config),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .await?;

        let (blocked_status, blocked_body) =
            request_json(blocked_address, "/v1/config/show").await?;
        assert_eq!(blocked_status, 503);
        assert_eq!(blocked_body["ok"], false);
        assert_eq!(blocked_body["error"]["code"], "NO_CONFIG");
        assert_eq!(blocked_body["meta"]["tool"], "config.show");

        let _ = blocked_shutdown_tx.send(());
        blocked_server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn report_endpoints_return_fixture_payloads() -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let fixture = fin_sdk::testing::fixture::materialize_fixture_home(
            temp.path(),
            &fin_sdk::testing::fixture::FixtureBuildOptions::default(),
        )?;
        let (address, shutdown_tx, server) = spawn_tcp_server(StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .await?;

        let (cashflow_status, cashflow_body) =
            request_json(address, "/v1/report/cashflow?group=business&months=12").await?;
        assert_eq!(cashflow_status, 200);
        assert_eq!(cashflow_body["ok"], true);
        assert_eq!(cashflow_body["meta"]["tool"], "report.cashflow");
        assert!(
            cashflow_body["data"]["series"]
                .as_array()
                .is_some_and(|series| !series.is_empty())
        );
        assert!(
            cashflow_body["data"]["totals"]["net_minor"]
                .as_i64()
                .is_some()
        );

        let (health_status, health_body) =
            request_json(address, "/v1/report/health?group=business").await?;
        assert_eq!(health_status, 200);
        assert_eq!(health_body["ok"], true);
        assert_eq!(health_body["meta"]["tool"], "report.health");
        assert!(
            health_body["data"]["latest"]["health_minor"]
                .as_i64()
                .is_some()
        );

        let (runway_status, runway_body) =
            request_json(address, "/v1/report/runway?group=personal").await?;
        assert_eq!(runway_status, 200);
        assert_eq!(runway_body["ok"], true);
        assert_eq!(runway_body["meta"]["tool"], "report.runway");
        assert_eq!(runway_body["data"]["groups"][0], "personal");
        assert!(
            runway_body["data"]["latest"]["runway_months"]
                .as_f64()
                .is_some()
        );

        let (runway_consolidated_status, runway_consolidated_body) = request_json(
            address,
            "/v1/report/runway?consolidated=true&include=personal%2Cbusiness",
        )
        .await?;
        assert_eq!(runway_consolidated_status, 200);
        assert_eq!(runway_consolidated_body["ok"], true);
        assert_eq!(runway_consolidated_body["data"]["groups"][0], "personal");
        assert_eq!(runway_consolidated_body["data"]["groups"][1], "business");

        let (reserves_status, reserves_body) =
            request_json(address, "/v1/report/reserves?group=business").await?;
        assert_eq!(reserves_status, 200);
        assert_eq!(reserves_body["ok"], true);
        assert_eq!(reserves_body["meta"]["tool"], "report.reserves");
        assert!(
            reserves_body["data"]["latest"]["available_minor"]
                .as_i64()
                .is_some()
        );

        let (summary_status, summary_body) =
            request_json(address, "/v1/report/summary?months=12").await?;
        assert_eq!(summary_status, 200);
        assert_eq!(summary_body["ok"], true);
        assert_eq!(summary_body["meta"]["tool"], "report.summary");
        assert!(
            summary_body["data"]["groups"]
                .as_object()
                .is_some_and(|groups| !groups.is_empty())
        );
        assert!(
            summary_body["data"]["consolidated"]["net_worth_minor"]
                .as_i64()
                .is_some()
        );

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn report_categories_audit_and_invalid_mode_are_supported() -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let fixture = fin_sdk::testing::fixture::materialize_fixture_home(
            temp.path(),
            &fin_sdk::testing::fixture::FixtureBuildOptions::default(),
        )?;
        let (address, shutdown_tx, server) = spawn_tcp_server(StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .await?;

        let (breakdown_status, breakdown_body) = request_json(
            address,
            "/v1/report/categories?group=business&mode=breakdown",
        )
        .await?;
        assert_eq!(breakdown_status, 200);
        assert_eq!(breakdown_body["ok"], true);
        assert_eq!(breakdown_body["data"]["mode"], "breakdown");
        assert!(
            breakdown_body["data"]["categories"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );

        let (median_status, median_body) =
            request_json(address, "/v1/report/categories?group=business&mode=median").await?;
        assert_eq!(median_status, 200);
        assert_eq!(median_body["ok"], true);
        assert_eq!(median_body["data"]["mode"], "median");
        assert!(median_body["data"]["estimatedMonthly"].as_i64().is_some());

        let (audit_status, audit_body) = request_json(
            address,
            "/v1/report/audit?account=Expenses%3ABusiness%3ASoftware&months=6&limit=10",
        )
        .await?;
        assert_eq!(audit_status, 200);
        assert_eq!(audit_body["ok"], true);
        assert_eq!(audit_body["meta"]["tool"], "report.audit");
        assert!(
            audit_body["data"]["payees"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );

        let (invalid_status, invalid_body) =
            request_json(address, "/v1/report/categories?group=business&mode=unknown").await?;
        assert_eq!(invalid_status, 400);
        assert_eq!(invalid_body["ok"], false);
        assert_eq!(invalid_body["error"]["code"], "INVALID_INPUT");
        assert_eq!(invalid_body["meta"]["tool"], "report.categories");

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn dashboard_kpis_allocation_hierarchy_and_flow_endpoints_return_payloads() -> Result<()>
    {
        let temp = tempdir().expect("tempdir");
        let fixture = fin_sdk::testing::fixture::materialize_fixture_home(
            temp.path(),
            &fin_sdk::testing::fixture::FixtureBuildOptions::default(),
        )?;
        let (address, shutdown_tx, server) = spawn_tcp_server(StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .await?;

        let (kpis_status, kpis_body) =
            request_json(address, "/v1/dashboard/kpis?group=business&months=18").await?;
        assert_eq!(kpis_status, 200);
        assert_eq!(kpis_body["ok"], true);
        assert_eq!(kpis_body["meta"]["tool"], "dashboard.kpis");
        assert_eq!(kpis_body["data"]["groupId"], "business");
        assert_eq!(kpis_body["data"]["months"], 18);
        assert!(
            kpis_body["data"]["kpis"]["median_spend_minor"]
                .as_i64()
                .is_some()
        );

        let (allocation_status, allocation_body) = request_json(
            address,
            "/v1/dashboard/allocation?group=personal&month=2026-03",
        )
        .await?;
        assert_eq!(allocation_status, 200);
        assert_eq!(allocation_body["ok"], true);
        assert_eq!(allocation_body["meta"]["tool"], "dashboard.allocation");
        assert_eq!(allocation_body["data"]["reportingMonth"], "2026-03");
        assert_eq!(
            allocation_body["data"]["snapshot"]["dashboard"]["basis"],
            "personal_buffer"
        );
        assert!(
            allocation_body["data"]["snapshot"]["dashboard"]["segments"]
                .as_array()
                .is_some_and(|segments| !segments.is_empty())
        );

        let (hierarchy_status, hierarchy_body) = request_json(
            address,
            "/v1/dashboard/hierarchy?group=business&months=6&mode=monthly_average",
        )
        .await?;
        assert_eq!(hierarchy_status, 200);
        assert_eq!(hierarchy_body["ok"], true);
        assert_eq!(hierarchy_body["meta"]["tool"], "dashboard.hierarchy");
        assert_eq!(hierarchy_body["data"]["mode"], "monthly_average");
        assert!(
            hierarchy_body["data"]["nodes"]
                .as_array()
                .is_some_and(|nodes| !nodes.is_empty())
        );
        assert!(hierarchy_body["data"]["totalMinor"].as_i64().is_some());

        let (flow_status, flow_body) = request_json(
            address,
            "/v1/dashboard/flow?group=business&months=6&mode=monthly_average",
        )
        .await?;
        assert_eq!(flow_status, 200);
        assert_eq!(flow_body["ok"], true);
        assert_eq!(flow_body["meta"]["tool"], "dashboard.flow");
        assert_eq!(flow_body["data"]["mode"], "monthly_average");
        assert!(
            flow_body["data"]["graph"]["edges"]
                .as_array()
                .is_some_and(|edges| !edges.is_empty())
        );
        assert!(flow_body["data"]["graph"]["total_minor"].as_i64().is_some());

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn dashboard_balances_contributions_and_projection_endpoints_return_payloads()
    -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let fixture = fin_sdk::testing::fixture::materialize_fixture_home(
            temp.path(),
            &fin_sdk::testing::fixture::FixtureBuildOptions::default(),
        )?;
        let (address, shutdown_tx, server) = spawn_tcp_server(StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .await?;

        let (balances_status, balances_body) =
            request_json(address, "/v1/dashboard/balances").await?;
        assert_eq!(balances_status, 200);
        assert_eq!(balances_body["ok"], true);
        assert_eq!(balances_body["meta"]["tool"], "dashboard.balances");
        assert_eq!(balances_body["data"]["scopeKind"], "all_assets");
        assert!(
            balances_body["data"]["series"]
                .as_array()
                .is_some_and(|series| !series.is_empty())
        );

        let (account_balances_status, account_balances_body) = request_json(
            address,
            "/v1/dashboard/balances?account=Assets%3APersonal%3AChecking&downsampleMinStepDays=14",
        )
        .await?;
        assert_eq!(account_balances_status, 200);
        assert_eq!(account_balances_body["ok"], true);
        assert_eq!(account_balances_body["data"]["scopeKind"], "account");
        assert_eq!(
            account_balances_body["data"]["scopeId"],
            "Assets:Personal:Checking"
        );

        let (contrib_status, contrib_body) = request_json(
            address,
            "/v1/dashboard/contributions?account=Assets%3APersonal%3AInvestments&downsampleMinStepDays=30",
        )
        .await?;
        assert_eq!(contrib_status, 200);
        assert_eq!(contrib_body["ok"], true);
        assert_eq!(contrib_body["meta"]["tool"], "dashboard.contributions");
        assert_eq!(
            contrib_body["data"]["accountId"],
            "Assets:Personal:Investments"
        );
        assert!(
            contrib_body["data"]["series"]
                .as_array()
                .is_some_and(|series| !series.is_empty())
        );

        let (projection_status, projection_body) =
            request_json(address, "/v1/dashboard/projection?group=business&months=12").await?;
        assert_eq!(projection_status, 200);
        assert_eq!(projection_body["ok"], true);
        assert_eq!(projection_body["meta"]["tool"], "dashboard.projection");
        assert_eq!(projection_body["data"]["groups"][0], "business");
        assert_eq!(projection_body["data"]["report"]["scope_kind"], "group");
        assert_eq!(
            projection_body["data"]["report"]["scenarios"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );

        let (consolidated_status, consolidated_body) = request_json(
            address,
            "/v1/dashboard/projection?consolidated=true&include=personal%2Cbusiness&months=12",
        )
        .await?;
        assert_eq!(consolidated_status, 200);
        assert_eq!(consolidated_body["ok"], true);
        assert_eq!(
            consolidated_body["data"]["report"]["scope_kind"],
            "consolidated"
        );
        assert_eq!(consolidated_body["data"]["groups"][0], "personal");
        assert_eq!(consolidated_body["data"]["groups"][1], "business");

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    #[tokio::test]
    async fn dashboard_endpoints_reject_invalid_scope_requests() -> Result<()> {
        let temp = tempdir().expect("tempdir");
        let fixture = fin_sdk::testing::fixture::materialize_fixture_home(
            temp.path(),
            &fin_sdk::testing::fixture::FixtureBuildOptions::default(),
        )?;
        let (address, shutdown_tx, server) = spawn_tcp_server(StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: None,
            tcp_addr: Some("127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .await?;

        let (balances_status, balances_body) = request_json(
            address,
            "/v1/dashboard/balances?group=personal&account=Assets%3APersonal%3AChecking",
        )
        .await?;
        assert_eq!(balances_status, 400);
        assert_eq!(balances_body["ok"], false);
        assert_eq!(balances_body["error"]["code"], "INVALID_INPUT");
        assert_eq!(balances_body["meta"]["tool"], "dashboard.balances");

        let (allocation_status, allocation_body) = request_json(
            address,
            "/v1/dashboard/allocation?group=personal&month=202603",
        )
        .await?;
        assert_eq!(allocation_status, 400);
        assert_eq!(allocation_body["ok"], false);
        assert_eq!(allocation_body["error"]["code"], "INVALID_INPUT");
        assert_eq!(allocation_body["meta"]["tool"], "dashboard.allocation");

        let (projection_status, projection_body) = request_json(
            address,
            "/v1/dashboard/projection?include=personal%2Cbusiness",
        )
        .await?;
        assert_eq!(projection_status, 400);
        assert_eq!(projection_body["ok"], false);
        assert_eq!(projection_body["error"]["code"], "INVALID_INPUT");
        assert_eq!(projection_body["meta"]["tool"], "dashboard.projection");

        let (missing_group_status, missing_group_body) =
            request_json(address, "/v1/dashboard/kpis?group=missing").await?;
        assert_eq!(missing_group_status, 404);
        assert_eq!(missing_group_body["ok"], false);
        assert_eq!(missing_group_body["error"]["code"], "NOT_FOUND");
        assert_eq!(missing_group_body["meta"]["tool"], "dashboard.kpis");

        let _ = shutdown_tx.send(());
        server.await.expect("join server")?;
        Ok(())
    }

    async fn spawn_tcp_server(
        args: StartArgs,
    ) -> Result<(
        std::net::SocketAddr,
        oneshot::Sender<()>,
        tokio::task::JoinHandle<Result<()>>,
    )> {
        let plan = prepare_start_plan(&args)?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            serve_with_shutdown(
                plan,
                async move {
                    let _ = shutdown_rx.await;
                },
                Some(ready_tx),
            )
            .await
        });

        let endpoint = timeout(Duration::from_secs(3), ready_rx)
            .await
            .expect("ready timeout")
            .expect("ready sender dropped");
        let BoundEndpoint::Tcp(address) = endpoint else {
            panic!("expected tcp endpoint");
        };
        Ok((address, shutdown_tx, server))
    }

    async fn request_tcp(address: std::net::SocketAddr, path: &str) -> Result<String> {
        let mut stream = tokio::net::TcpStream::connect(address)
            .await
            .context("connect fin-api tcp probe")?;
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .with_context(|| format!("write fin-api tcp request for {path}"))?;
        let mut bytes = Vec::new();
        stream
            .read_to_end(&mut bytes)
            .await
            .context("read fin-api tcp probe response")?;
        String::from_utf8(bytes).context("decode fin-api tcp probe response")
    }

    #[cfg(unix)]
    async fn request_unix(socket_path: &std::path::Path, path: &str) -> Result<String> {
        let mut stream = tokio::net::UnixStream::connect(socket_path)
            .await
            .context("connect fin-api unix probe")?;
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .with_context(|| format!("write fin-api unix request for {path}"))?;
        let mut bytes = Vec::new();
        stream
            .read_to_end(&mut bytes)
            .await
            .context("read fin-api unix probe response")?;
        String::from_utf8(bytes).context("decode fin-api unix probe response")
    }

    async fn request_json(
        address: std::net::SocketAddr,
        path: &str,
    ) -> Result<(u16, serde_json::Value)> {
        let response = request_tcp(address, path).await?;
        parse_http_json(&response)
    }

    fn percent_encode(value: &str) -> String {
        let mut encoded = String::new();
        for byte in value.bytes() {
            let is_unreserved =
                matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~');
            if is_unreserved {
                encoded.push(char::from(byte));
            } else {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
        encoded
    }

    fn parse_http_json(response: &str) -> Result<(u16, serde_json::Value)> {
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
}

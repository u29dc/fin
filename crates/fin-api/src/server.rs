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

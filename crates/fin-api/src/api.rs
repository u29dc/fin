use std::path::PathBuf;
use std::time::Instant;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::get};
use fin_sdk::{
    EnvelopeMeta, ErrorEnvelope, ErrorPayload, GlobalFlag, HealthCheckOptions, HealthReport,
    SDK_VERSION, SuccessEnvelope, ToolMeta, global_flags, run_health_checks, sdk_banner,
    tool_registry,
};
use serde::Serialize;

use crate::server::BoundEndpoint;

#[derive(Debug, Clone)]
pub struct ApiState {
    pub endpoint: BoundEndpoint,
    pub config_path_override: Option<PathBuf>,
    pub db_path_override: Option<PathBuf>,
}

impl ApiState {
    #[must_use]
    pub fn new(
        endpoint: BoundEndpoint,
        config_path_override: Option<PathBuf>,
        db_path_override: Option<PathBuf>,
    ) -> Self {
        Self {
            endpoint,
            config_path_override,
            db_path_override,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct MetaExtras {
    count: Option<usize>,
    total: Option<usize>,
    has_more: Option<bool>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    payload: ErrorPayload,
    tool: &'static str,
    started: Instant,
}

impl ApiError {
    fn not_found(
        tool: &'static str,
        message: impl Into<String>,
        hint: impl Into<String>,
        started: Instant,
    ) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            payload: ErrorPayload::new("NOT_FOUND", message, hint),
            tool,
            started,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorEnvelope {
            ok: false,
            error: self.payload,
            meta: meta(self.tool, self.started, MetaExtras::default()),
        });
        (self.status, body).into_response()
    }
}

#[derive(Debug, Serialize)]
struct ProbeResponse {
    ok: bool,
    transport: &'static str,
    endpoint: String,
}

#[derive(Debug, Serialize)]
struct VersionPayload {
    tool: &'static str,
    sdk: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolsPayload {
    version: &'static str,
    tools: Vec<ToolMeta>,
    global_flags: Vec<GlobalFlag>,
}

#[derive(Debug, Serialize)]
struct ToolDetailPayload {
    tool: ToolMeta,
}

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/__probe", get(probe_handler))
        .route("/v1/version", get(version_handler))
        .route("/v1/tools", get(tools_handler))
        .route("/v1/tools/{name}", get(tool_detail_handler))
        .route("/v1/health", get(health_handler))
        .with_state(state)
}

async fn probe_handler(State(state): State<ApiState>) -> Json<ProbeResponse> {
    Json(ProbeResponse {
        ok: true,
        transport: state.endpoint.transport_name(),
        endpoint: state.endpoint.endpoint_label(),
    })
}

async fn version_handler() -> Json<SuccessEnvelope<VersionPayload>> {
    let started = Instant::now();
    success(
        "version",
        VersionPayload {
            tool: "version",
            sdk: sdk_banner(),
        },
        started,
        MetaExtras::default(),
    )
}

async fn tools_handler() -> Json<SuccessEnvelope<ToolsPayload>> {
    let started = Instant::now();
    let tools = tool_registry();
    let count = tools.len();
    success(
        "tools",
        ToolsPayload {
            version: SDK_VERSION,
            tools,
            global_flags: global_flags(),
        },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    )
}

async fn tool_detail_handler(
    Path(name): Path<String>,
) -> Result<Json<SuccessEnvelope<ToolDetailPayload>>, ApiError> {
    let started = Instant::now();
    let Some(tool) = tool_registry()
        .into_iter()
        .find(|candidate| candidate.name == name)
    else {
        return Err(ApiError::not_found(
            "tools",
            format!("tool \"{name}\" not found"),
            "Call GET /v1/tools to list all available tools.",
            started,
        ));
    };

    Ok(success(
        "tools",
        ToolDetailPayload { tool },
        started,
        MetaExtras::default(),
    ))
}

async fn health_handler(State(state): State<ApiState>) -> Json<SuccessEnvelope<HealthReport>> {
    let started = Instant::now();
    let report = run_health_checks(HealthCheckOptions {
        config_path: state.config_path_override.clone(),
        db_path: state.db_path_override.clone(),
        ..HealthCheckOptions::default()
    });
    success("health", report, started, MetaExtras::default())
}

fn success<T: Serialize>(
    tool: &'static str,
    data: T,
    started: Instant,
    extras: MetaExtras,
) -> Json<SuccessEnvelope<T>> {
    Json(SuccessEnvelope::new(data, meta(tool, started, extras)))
}

fn meta(tool: &str, started: Instant, extras: MetaExtras) -> EnvelopeMeta {
    let mut meta = EnvelopeMeta::new(tool, elapsed_ms(started));
    meta.count = extras.count;
    meta.total = extras.total;
    meta.has_more = extras.has_more;
    meta
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

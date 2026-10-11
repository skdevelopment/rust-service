//! MCP HTTP server
//!
//! A minimal MCP JSON-RPC endpoint (`POST /mcp`) for remote LLM control.
//!
//! Security model: the server can run commands on the machine with the app's
//! privileges, so
//! - it listens on 127.0.0.1 unless remote access is explicitly enabled,
//! - every request needs `Authorization: Bearer <key>` (compared in constant time),
//! - browser requests (any `Origin` header) are refused, and on loopback the
//!   `Host` header must be local, which blocks DNS-rebinding attacks,
//! - request bodies are size-limited.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{AUTHORIZATION, CONTENT_TYPE, HOST, ORIGIN};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpListener;

use super::tools;
use crate::types::AgentSettings;

type BoxBody = Full<Bytes>;

/// Shortest API key the server accepts (the UI generates 32 hex characters).
pub const MIN_API_KEY_LEN: usize = 16;

/// Largest accepted request body.
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

const PROTOCOL_VERSION: &str = "2024-11-05";

// =============================================================================
// Lifecycle
// =============================================================================

#[derive(Clone, PartialEq, Eq, Debug)]
struct ServerConfig {
    port: u16,
    api_key: String,
    allow_remote: bool,
}

struct RunningServer {
    config: ServerConfig,
    task: tauri::async_runtime::JoinHandle<()>,
}

static RUNNING: Mutex<Option<RunningServer>> = Mutex::new(None);

/// Start, restart or stop the server so it matches the agent settings.
///
/// Called at startup and whenever agent settings change, so enabling the
/// server or rotating its key takes effect without restarting the app.
pub fn apply_settings(agent: &AgentSettings) {
    let desired = desired_config(agent);
    let mut running = crate::lock::lock_mutex(&RUNNING);

    if running.as_ref().map(|r| &r.config) == desired.as_ref() {
        return;
    }

    if let Some(old) = running.take() {
        old.task.abort();
        log::info!("[MCP] Server on port {} stopped", old.config.port);
    }

    if let Some(config) = desired {
        let task_config = config.clone();
        let task = tauri::async_runtime::spawn(async move { serve(task_config).await });
        *running = Some(RunningServer { config, task });
    }
}

fn desired_config(agent: &AgentSettings) -> Option<ServerConfig> {
    if !agent.mcp_server_enabled {
        return None;
    }
    match agent.mcp_api_key.as_deref().map(str::trim) {
        Some(key) if key.len() >= MIN_API_KEY_LEN => Some(ServerConfig {
            port: agent.mcp_port,
            api_key: key.to_string(),
            allow_remote: agent.mcp_allow_remote,
        }),
        Some(_) => {
            log::error!(
                "[MCP] Server enabled but the API key is shorter than {} characters; not starting",
                MIN_API_KEY_LEN
            );
            None
        }
        None => {
            log::error!("[MCP] Server enabled but no API key is configured; not starting");
            None
        }
    }
}

async fn serve(config: ServerConfig) {
    let ip = if config.allow_remote {
        [0, 0, 0, 0]
    } else {
        [127, 0, 0, 1]
    };
    let addr = SocketAddr::from((ip, config.port));

    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("[MCP] Failed to bind {}: {}", addr, e);
            return;
        }
    };
    log::info!("[MCP] Listening on http://{}/mcp", addr);
    if config.allow_remote {
        log::warn!("[MCP] Remote access is enabled; any host that can reach this port and knows the key can control this machine");
    }

    let config = Arc::new(config);
    loop {
        let (stream, remote) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                log::warn!("[MCP] Failed to accept connection: {}", e);
                continue;
            }
        };

        let config = config.clone();
        tauri::async_runtime::spawn(async move {
            let service = service_fn(move |req| {
                let config = config.clone();
                async move { Ok::<_, hyper::Error>(handle(req, &config).await) }
            });
            if let Err(e) = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                log::debug!("[MCP] Connection from {} ended: {}", remote, e);
            }
        });
    }
}

// =============================================================================
// HTTP handling
// =============================================================================

fn respond(status: StatusCode, body: Value) -> Response<BoxBody> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(BoxBody::new(Bytes::from(body.to_string())))
        .unwrap_or_else(|_| Response::new(BoxBody::new(Bytes::new())))
}

fn empty(status: StatusCode) -> Response<BoxBody> {
    Response::builder()
        .status(status)
        .body(BoxBody::new(Bytes::new()))
        .unwrap_or_else(|_| Response::new(BoxBody::new(Bytes::new())))
}

/// Compare secrets without leaking the matching prefix length through timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn is_authorized(req: &Request<Incoming>, api_key: &str) -> bool {
    req.headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .is_some_and(|token| constant_time_eq(token.trim().as_bytes(), api_key.as_bytes()))
}

/// On loopback, only accept requests addressed to a local host name.
fn host_is_local(req: &Request<Incoming>) -> bool {
    let Some(host) = req.headers().get(HOST).and_then(|h| h.to_str().ok()) else {
        return false;
    };
    let name = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host.split(':').next().unwrap_or("")
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1"
    )
}

async fn handle(req: Request<Incoming>, config: &ServerConfig) -> Response<BoxBody> {
    // MCP clients are not browsers. Refusing any Origin stops web pages from
    // driving the server (there are deliberately no CORS headers either).
    if req.headers().contains_key(ORIGIN) {
        return respond(
            StatusCode::FORBIDDEN,
            json!({ "error": "Browser requests are not allowed" }),
        );
    }
    if !config.allow_remote && !host_is_local(&req) {
        return respond(
            StatusCode::FORBIDDEN,
            json!({ "error": "Invalid Host header" }),
        );
    }

    let path = req.uri().path().to_string();
    if path == "/" || path == "/health" {
        return respond(
            StatusCode::OK,
            json!({ "status": "ok", "service": "RustService MCP Server", "tools": tools::tool_count() }),
        );
    }

    if !is_authorized(&req, &config.api_key) {
        log::warn!("[MCP] Unauthorized request to {}", path);
        return respond(
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Unauthorized - Bearer token required" }),
        );
    }
    if path != "/mcp" {
        return respond(
            StatusCode::NOT_FOUND,
            json!({ "error": "Not found - use POST /mcp" }),
        );
    }
    if req.method() != Method::POST {
        return respond(
            StatusCode::METHOD_NOT_ALLOWED,
            json!({ "error": "Method not allowed - use POST" }),
        );
    }

    let body = match Limited::new(req.into_body(), MAX_BODY_BYTES)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            log::warn!("[MCP] Failed to read request body: {}", e);
            return respond(
                StatusCode::PAYLOAD_TOO_LARGE,
                rpc_error(Value::Null, -32700, "Request body too large or unreadable"),
            );
        }
    };

    let request: RpcRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return respond(
                StatusCode::BAD_REQUEST,
                rpc_error(
                    Value::Null,
                    -32700,
                    "Parse error - invalid JSON-RPC request",
                ),
            )
        }
    };

    // Notifications (no id) get no JSON-RPC response.
    let Some(id) = request.id.clone() else {
        log::debug!("[MCP] Notification '{}'", request.method);
        return empty(StatusCode::ACCEPTED);
    };

    log::debug!("[MCP] Request '{}' id={}", request.method, id);
    respond(StatusCode::OK, dispatch(request, id).await)
}

// =============================================================================
// JSON-RPC
// =============================================================================

#[derive(Debug, Deserialize, specta::Type)]
struct RpcRequest {
    method: String,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    id: Option<Value>,
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

async fn dispatch(request: RpcRequest, id: Value) -> Value {
    match request.method.as_str() {
        "initialize" => rpc_result(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "RustService MCP", "version": env!("CARGO_PKG_VERSION") }
            }),
        ),
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, tools::definitions()),
        "tools/call" => {
            let Some(name) = request.params.get("name").and_then(Value::as_str) else {
                return rpc_error(id, -32602, "Missing tool name");
            };
            if !tools::exists(name) {
                return rpc_error(id, -32602, &format!("Unknown tool: {}", name));
            }
            let arguments = request
                .params
                .get("arguments")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();

            // Tool failures are results with isError, per the MCP spec.
            let (text, is_error) = match tools::call(name, arguments).await {
                Ok(text) => (text, false),
                Err(e) => (e, true),
            };
            rpc_result(
                id,
                json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }),
            )
        }
        other => rpc_error(id, -32601, &format!("Method not found: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[test]
    fn short_keys_do_not_start_server() {
        let mut agent = AgentSettings {
            mcp_server_enabled: true,
            mcp_api_key: Some("short".into()),
            ..AgentSettings::default()
        };
        assert!(desired_config(&agent).is_none());
        agent.mcp_api_key = Some("0123456789abcdef0123".into());
        let config = desired_config(&agent).unwrap();
        assert!(!config.allow_remote);
        agent.mcp_server_enabled = false;
        assert!(desired_config(&agent).is_none());
    }
}

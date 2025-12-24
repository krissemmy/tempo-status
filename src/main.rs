use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use axum::http::header::{HeaderName, HeaderValue};

#[derive(Clone)]
struct AppState {
    client: Client,
    node_rpc: String,
    node_auth_header: Option<String>, // e.g. "X-Api-Key: mykey" (optional)
    ref_rpc: String,
    max_lag: u64,
    timeout_ms: u64,
}

#[derive(Debug, Serialize)]
struct StatusPayload {
    ok: bool,
    node_up: bool,
    ref_up: bool,
    node_block: Option<u64>,
    ref_block: Option<u64>,
    blocks_behind: Option<u64>,
    syncing: bool,
    message: String,
}

#[derive(Debug, Deserialize)]
struct RpcResp {
    result: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Config via env vars
    // Your node RPC, e.g. http://127.0.0.1:8545
    let node_rpc = std::env::var("NODE_RPC").unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());

    // Tempo testnet public RPC (official): https://rpc.testnet.tempo.xyz :contentReference[oaicite:1]{index=1}
    let ref_rpc = std::env::var("REF_RPC").unwrap_or_else(|_| "https://rpc.testnet.tempo.xyz".to_string());

    // Allow a small lag before marking "syncing"
    let max_lag: u64 = std::env::var("MAX_LAG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);

    // Timeout for each RPC call
    let timeout_ms: u64 = std::env::var("RPC_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2500);

    // Optional auth header for your node, format: "Header-Name: value"
    // Example: NODE_AUTH_HEADER="X-Api-Key: mysecret"
    let node_auth_header = std::env::var("NODE_AUTH_HEADER").ok();

    let client = Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .context("failed to build reqwest client")?;

    let state = Arc::new(AppState {
        client,
        node_rpc,
        node_auth_header,
        ref_rpc,
        max_lag,
        timeout_ms,
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/status.json", get(status_json))
        .with_state(state);

    let addr: SocketAddr = std::env::var("BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()
        .context("invalid BIND address")?;

    println!("Serving on http://{addr}");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

async fn index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let s = compute_status(&state).await;

    let badge = if s.ok {
        "<span style='padding:6px 10px;border-radius:999px;background:#d1fae5;color:#065f46;'>LIVE</span>"
    } else if s.node_up {
        "<span style='padding:6px 10px;border-radius:999px;background:#fef3c7;color:#92400e;'>SYNCING / DEGRADED</span>"
    } else {
        "<span style='padding:6px 10px;border-radius:999px;background:#fee2e2;color:#991b1b;'>DOWN</span>"
    };

    let node_block = s.node_block.map(|b| b.to_string()).unwrap_or_else(|| "-".to_string());
    let ref_block = s.ref_block.map(|b| b.to_string()).unwrap_or_else(|| "-".to_string());
    let behind = s.blocks_behind.map(|b| b.to_string()).unwrap_or_else(|| "-".to_string());

    let html = format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <meta http-equiv="refresh" content="10">
  <title>Tempo Node Status</title>
  <style>
    body {{ font-family: ui-sans-serif, system-ui, -apple-system; padding: 24px; max-width: 820px; margin: 0 auto; }}
    .card {{ border:1px solid #e5e7eb; border-radius:16px; padding:18px; }}
    .row {{ display:flex; gap:14px; flex-wrap:wrap; }}
    .k {{ color:#6b7280; font-size: 12px; text-transform: uppercase; letter-spacing: .04em; }}
    .v {{ font-size: 20px; }}
    .grid {{ display:grid; grid-template-columns: 1fr 1fr; gap:12px; margin-top:14px; }}
    .mono {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }}
    .small {{ color:#6b7280; font-size: 12px; margin-top: 8px; }}
  </style>
</head>
<body>
  <div class="row" style="justify-content:space-between;align-items:center;">
    <h2 style="margin:0;">Tempo Node Status</h2>
    {badge}
  </div>

  <div class="card" style="margin-top:16px;">
    <div class="grid">
      <div>
        <div class="k">Node RPC</div>
        <div class="v mono">{node_rpc}</div>
      </div>
      <div>
        <div class="k">Reference RPC</div>
        <div class="v mono">{ref_rpc}</div>
      </div>

      <div>
        <div class="k">Node reachable</div>
        <div class="v">{node_up}</div>
      </div>
      <div>
        <div class="k">Reference reachable</div>
        <div class="v">{ref_up}</div>
      </div>

      <div>
        <div class="k">Node block</div>
        <div class="v mono">{node_block}</div>
      </div>
      <div>
        <div class="k">Reference block</div>
        <div class="v mono">{ref_block}</div>
      </div>

      <div>
        <div class="k">Blocks behind</div>
        <div class="v mono">{behind}</div>
      </div>
      <div>
        <div class="k">Max lag</div>
        <div class="v mono">{max_lag}</div>
      </div>
    </div>

    <div class="small">Message: {msg}</div>
    <div class="small">Auto-refreshes every 10s. JSON: <span class="mono">/status.json</span> Health: <span class="mono">/healthz</span></div>
  </div>
</body>
</html>"#,
        badge = badge,
        node_rpc = escape_html(&state.node_rpc),
        ref_rpc = escape_html(&state.ref_rpc),
        node_up = if s.node_up { "yes" } else { "no" },
        ref_up = if s.ref_up { "yes" } else { "no" },
        node_block = node_block,
        ref_block = ref_block,
        behind = behind,
        max_lag = state.max_lag,
        msg = escape_html(&s.message),
    );

    (StatusCode::OK, Html(html))
}

async fn healthz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let s = compute_status(&state).await;
    if s.ok {
        (StatusCode::OK, s.message)
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, s.message)
    }
}

async fn status_json(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let s = compute_status(&state).await;
    (StatusCode::OK, axum::Json(s))
}

async fn compute_status(state: &AppState) -> StatusPayload {
    let (ref_res, node_res) = tokio::join!(
        eth_block_number(&state.client, &state.ref_rpc, None),
        eth_block_number(&state.client, &state.node_rpc, state.node_auth_header.as_deref()),
    );

    let ref_block = ref_res.ok();
    let node_block = node_res.ok();

    let ref_up = ref_block.is_some();
    let node_up = node_block.is_some();

    // If node is down, that's a hard fail.
    if !node_up {
        return StatusPayload {
            ok: false,
            node_up,
            ref_up,
            node_block: None,
            ref_block,
            blocks_behind: None,
            syncing: false,
            message: "Node RPC is not responding".to_string(),
        };
    }

    // If reference is down but node is up, call it OK (degraded),
    // because your node being live is what you can control.
    if !ref_up {
        return StatusPayload {
            ok: true,
            node_up,
            ref_up,
            node_block,
            ref_block: None,
            blocks_behind: None,
            syncing: false,
            message: "Node is up (reference RPC unreachable)".to_string(),
        };
    }

    let nb = node_block.unwrap();
    let rb = ref_block.unwrap();

    if rb > nb {
        let behind = rb - nb;
        let syncing = behind > state.max_lag;
        let ok = !syncing;

        let msg = if syncing {
            format!("Node is syncing: {behind} blocks behind (node={nb}, ref={rb})")
        } else {
            format!("Node is live: node={nb}, ref={rb} (lag={})", rb.saturating_sub(nb))
        };

        StatusPayload {
            ok,
            node_up,
            ref_up,
            node_block: Some(nb),
            ref_block: Some(rb),
            blocks_behind: Some(behind),
            syncing,
            message: msg,
        }
    } else {
        // node ahead or equal (ahead can happen due to ref lag or timing)
        StatusPayload {
            ok: true,
            node_up,
            ref_up,
            node_block: Some(nb),
            ref_block: Some(rb),
            blocks_behind: Some(0),
            syncing: false,
            message: format!("Node is live: node={nb}, ref={rb}"),
        }
    }
}

async fn eth_block_number(client: &Client, rpc_url: &str, auth_header: Option<&str>) -> Result<u64> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_blockNumber",
        "params": []
    });

    let mut req = client.post(rpc_url).json(&body);

    if let Some(h) = auth_header {
        if let Some((k, v)) = h.split_once(':') {
            let mut headers = HeaderMap::new();
    
            let name = k.trim().parse::<HeaderName>().context("invalid header name")?;
            let value = v.trim().parse::<HeaderValue>().context("invalid header value")?;
    
            headers.insert(name, value);
            req = req.headers(headers);
        } else {
            return Err(anyhow!("NODE_AUTH_HEADER must look like 'Header-Name: value'"));
        }
    }
    

    let resp = req.send().await.context("rpc request failed")?;
    if !resp.status().is_success() {
        return Err(anyhow!("rpc http {}", resp.status()));
    }

    let parsed: serde_json::Value = resp.json().await.context("invalid json")?;
    let hex = parsed
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing result"))?;

    // hex string like "0xabc123"
    let n = u64::from_str_radix(hex.trim_start_matches("0x"), 16)
        .context("failed parsing hex block number")?;

    Ok(n)
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

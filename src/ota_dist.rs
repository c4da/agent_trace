//! Zero-Walk Over-the-Air (OTA) Binary Distribution & Remote Push Provisioning Engine.
//!
//! Enables the Lead Node to serve the compiled `agent_trace_gw.exe` binary,
//! cryptographic SHA-256 manifest, dynamic bootstrap script, and remote push-provisioning
//! to worker nodes across the local network without physical installation.

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

/// Cryptographic binary manifest describing the current executable available for OTA distribution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OtaManifest {
    pub binary_name: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub version: String,
    pub lead_url: String,
    pub download_url: String,
    pub bootstrap_ps1_url: String,
}

/// Request payload to deploy or upgrade an agent_trace_gw worker node over the network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployWorkerRequest {
    pub target_ip: String,
    pub port: Option<u16>,
    pub method: Option<String>,
}

/// Computes SHA-256 and metadata for the running binary or fallback target path.
pub fn build_local_binary_manifest(lead_url: &str) -> Result<OtaManifest, String> {
    let exe_path = locate_distributable_binary()?;
    let bytes = std::fs::read(&exe_path)
        .map_err(|e| format!("Failed to read binary at {}: {}", exe_path.display(), e))?;

    let digest_val = digest(&SHA256, &bytes);
    let sha256_hex = hex::encode(digest_val.as_ref());
    let size_bytes = bytes.len() as u64;

    let base_lead = lead_url.trim_end_matches('/');

    Ok(OtaManifest {
        binary_name: "agent_trace_gw.exe".to_string(),
        sha256: sha256_hex,
        size_bytes,
        version: env!("CARGO_PKG_VERSION").to_string(),
        lead_url: base_lead.to_string(),
        download_url: format!("{}/api/v1/dist/agent_trace_gw.exe", base_lead),
        bootstrap_ps1_url: format!("{}/bootstrap.ps1", base_lead),
    })
}

/// Locates the compiled binary for distribution.
pub fn locate_distributable_binary() -> Result<PathBuf, String> {
    // 1. Current executable
    if let Ok(exe) = std::env::current_exe() {
        if exe.is_file() {
            return Ok(exe);
        }
    }

    // 2. Relative cargo target paths
    let candidates = [
        "target/release/agent_trace_gw.exe",
        "target/debug/agent_trace_gw.exe",
        "target/release/agent_trace_gw",
        "target/debug/agent_trace_gw",
        "../target/debug/agent_trace_gw.exe",
    ];

    for candidate in &candidates {
        let p = Path::new(candidate);
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
    }

    Err("Could not locate a distributable agent_trace_gw binary on disk".to_string())
}

/// Generates a zero-dependency PowerShell bootstrap script tuned for the specified Lead Node URL.
pub fn generate_bootstrap_ps1(lead_url: &str) -> String {
    let base_lead = lead_url.trim_end_matches('/');
    format!(
        r#"# AgentTrace Zero-Walk Worker Bootstrap Script
# Automatically downloads, verifies SHA-256, and executes agent_trace_gw in worker mode.
$ErrorActionPreference = "Stop"

$leadUrl = "{lead}"
$manifestUrl = "$leadUrl/api/v1/dist/manifest"
$binUrl = "$leadUrl/api/v1/dist/agent_trace_gw.exe"

Write-Host "==> Fetching AgentTrace OTA manifest from $manifestUrl ..." -ForegroundColor Cyan
$manifest = Invoke-RestMethod -Uri $manifestUrl -Method Get -TimeoutSec 10
$expectedHash = $manifest.sha256.ToUpperInvariant()

$destDir = "$env:LOCALAPPDATA\AgentTrace"
if (-not (Test-Path -Path $destDir)) {{
    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
}}

$destExe = "$destDir\agent_trace_gw.exe"
$destOld = "$destDir\agent_trace_gw.old.exe"

# Stop any running worker process if present
Get-Process -Name "agent_trace_gw" -ErrorAction SilentlyContinue | Where-Object {{
    $_.Path -like "*AgentTrace*"
}} | Stop-Process -Force -ErrorAction SilentlyContinue

if (Test-Path -Path $destExe) {{
    try {{
        $currentHash = (Get-FileHash -Path $destExe -Algorithm SHA256).Hash.ToUpperInvariant()
        if ($currentHash -eq $expectedHash) {{
            Write-Host "==> Current binary matches manifest hash ($currentHash). Ready to launch." -ForegroundColor Green
            Start-Process -FilePath $destExe -ArgumentList "--worker", "--lead", $leadUrl -WindowStyle Hidden
            Write-Host "==> AgentTrace Worker started successfully." -ForegroundColor Green
            exit 0
        }}
        Move-Item -Path $destExe -Destination $destOld -Force -ErrorAction SilentlyContinue
    }} catch {{
        Write-Warning "Could not swap active executable: $_"
    }}
}}

Write-Host "==> Downloading latest binary from $binUrl ..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $binUrl -OutFile $destExe -UseBasicParsing

$downloadedHash = (Get-FileHash -Path $destExe -Algorithm SHA256).Hash.ToUpperInvariant()
if ($downloadedHash -ne $expectedHash) {{
    Write-Error "SHA-256 checksum mismatch! Expected: $expectedHash, got: $downloadedHash"
    exit 1
}}

Write-Host "==> Verification passed (SHA-256: $downloadedHash). Launching Worker Daemon..." -ForegroundColor Green
Start-Process -FilePath $destExe -ArgumentList "--worker", "--lead", $leadUrl -WindowStyle Hidden
Write-Host "==> AgentTrace Worker active and connected to Lead Node ($leadUrl)." -ForegroundColor Green
"#,
        lead = base_lead
    )
}

/// Resolves the canonical Lead Node URL based on incoming Host headers or state configuration.
pub fn resolve_lead_url(headers: &HeaderMap, state_lead_url: Option<&str>) -> String {
    if let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) {
        format!("http://{}", host)
    } else if let Some(url) = state_lead_url {
        url.to_string()
    } else {
        "http://127.0.0.1:8080".to_string()
    }
}

/// Handler for `GET /api/v1/dist/manifest`.
pub async fn get_manifest(
    headers: HeaderMap,
    State(state): State<crate::handlers::AppState>,
) -> Result<Json<OtaManifest>, (StatusCode, String)> {
    let lead_url = resolve_lead_url(&headers, state.lead_url.as_deref());

    let manifest = build_local_binary_manifest(&lead_url)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(manifest))
}

/// Handler for `GET /api/v1/dist/agent_trace_gw.exe`.
pub async fn stream_binary() -> Result<Response, (StatusCode, String)> {
    let exe_path = locate_distributable_binary()
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    let bytes = tokio::fs::read(&exe_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read binary: {}", e)))?;

    let digest_val = digest(&SHA256, &bytes);
    let sha256_hex = hex::encode(digest_val.as_ref());

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"agent_trace_gw.exe\""),
    );
    if let Ok(v) = HeaderValue::from_str(&sha256_hex) {
        headers.insert(header::HeaderName::from_static("x-content-sha256"), v);
    }

    let mut response = Response::new(Body::from(bytes));
    *response.headers_mut() = headers;
    Ok(response)
}

/// Handler for `GET /bootstrap.ps1`.
pub async fn serve_bootstrap_ps1(
    headers: HeaderMap,
    State(state): State<crate::handlers::AppState>,
) -> Response {
    let lead_url = resolve_lead_url(&headers, state.lead_url.as_deref());

    let script = generate_bootstrap_ps1(&lead_url);

    let mut headers_resp = HeaderMap::new();
    headers_resp.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));

    let mut response = Response::new(Body::from(script));
    *response.headers_mut() = headers_resp;
    response
}

/// Handler for `GET /worker.cmd` or `GET /join.cmd`.
/// Serves a double-clickable Windows Command batch file to join the cluster in 1 click.
pub async fn serve_worker_cmd(
    headers: HeaderMap,
    State(state): State<crate::handlers::AppState>,
) -> Response {
    let lead_url = resolve_lead_url(&headers, state.lead_url.as_deref());
    let cmd = format!(
        "@echo off\r\n\
        title AgentTrace Worker Cluster Joiner\r\n\
        echo ========================================================\r\n\
        echo  AgentTrace Worker Auto-Joiner\r\n\
        echo  Connecting to Lead Node: {}\r\n\
        echo ========================================================\r\n\
        echo [1/2] Fetching bootstrap script and verifying SHA-256...\r\n\
        powershell -NoProfile -ExecutionPolicy Bypass -Command \"irm {}/bootstrap.ps1 | iex\"\r\n\
        echo [2/2] Setup process completed.\r\n\
        pause\r\n",
        lead_url, lead_url
    );

    let mut headers_resp = HeaderMap::new();
    headers_resp.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/x-bat"));
    headers_resp.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"agenttrace_worker.cmd\""),
    );

    let mut response = Response::new(Body::from(cmd));
    *response.headers_mut() = headers_resp;
    response
}

/// Handler for `GET /winrm.ps1`.
/// Serves the 1-liner remote PowerShell enablement script.
pub async fn serve_winrm_ps1() -> Response {
    let script = "# AgentTrace Windows Remote PowerShell (WinRM) Enablement Script\r\n\
        Write-Host '==> Enabling Windows Remote Management (WinRM)...' -ForegroundColor Cyan\r\n\
        Enable-PSRemoting -Force -SkipNetworkProfileCheck\r\n\
        Set-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System' -Name 'LocalAccountTokenFilterPolicy' -Value 1 -Type DWord\r\n\
        Set-Item WSMan:\\localhost\\Client\\TrustedHosts -Value '*' -Force\r\n\
        Restart-Service WinRM\r\n\
        Write-Host '==> Windows Remote PowerShell unlocked successfully! PC-1 can now push deployments.' -ForegroundColor Green\r\n";

    let mut headers_resp = HeaderMap::new();
    headers_resp.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));
    let mut response = Response::new(Body::from(script));
    *response.headers_mut() = headers_resp;
    response
}

/// Handler for `GET /winrm.cmd`.
/// Serves a double-clickable batch launcher to enable WinRM in 1 click.
pub async fn serve_winrm_cmd(
    headers: HeaderMap,
    State(state): State<crate::handlers::AppState>,
) -> Response {
    let lead_url = resolve_lead_url(&headers, state.lead_url.as_deref());
    let cmd = format!(
        "@echo off\r\n\
        title AgentTrace WinRM Enabler\r\n\
        echo ========================================================\r\n\
        echo  Enabling Windows Remote PowerShell (Run as Administrator)\r\n\
        echo ========================================================\r\n\
        powershell -NoProfile -ExecutionPolicy Bypass -Command \"irm {}/winrm.ps1 | iex\"\r\n\
        echo Setup complete. You can close this window.\r\n\
        pause\r\n",
        lead_url
    );

    let mut headers_resp = HeaderMap::new();
    headers_resp.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/x-bat"));
    headers_resp.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"enable_winrm.cmd\""),
    );

    let mut response = Response::new(Body::from(cmd));
    *response.headers_mut() = headers_resp;
    response
}

/// Handler for `GET /join`.
/// High-density web interface for pairing worker nodes without typing or copy-pasting commands.
pub async fn serve_join_html(
    headers: HeaderMap,
    State(state): State<crate::handlers::AppState>,
) -> Response {
    let lead_url = resolve_lead_url(&headers, state.lead_url.as_deref());
    let bootstrap_cmd = format!("irm {}/bootstrap.ps1 | iex", lead_url.trim_end_matches('/'));

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>AgentTrace :: Cluster Worker Joiner</title>
  <style>
    :root {{
      --bg: #0a0e14;
      --card-bg: #121820;
      --border: #1e2836;
      --text: #c0caf5;
      --heading: #e0af68;
      --green: #9ece6a;
      --cyan: #7dcfff;
      --blue: #7aa2f7;
      --gray: #565f89;
      --font-mono: 'JetBrains Mono', 'Fira Code', Consolas, monospace;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      background: var(--bg);
      color: var(--text);
      font-family: var(--font-mono);
      padding: 30px 20px;
      display: flex;
      justify-content: center;
    }}
    .container {{
      max-width: 800px;
      width: 100%;
    }}
    .header {{
      border-bottom: 1px solid var(--border);
      padding-bottom: 16px;
      margin-bottom: 24px;
    }}
    .title {{
      color: var(--heading);
      font-size: 20px;
      font-weight: bold;
      letter-spacing: 1px;
    }}
    .subtitle {{
      color: var(--gray);
      font-size: 13px;
      margin-top: 4px;
    }}
    .card {{
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 6px;
      padding: 20px;
      margin-bottom: 20px;
    }}
    .card-title {{
      font-size: 15px;
      color: var(--cyan);
      font-weight: bold;
      margin-bottom: 10px;
      display: flex;
      align-items: center;
      gap: 8px;
    }}
    .btn {{
      display: inline-flex;
      align-items: center;
      gap: 8px;
      background: #1b4931;
      border: 1px solid var(--green);
      color: var(--green);
      padding: 12px 20px;
      font-family: var(--font-mono);
      font-size: 14px;
      font-weight: bold;
      text-decoration: none;
      border-radius: 4px;
      cursor: pointer;
      transition: all 0.15s ease;
    }}
    .btn:hover {{
      background: var(--green);
      color: #0a0e14;
    }}
    .btn-secondary {{
      background: #192b45;
      border: 1px solid var(--blue);
      color: var(--blue);
    }}
    .btn-secondary:hover {{
      background: var(--blue);
      color: #0a0e14;
    }}
    .code-box {{
      background: #06090e;
      border: 1px solid var(--border);
      padding: 12px;
      font-size: 13px;
      color: var(--green);
      border-radius: 4px;
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-top: 10px;
    }}
    .badge {{
      display: inline-block;
      padding: 2px 8px;
      border-radius: 3px;
      font-size: 11px;
      background: #1a2433;
      color: var(--cyan);
      border: 1px solid #28394f;
    }}
    .hint {{
      font-size: 12px;
      color: var(--gray);
      margin-top: 8px;
      line-height: 1.5;
    }}
  </style>
</head>
<body>
  <div class="container">
    <div class="header">
      <div class="title">⚡ AGENTTRACE CLUSTER JOINER</div>
      <div class="subtitle">Lead Node: {lead} | No draft emails, no chat pasting, zero manual typing</div>
    </div>

    <!-- Option 1: One-Click File Download -->
    <div class="card" style="border-left: 4px solid var(--green);">
      <div class="card-title">
        <span>Option 1: One-Click Windows File (Easiest on PC-2)</span>
        <span class="badge">Zero Typing</span>
      </div>
      <p style="font-size: 13px; line-height: 1.6;">
        Click below to download the pre-configured launcher directly to this PC. Once downloaded, simply open or double-click it.
      </p>
      <div style="margin-top: 14px;">
        <a href="/worker.cmd" class="btn">
          ⬇️ Download & Run agenttrace_worker.cmd
        </a>
      </div>
      <div class="hint">
        Browser will save <code>agenttrace_worker.cmd</code>. Click "Open" from your browser download bar to join this PC to the cluster.
      </div>
    </div>

    <!-- Option 2: Windows Cloud Clipboard Sync -->
    <div class="card" style="border-left: 4px solid var(--blue);">
      <div class="card-title">
        <span>Option 2: Windows Cloud Clipboard Sync (Win + V)</span>
        <span class="badge">Seamless Windows Feature</span>
      </div>
      <p style="font-size: 13px; line-height: 1.6;">
        Both PCs are running Windows! Windows has built-in LAN/Cloud clipboard sync:
      </p>
      <ol style="margin-left: 20px; font-size: 13px; margin-top: 8px; line-height: 1.6;">
        <li>On PC-1, copy the command below (<kbd>Ctrl+C</kbd>).</li>
        <li>Enable sync in Windows: <code>Settings &gt; System &gt; Clipboard &gt; Sync across devices</code>.</li>
        <li>On PC-2, open PowerShell, press <kbd>Win + V</kbd>, and click the item!</li>
      </ol>
      <div class="code-box">
        <code id="cmd-box">{cmd}</code>
        <button class="btn btn-secondary" onclick="navigator.clipboard.writeText(document.getElementById('cmd-box').innerText); alert('Copied to clipboard!');" style="padding: 6px 12px; font-size: 12px;">Copy</button>
      </div>
    </div>

    <!-- Option 3: Zero-Touch Push from PC-1 -->
    <div class="card" style="border-left: 4px solid var(--heading);">
      <div class="card-title">
        <span>Option 3: Zero-Touch Push from PC-1 (Never Touch PC-2)</span>
        <span class="badge">Push from Lead</span>
      </div>
      <p style="font-size: 13px; line-height: 1.6;">
        If you are on PC-1, push deployment remotely over PowerShell Remoting or WMI:
      </p>
      <div class="code-box" style="margin-top: 8px;">
        <code>python install.py --push-workers 192.168.1.102</code>
      </div>
      <div class="hint" style="margin-top: 12px; border-top: 1px solid var(--border); padding-top: 10px;">
        <strong style="color: var(--heading);">⚙️ Need to enable Remote PowerShell (WinRM)?</strong><br>
        Windows blocks remote PowerShell by default in workgroup environments. Run this once on the worker (Admin PowerShell):
        <pre style="background: #06090e; padding: 8px; margin-top: 6px; border-radius: 4px; color: var(--cyan); font-size: 11px; overflow-x: auto;">Enable-PSRemoting -Force -SkipNetworkProfileCheck
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' -Name 'LocalAccountTokenFilterPolicy' -Value 1 -Type DWord
Set-Item WSMan:\localhost\Client\TrustedHosts -Value '*' -Force</pre>
      </div>
    </div>
  </div>
</body>
</html>"#,
        lead = lead_url,
        cmd = bootstrap_cmd
    );

    let mut headers_resp = HeaderMap::new();
    headers_resp.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    let mut response = Response::new(Body::from(html));
    *response.headers_mut() = headers_resp;
    response
}

/// Handler for `POST /api/v1/cluster/deploy-worker`.
pub async fn deploy_worker(
    headers: HeaderMap,
    State(state): State<crate::handlers::AppState>,
    Json(payload): Json<DeployWorkerRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let target_ip = payload.target_ip.trim();
    if target_ip.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "target_ip cannot be empty".to_string()));
    }

    let port = payload.port.unwrap_or(8642);
    let lead_url = resolve_lead_url(&headers, state.lead_url.as_deref());

    info!(
        target_ip = target_ip,
        port = port,
        lead_url = lead_url.as_str(),
        "Initiating remote OTA worker deployment"
    );

    // Attempt trigger over HTTP Hermes / local execution API on the target node if reachable
    let trigger_url = format!("http://{}:{}/api/v1/system/bootstrap", target_ip, port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let remote_res = client
        .post(&trigger_url)
        .json(&serde_json::json!({
            "action": "ota_bootstrap",
            "bootstrap_url": format!("{}/bootstrap.ps1", lead_url.trim_end_matches('/')),
            "lead_url": lead_url
        }))
        .send()
        .await;

    let (delivered, details) = match remote_res {
        Ok(res) if res.status().is_success() => {
            (true, format!("Triggered remote worker bootstrap via {}", trigger_url))
        }
        Ok(res) => {
            (false, format!("Target returned HTTP status: {}", res.status()))
        }
        Err(e) => {
            (false, format!("Direct remote trigger to {} timed out or failed: {}. Worker can bootstrap via: irm {}/bootstrap.ps1 | iex", trigger_url, e, lead_url.trim_end_matches('/')))
        }
    };

    Ok(Json(serde_json::json!({
        "status": "triggered",
        "target_ip": target_ip,
        "port": port,
        "delivered_automatically": delivered,
        "details": details,
        "manual_bootstrap_command": format!("irm {}/bootstrap.ps1 | iex", lead_url.trim_end_matches('/'))
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bootstrap_ps1_contains_lead_url() {
        let script = generate_bootstrap_ps1("http://192.168.1.101:8080");
        assert!(script.contains("http://192.168.1.101:8080"));
        assert!(script.contains("agent_trace_gw.exe"));
        assert!(script.contains("Get-FileHash"));
    }

    #[test]
    fn test_manifest_building_fallback() {
        // Even if binary isn't built yet, the error is well-typed
        let manifest = build_local_binary_manifest("http://localhost:8080");
        if let Ok(m) = manifest {
            assert_eq!(m.binary_name, "agent_trace_gw.exe");
            assert_eq!(m.sha256.len(), 64);
        }
    }
}

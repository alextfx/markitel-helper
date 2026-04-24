//! HTTP client for the Markitel backend. All endpoints live under
//! `/api/v1/bridge/` and speak JSON.

use crate::config::{api_base, HELPER_VERSION};
use serde::{Deserialize, Serialize};
use std::time::Duration;

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(format!("MarkitelHelper/{HELPER_VERSION}"))
        .timeout(Duration::from_secs(15))
        .build()
        .expect("reqwest client build")
}

// ── /pair/exchange ─────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct PairExchangeReq<'a> {
    pub code: &'a str,
    pub device: DeviceInfo,
}

#[derive(Serialize)]
pub struct DeviceInfo {
    pub os: String,
    pub hostname: String,
    #[serde(rename = "helperVersion")]
    pub helper_version: String,
}

#[derive(Deserialize)]
pub struct PairExchangeResp {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "userEmail")]
    pub user_email: Option<String>,
    pub connection: ConnectionSummary,
}

#[derive(Deserialize, Clone)]
pub struct ConnectionSummary {
    pub id: String,
    #[serde(rename = "brokerName")]
    pub broker_name: String,
    pub platform: String,
    #[serde(rename = "apiKeyPrefix")]
    pub api_key_prefix: String,
    #[serde(rename = "isActive")]
    pub is_active: Option<bool>,
    #[serde(rename = "isLive")]
    pub is_live: Option<bool>,
}

pub async fn exchange_pairing_code(code: &str) -> Result<PairExchangeResp, String> {
    let device = DeviceInfo {
        os: std::env::consts::OS.to_string(),
        hostname: hostname().unwrap_or_default(),
        helper_version: HELPER_VERSION.to_string(),
    };

    let resp = client()
        .post(format!("{}/api/v1/bridge/pair/exchange", api_base()))
        .json(&PairExchangeReq { code, device })
        .send()
        .await
        .map_err(|e| format!("exchange request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("exchange failed ({status}): {body}"));
    }

    resp.json::<PairExchangeResp>()
        .await
        .map_err(|e| format!("exchange decode failed: {e}"))
}

// ── /rotate ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RotateResp {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub connection: ConnectionSummary,
}

pub async fn rotate_api_key(current_key: &str) -> Result<RotateResp, String> {
    let resp = client()
        .post(format!("{}/api/v1/bridge/rotate", api_base()))
        .header("X-Bridge-Key", current_key)
        .send()
        .await
        .map_err(|e| format!("rotate request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("rotate failed ({status}): {body}"));
    }

    resp.json::<RotateResp>()
        .await
        .map_err(|e| format!("rotate decode failed: {e}"))
}

// ── /ea-source ────────────────────────────────────────────────────────

pub struct EaSource {
    pub source: String,
    pub version: String,
}

pub async fn fetch_ea_source(cached_version: Option<&str>) -> Result<Option<EaSource>, String> {
    let mut url = format!("{}/api/v1/bridge/ea-source", api_base());
    if let Some(v) = cached_version {
        url.push_str(&format!("?version={v}"));
    }

    let resp = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("ea-source request failed: {e}"))?;

    if resp.status().as_u16() == 304 {
        return Ok(None);
    }

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("ea-source failed ({status}): {body}"));
    }

    let version = resp
        .headers()
        .get("x-ea-version")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let source = resp
        .text()
        .await
        .map_err(|e| format!("ea-source read failed: {e}"))?;

    Ok(Some(EaSource { source, version }))
}

// ── /helper-version ───────────────────────────────────────────────────

// `Serialize` is required because this struct is returned by a
// `#[tauri::command]` and therefore crosses the Rust→JS IPC boundary.
#[derive(Serialize, Deserialize, Clone)]
pub struct HelperVersionInfo {
    pub latest: String,
    #[serde(rename = "minSupported")]
    pub min_supported: Option<String>,
    #[serde(rename = "downloadUrls")]
    pub download_urls: DownloadUrls,
    #[serde(rename = "releaseNotes")]
    pub release_notes: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DownloadUrls {
    pub mac: Option<String>,
    pub windows: Option<String>,
}

pub async fn fetch_helper_version() -> Result<HelperVersionInfo, String> {
    let resp = client()
        .get(format!("{}/api/v1/bridge/helper-version", api_base()))
        .send()
        .await
        .map_err(|e| format!("helper-version request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!("helper-version failed ({status})"));
    }

    resp.json::<HelperVersionInfo>()
        .await
        .map_err(|e| format!("helper-version decode failed: {e}"))
}

// ── helper ────────────────────────────────────────────────────────────

fn hostname() -> Option<String> {
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    }
    #[cfg(windows)]
    {
        std::env::var("COMPUTERNAME").ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

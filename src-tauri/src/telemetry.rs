//! Fire-and-forget install-funnel events. Never blocks the main flow;
//! a network failure here silently swallows.
//!
//! Phases (see /api/v1/bridge/install-telemetry):
//!   pair-started | pair-exchanged | mt5-detected | mt5-not-found
//!   ea-written | ini-whitelisted | profile-written | mt5-launched
//!   first-heartbeat | error

use crate::config::{api_base, HELPER_VERSION};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct TelemetryEvent<'a> {
    phase: &'a str,
    platform: &'a str,
    #[serde(rename = "helperVersion")]
    helper_version: &'a str,
    #[serde(rename = "pairingCode", skip_serializing_if = "Option::is_none")]
    pairing_code: Option<&'a str>,
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Value>,
}

pub fn fire(phase: &str, pairing_code: Option<&str>, error: Option<&str>, metadata: Option<Value>) {
    let phase = phase.to_string();
    let pairing_code = pairing_code.map(String::from);
    let error = error.map(String::from);
    let metadata = metadata;

    tokio::spawn(async move {
        let event = TelemetryEvent {
            phase: &phase,
            platform: std::env::consts::OS,
            helper_version: HELPER_VERSION,
            pairing_code: pairing_code.as_deref(),
            error: error.as_deref(),
            metadata,
        };
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok();
        if let Some(c) = client {
            let _ = c
                .post(format!("{}/api/v1/bridge/install-telemetry", api_base()))
                .json(&event)
                .send()
                .await;
        }
    });
}

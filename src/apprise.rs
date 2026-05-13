use std::time::Duration;

use anyhow::Result;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::AppriseConfig;

/// 30-second timeout for all upstream Apprise requests.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Notification type matching Apprise API values.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotifyType {
    #[default]
    Info,
    Success,
    Warning,
    Failure,
}

impl NotifyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotifyType::Info => "info",
            NotifyType::Success => "success",
            NotifyType::Warning => "warning",
            NotifyType::Failure => "failure",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "info" => Some(NotifyType::Info),
            "success" => Some(NotifyType::Success),
            "warning" | "warn" => Some(NotifyType::Warning),
            "failure" | "fail" | "error" => Some(NotifyType::Failure),
            _ => None,
        }
    }
}

/// Thin HTTP client for the Apprise REST API.
#[derive(Clone)]
pub struct AppriseClient {
    client: Client,
    pub base_url: String,
}

impl AppriseClient {
    /// Build a new client. `token` is optional — pass empty string for open installs.
    pub fn new(config: &AppriseConfig) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        if !config.token.is_empty() {
            let val = header::HeaderValue::from_str(&format!("Bearer {}", config.token))
                .map_err(|e| anyhow::anyhow!("invalid token header value: {e}"))?;
            headers.insert(header::AUTHORIZATION, val);

            // Also send X-Apprise-API-Key for older Apprise versions
            let key_val = header::HeaderValue::from_str(&config.token)
                .map_err(|e| anyhow::anyhow!("invalid token header value: {e}"))?;
            headers.insert("X-Apprise-API-Key", key_val);
        }

        let client = Client::builder()
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {e}"))?;

        let base_url = config.url.trim_end_matches('/').to_string();

        Ok(Self { client, base_url })
    }

    /// POST /notify/{tag} — send to all services under a tag.
    pub async fn notify(
        &self,
        tag: &str,
        title: Option<&str>,
        body: &str,
        notify_type: &NotifyType,
    ) -> Result<Value> {
        let url = format!("{}/notify/{}", self.base_url, tag);
        tracing::debug!(url = %url, "upstream notify");
        self.post_notify(&url, None, title, body, notify_type).await
    }

    /// POST /notify — send to all configured services (no tag filter).
    pub async fn notify_all(
        &self,
        title: Option<&str>,
        body: &str,
        notify_type: &NotifyType,
    ) -> Result<Value> {
        let url = format!("{}/notify", self.base_url);
        tracing::debug!(url = %url, "upstream notify_all");
        self.post_notify(&url, None, title, body, notify_type).await
    }

    /// POST /notify/ (stateless) — one-off notification to an Apprise URL schema.
    pub async fn notify_url(
        &self,
        urls: &str,
        title: Option<&str>,
        body: &str,
        notify_type: &NotifyType,
    ) -> Result<Value> {
        let url = format!("{}/notify/", self.base_url);
        tracing::debug!(url = %url, "upstream notify_url");
        self.post_notify(&url, Some(urls), title, body, notify_type)
            .await
    }

    /// GET /health — server liveness check.
    pub async fn health(&self) -> Result<Value> {
        let url = format!("{}/health", self.base_url);
        tracing::debug!(url = %url, "upstream health check");
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("health request failed: {e}"))?;

        let status = resp.status();
        if status.is_success() {
            // Apprise /health may return plain text "OK" or JSON — handle both
            let text = resp.text().await.unwrap_or_else(|_| "ok".into());
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                Ok(v)
            } else {
                Ok(json!({ "status": text.trim() }))
            }
        } else {
            Err(anyhow::anyhow!(
                "health check failed: HTTP {}",
                status.as_u16()
            ))
        }
    }

    // ── internal ──────────────────────────────────────────────────────────────

    async fn post_notify(
        &self,
        url: &str,
        urls_field: Option<&str>,
        title: Option<&str>,
        body: &str,
        notify_type: &NotifyType,
    ) -> Result<Value> {
        let mut payload = json!({
            "body": body,
            "type": notify_type.as_str(),
        });

        if let Some(t) = title {
            payload["title"] = json!(t);
        }
        if let Some(u) = urls_field {
            payload["urls"] = json!(u);
        }

        let resp = self
            .client
            .post(url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("notify request failed: {e}"))?;

        let status = resp.status();
        if status.is_success() {
            // Apprise returns plain "OK" or empty body on success
            let text = resp.text().await.unwrap_or_default();
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                Ok(v)
            } else {
                Ok(json!({ "ok": true, "response": text.trim() }))
            }
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "notify failed: HTTP {} — {}",
                status.as_u16(),
                body.trim()
            ))
        }
    }
}

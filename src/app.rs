use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use crate::apprise::{AppriseClient, NotifyType};
use crate::config::McpConfig;
use crate::observability::{Counters, ServerClock};
use crate::token_limit::{truncate_body, MAX_BODY_BYTES};

/// Business logic layer. CLI and MCP are thin shims that call into this.
#[derive(Clone)]
pub struct AppriseService {
    client: AppriseClient,
    apprise_url: String,
    config: Option<McpConfig>,
    pub counters: Arc<Counters>,
    pub clock: Arc<ServerClock>,
}

impl AppriseService {
    pub fn new(client: AppriseClient, apprise_url: String) -> Self {
        Self {
            client,
            apprise_url,
            config: None,
            counters: Arc::new(Counters::default()),
            clock: Arc::new(ServerClock::new()),
        }
    }

    pub fn with_config(mut self, config: McpConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_counters(mut self, counters: Arc<Counters>) -> Self {
        self.counters = counters;
        self
    }

    pub fn with_clock(mut self, clock: Arc<ServerClock>) -> Self {
        self.clock = clock;
        self
    }

    /// Send notification to all services configured under `tag`.
    pub async fn notify(
        &self,
        tag: &str,
        title: Option<&str>,
        body: &str,
        notify_type: &NotifyType,
    ) -> Result<Value> {
        let (body, truncation_warn) = truncate_body(body);
        let _span = tracing::info_span!("upstream.notify", tag = %tag);
        let _guard = _span.enter();
        drop(_guard);
        self.counters.inc_upstream_calls();

        let result = self
            .client
            .notify(tag, title, &body, notify_type)
            .await
            .map_err(|e| self.enrich_error(e, tag));

        match &result {
            Ok(_) => tracing::debug!(tag = %tag, "notify ok"),
            Err(e) => {
                self.counters.inc_upstream_errors();
                tracing::warn!(tag = %tag, error = %e, "notify failed");
            }
        }

        let mut val = result?;
        if let Some(warn) = truncation_warn {
            val["body_truncation_warning"] = json!(warn);
        }
        Ok(val)
    }

    /// Send notification to all configured services (no tag filter).
    pub async fn notify_all(
        &self,
        title: Option<&str>,
        body: &str,
        notify_type: &NotifyType,
    ) -> Result<Value> {
        let (body, truncation_warn) = truncate_body(body);
        let _span = tracing::info_span!("upstream.notify_all");
        let _guard = _span.enter();
        drop(_guard);
        self.counters.inc_upstream_calls();

        let result = self
            .client
            .notify_all(title, &body, notify_type)
            .await
            .map_err(|e| self.enrich_error(e, "<all>"));

        match &result {
            Ok(_) => tracing::debug!("notify_all ok"),
            Err(e) => {
                self.counters.inc_upstream_errors();
                tracing::warn!(error = %e, "notify_all failed");
            }
        }

        let mut val = result?;
        if let Some(warn) = truncation_warn {
            val["body_truncation_warning"] = json!(warn);
        }
        Ok(val)
    }

    /// Stateless one-off notification to an Apprise URL schema string.
    pub async fn notify_url(
        &self,
        urls: &str,
        title: Option<&str>,
        body: &str,
        notify_type: &NotifyType,
    ) -> Result<Value> {
        let (body, truncation_warn) = truncate_body(body);
        let _span = tracing::info_span!("upstream.notify_url");
        let _guard = _span.enter();
        drop(_guard);
        self.counters.inc_upstream_calls();

        let result = self
            .client
            .notify_url(urls, title, &body, notify_type)
            .await
            .map_err(|e| self.enrich_error(e, "<url>"));

        match &result {
            Ok(_) => tracing::debug!("notify_url ok"),
            Err(e) => {
                self.counters.inc_upstream_errors();
                tracing::warn!(error = %e, "notify_url failed");
            }
        }

        let mut val = result?;
        if let Some(warn) = truncation_warn {
            val["body_truncation_warning"] = json!(warn);
        }
        Ok(val)
    }

    /// Health-check the Apprise server.
    pub async fn health(&self) -> Result<Value> {
        let _span = tracing::info_span!("upstream.health");
        let _guard = _span.enter();
        drop(_guard);
        self.counters.inc_upstream_calls();

        let result = self.client.health().await.map_err(|e| {
            self.counters.inc_upstream_errors();
            self.enrich_connection_error(e)
        });

        match &result {
            Ok(_) => tracing::debug!("health ok"),
            Err(e) => tracing::warn!(error = %e, "health check failed"),
        }

        result
    }

    /// Return the full runtime status (counters, config, uptime).
    /// Always succeeds even when the upstream is unreachable.
    pub fn status(&self) -> Value {
        let snap = self.counters.snapshot();
        let mut out = json!({
            "status": "ok",
            "server": {
                "version": env!("CARGO_PKG_VERSION"),
                "uptime_secs": self.clock.uptime_secs(),
                "pid": std::process::id(),
                "data_dir": crate::config::default_data_dir(),
            },
            "counters": {
                "requests_total":  snap.requests_total,
                "errors_total":    snap.errors_total,
                "upstream_calls":  snap.upstream_calls,
                "upstream_errors": snap.upstream_errors,
            },
            "upstream": {
                "url": self.apprise_url,
            },
        });

        if let Some(cfg) = &self.config {
            out["config"] = json!({
                "host": cfg.host,
                "port": cfg.port,
                "server_name": cfg.server_name,
            });
        }

        out
    }

    // ── error enrichment ──────────────────────────────────────────────────────

    fn enrich_error(&self, e: anyhow::Error, tag: &str) -> anyhow::Error {
        let msg = e.to_string();
        if is_connection_error(&msg) {
            return self.enrich_connection_error(e);
        }
        if msg.contains("404") || msg.contains("Not Found") {
            return anyhow::anyhow!(
                "{e}\n\
                 Hint: tag '{tag}' has no configured services — \
                 check Apprise config at {url}\n\
                 Hint: use action=health to verify the server is reachable",
                url = self.apprise_url
            );
        }
        if msg.contains("401")
            || msg.contains("403")
            || msg.contains("Unauthorized")
            || msg.contains("Forbidden")
        {
            return anyhow::anyhow!(
                "APPRISE_TOKEN rejected — check the token matches your Apprise server config\n\
                 URL: {url}",
                url = self.apprise_url
            );
        }
        e
    }

    fn enrich_connection_error(&self, e: anyhow::Error) -> anyhow::Error {
        anyhow::anyhow!(
            "Apprise server at {url} unreachable — is it running?\n\
             Detail: {e}\n\
             Hint: use action=health to check connectivity",
            url = self.apprise_url
        )
    }
}

fn is_connection_error(msg: &str) -> bool {
    msg.contains("connection refused")
        || msg.contains("unreachable")
        || msg.contains("timed out")
        || msg.contains("timeout")
        || msg.contains("dns error")
        || msg.contains("failed to connect")
        || msg.contains("No such host")
}

// Re-export so tools.rs / routes.rs can call truncate_response on outputs.

// Silence unused import warning — MAX_BODY_BYTES is used indirectly via truncate_body.
const _: usize = MAX_BODY_BYTES;

/// Timeout for all upstream Apprise requests.
pub const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(30);

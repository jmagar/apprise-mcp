use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Atomic counters for MCP tool call observability.
///
/// Lives on [`crate::mcp::AppState`] and is incremented by the tool dispatcher
/// and the API client respectively.
#[derive(Debug, Default)]
pub struct Counters {
    pub requests_total: AtomicU64,
    pub errors_total: AtomicU64,
    pub upstream_calls: AtomicU64,
    pub upstream_errors: AtomicU64,
}

impl Counters {
    pub fn inc_requests(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_errors(&self) {
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_upstream_calls(&self) {
        self.upstream_calls.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_upstream_errors(&self) {
        self.upstream_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> CounterSnapshot {
        CounterSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            errors_total: self.errors_total.load(Ordering::Relaxed),
            upstream_calls: self.upstream_calls.load(Ordering::Relaxed),
            upstream_errors: self.upstream_errors.load(Ordering::Relaxed),
        }
    }
}

/// Point-in-time snapshot of [`Counters`] — serialization-safe (no atomics).
#[derive(Debug, serde::Serialize)]
pub struct CounterSnapshot {
    pub requests_total: u64,
    pub errors_total: u64,
    pub upstream_calls: u64,
    pub upstream_errors: u64,
}

/// Server startup time, used to compute `uptime_secs` in health/status responses.
#[derive(Debug)]
pub struct ServerClock {
    started_at: Instant,
}

impl ServerClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }

    #[must_use]
    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }

    #[must_use]
    pub fn uptime_secs(&self) -> u64 {
        self.uptime().as_secs()
    }
}

impl Default for ServerClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Probe the upstream Apprise health endpoint and report reachability.
///
/// Returns `(reachable, latency_ms)`. Never panics — errors are swallowed and
/// reported as unreachable.
pub async fn probe_upstream(client: &reqwest::Client, base_url: &str) -> (bool, u64) {
    let url = format!("{}/health", base_url.trim_end_matches('/'));
    let start = Instant::now();
    match client
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => (true, start.elapsed().as_millis() as u64),
        _ => (false, start.elapsed().as_millis() as u64),
    }
}

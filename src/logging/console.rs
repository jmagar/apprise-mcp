use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{Level, Subscriber};
use tracing_subscriber::fmt::{format::Writer, FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

use super::aurora;

/// Custom console formatter using the aurora colour palette.
///
/// Format: `2026-05-13T14:32:01Z  INFO  <message>  key=value ...`
pub struct AuroraFormatter {
    pub colorize: bool,
}

impl<S, N> FormatEvent<S, N> for AuroraFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> fmt::Result {
        // Timestamp (ISO 8601 UTC, no external dep)
        let ts = utc_timestamp_now();
        let ts_str = aurora::paint(aurora::TEXT_MUTED, &ts, self.colorize);
        write!(writer, "{ts_str}  ")?;

        // Level
        let level = *event.metadata().level();
        let level_str = level_label(level, self.colorize);
        write!(writer, "{level_str}  ")?;

        // Message + fields
        ctx.format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

fn level_label(level: Level, colorize: bool) -> String {
    match level {
        Level::ERROR => aurora::bold(&aurora::paint(aurora::ERROR, "ERROR", colorize), colorize),
        Level::WARN => aurora::bold(&aurora::paint(aurora::WARN, "WARN ", colorize), colorize),
        Level::INFO => "INFO ".to_string(),
        Level::DEBUG => aurora::dim("DEBUG", colorize),
        Level::TRACE => aurora::dim("TRACE", colorize),
    }
}

/// Format current UTC time as `YYYY-MM-DDTHH:MM:SSZ` without an external crate.
fn utc_timestamp_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_utc(secs)
}

fn format_utc(secs: u64) -> String {
    // Simple UTC formatter — no leap seconds, good enough for log timestamps.
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;

    // Gregorian calendar algorithm (days since 1970-01-01)
    let (year, month, day) = days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Adapted from Howard Hinnant's civil-from-days algorithm.
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

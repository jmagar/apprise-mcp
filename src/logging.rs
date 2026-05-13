pub mod aurora;
pub mod console;
pub mod file;

use std::io::IsTerminal;
use std::path::Path;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use self::console::AuroraFormatter;
use self::file::{RollingFileWriter, MAX_LOG_BYTES};

/// Initialise dual logging: pretty aurora console (stderr) + JSON file.
///
/// Log file location: `{data_dir}/logs/apprise.log`
pub fn init(data_dir: &Path, default_level: &str) -> anyhow::Result<()> {
    let log_path = data_dir.join("logs").join("apprise.log");
    let colorize = should_colorize();
    let file_writer = RollingFileWriter::open(log_path, MAX_LOG_BYTES)?;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level.to_string()));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            // Console: pretty, aurora-coloured, human-readable
            tracing_subscriber::fmt::layer()
                .with_ansi(colorize)
                .with_writer(std::io::stderr)
                .event_format(AuroraFormatter { colorize }),
        )
        .with(
            // File: structured JSON, no ANSI
            tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(file_writer),
        )
        .init();

    Ok(())
}

/// Initialise console-only logging (no file). Used in stdio/CLI modes where a
/// data dir may not be available at startup.
pub fn init_console(default_level: &str) {
    let colorize = should_colorize();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level.to_string()));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(colorize)
                .with_writer(std::io::stderr)
                .event_format(AuroraFormatter { colorize }),
        )
        .try_init();
}

/// Determine whether to emit ANSI colours on stderr.
///
/// Rules (in priority order):
/// 1. `NO_COLOR` set → no colour
/// 2. `FORCE_COLOR` set → colour
/// 3. stderr is a TTY → colour
fn should_colorize() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var_os("FORCE_COLOR").is_some() {
        return true;
    }
    std::io::stderr().is_terminal()
}

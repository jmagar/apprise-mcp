use anyhow::{bail, Result};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Instant;

use apprise_mcp::{
    app::AppriseService,
    apprise::NotifyType,
    config::{AuthMode, Config},
};

// ── command enum ──────────────────────────────────────────────────────────────

pub enum CliCommand {
    Setup(SetupCommand),
    Notify {
        body: String,
        tag: Option<String>,
        title: Option<String>,
        notify_type: NotifyType,
    },
    NotifyUrl {
        urls: String,
        body: String,
        title: Option<String>,
        notify_type: NotifyType,
    },
    Health,
    Doctor,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupCommand {
    Check,
    Repair,
    PluginHook { no_repair: bool },
}

impl CliCommand {
    pub fn parse(args: &[String]) -> Result<Self> {
        let rest: Vec<&str> = args.iter().map(String::as_str).collect();

        match rest.as_slice() {
            ["health"] => Ok(Self::Health),
            ["doctor"] => Ok(Self::Doctor),
            ["help"] => Ok(Self::Help),
            ["setup", "check"] => Ok(Self::Setup(SetupCommand::Check)),
            ["setup", "repair"] => Ok(Self::Setup(SetupCommand::Repair)),
            ["setup", "plugin-hook", flags @ ..] => Ok(Self::Setup(SetupCommand::PluginHook {
                no_repair: flags.contains(&"--no-repair"),
            })),

            ["notify", body, rest @ ..] => {
                let tag = flag_str(rest, "--tag")?;
                let title = flag_str(rest, "--title")?;
                let notify_type = parse_type_flag(rest)?;
                Ok(Self::Notify {
                    body: body.to_string(),
                    tag,
                    title,
                    notify_type,
                })
            }

            ["notify-url", urls, body, rest @ ..] => {
                let title = flag_str(rest, "--title")?;
                let notify_type = parse_type_flag(rest)?;
                Ok(Self::NotifyUrl {
                    urls: urls.to_string(),
                    body: body.to_string(),
                    title,
                    notify_type,
                })
            }

            _ => bail!(
                "Unknown command. Run with --help for usage.\n\n\
                 Commands:\n\
                   notify <body> [--tag TAG] [--title T] [--type info|success|warning|failure]\n\
                   notify-url <urls> <body> [--title T] [--type ...]\n\
                   health\n\
                   help"
            ),
        }
    }
}

pub async fn run(service: &AppriseService, cmd: CliCommand, json: bool) -> Result<()> {
    if let CliCommand::Help = cmd {
        print!("{}", HELP_TEXT);
        return Ok(());
    }

    if let CliCommand::Doctor = cmd {
        let config = Config::load()?;
        run_doctor(&config, json).await?;
        return Ok(());
    }

    let result = match cmd {
        CliCommand::Health => service.health().await?,
        CliCommand::Help | CliCommand::Doctor | CliCommand::Setup(_) => unreachable!(),
        CliCommand::Notify {
            body,
            tag,
            title,
            notify_type,
        } => match tag.as_deref() {
            Some(t) => {
                service
                    .notify(t, title.as_deref(), &body, &notify_type)
                    .await?
            }
            None => {
                service
                    .notify_all(title.as_deref(), &body, &notify_type)
                    .await?
            }
        },
        CliCommand::NotifyUrl {
            urls,
            body,
            title,
            notify_type,
        } => {
            service
                .notify_url(&urls, title.as_deref(), &body, &notify_type)
                .await?
        }
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        // Pretty-print key fields for human consumption
        if let Some(obj) = result.as_object() {
            for (k, v) in obj {
                let owned;
                let val = if let Some(s) = v.as_str() {
                    s
                } else {
                    owned = v.to_string();
                    &owned
                };
                println!("{k}: {val}");
            }
        } else {
            println!("{result}");
        }
    }

    Ok(())
}

// ── setup command ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct SetupFailure {
    code: &'static str,
    message: String,
}

#[derive(Debug, serde::Serialize)]
struct SetupReport {
    exit_policy: &'static str,
    ran_repair: bool,
    no_repair: bool,
    blocking_failures: Vec<SetupFailure>,
    advisory_failures: Vec<SetupFailure>,
}

impl SetupReport {
    fn new(no_repair: bool) -> Self {
        Self {
            exit_policy: "success",
            ran_repair: false,
            no_repair,
            blocking_failures: Vec::new(),
            advisory_failures: Vec::new(),
        }
    }

    fn finish(mut self) -> Self {
        self.exit_policy = if !self.blocking_failures.is_empty() {
            "blocking_failure"
        } else if !self.advisory_failures.is_empty() {
            "advisory_failure"
        } else {
            "success"
        };
        self
    }
}

pub async fn run_setup(config: &Config, command: SetupCommand) -> Result<()> {
    let report = match command {
        SetupCommand::Check => setup_check(config, true),
        SetupCommand::Repair => setup_repair(config)?,
        SetupCommand::PluginHook { no_repair } => setup_plugin_hook(config, no_repair)?,
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.blocking_failures.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

fn setup_plugin_hook(config: &Config, no_repair: bool) -> Result<SetupReport> {
    let initial = setup_check(config, no_repair);
    if initial.blocking_failures.is_empty() || no_repair {
        return Ok(initial);
    }
    setup_repair(config)
}

fn setup_check(config: &Config, no_repair: bool) -> SetupReport {
    let mut report = SetupReport::new(no_repair);
    let data_dir = setup_data_dir();

    if !data_dir.is_dir() {
        report.blocking_failures.push(SetupFailure {
            code: "appdata_missing",
            message: format!("appdata directory does not exist: {}", data_dir.display()),
        });
    }
    let env_path = data_dir.join(".env");
    if !env_path.is_file() {
        report.advisory_failures.push(SetupFailure {
            code: "env_file_missing",
            message: format!(
                "{} does not exist; setup repair will create one, but process env can supply values",
                env_path.display()
            ),
        });
    }

    if config.apprise.url.is_empty() {
        report.blocking_failures.push(SetupFailure {
            code: "missing_apprise_url",
            message: "APPRISE_URL is required".into(),
        });
    }

    validate_setup_auth(config, &mut report);
    check_setup_port(config.mcp.port, &mut report);

    report.finish()
}

fn setup_repair(config: &Config) -> Result<SetupReport> {
    let data_dir = setup_data_dir();
    std::fs::create_dir_all(&data_dir)?;
    write_setup_env(&data_dir, config)?;

    let mut report = setup_check(config, false);
    report.ran_repair = true;
    if report
        .blocking_failures
        .iter()
        .any(|failure| failure.code == "appdata_missing")
    {
        report = setup_check(config, false);
        report.ran_repair = true;
    }

    Ok(report.finish())
}

fn validate_setup_auth(config: &Config, report: &mut SetupReport) {
    if config.mcp.no_auth {
        return;
    }

    if config.mcp.auth.mode == AuthMode::OAuth {
        if config
            .mcp
            .auth
            .public_url
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_public_url",
                message: "APPRISE_MCP_PUBLIC_URL is required for OAuth mode".into(),
            });
        }
        if config
            .mcp
            .auth
            .google_client_id
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_client_id",
                message: "APPRISE_MCP_GOOGLE_CLIENT_ID is required for OAuth mode".into(),
            });
        }
        if config
            .mcp
            .auth
            .google_client_secret
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_client_secret",
                message: "APPRISE_MCP_GOOGLE_CLIENT_SECRET is required for OAuth mode".into(),
            });
        }
        if config.mcp.auth.admin_email.is_empty() {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_admin_email",
                message: "APPRISE_MCP_AUTH_ADMIN_EMAIL is required for OAuth mode".into(),
            });
        }
    } else if config.mcp.api_token.as_deref().unwrap_or("").is_empty() {
        report.blocking_failures.push(SetupFailure {
            code: "missing_mcp_token",
            message: "APPRISE_MCP_TOKEN is required unless no_auth or OAuth mode is enabled".into(),
        });
    }
}

fn check_setup_port(port: u16, report: &mut SetupReport) {
    if TcpListener::bind(("127.0.0.1", port)).is_err() {
        report.advisory_failures.push(SetupFailure {
            code: "mcp_port_in_use",
            message: format!("MCP port {port} is already in use"),
        });
    }
}

fn setup_data_dir() -> PathBuf {
    std::env::var_os("CLAUDE_PLUGIN_DATA")
        .or_else(|| std::env::var_os("APPRISE_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(apprise_mcp::config::default_data_dir)
}

fn write_setup_env(data_dir: &Path, config: &Config) -> Result<()> {
    let mut lines = vec![
        format!("APPRISE_URL={}", config.apprise.url),
        format!("APPRISE_MCP_HOST={}", config.mcp.host),
        format!("APPRISE_MCP_PORT={}", config.mcp.port),
        format!("APPRISE_MCP_NO_AUTH={}", config.mcp.no_auth),
    ];

    if !config.apprise.token.is_empty() {
        lines.push(format!("APPRISE_TOKEN={}", config.apprise.token));
    }
    if let Some(token) = config
        .mcp
        .api_token
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("APPRISE_MCP_TOKEN={token}"));
    }
    if config.mcp.auth.mode == AuthMode::OAuth {
        lines.push("APPRISE_MCP_AUTH_MODE=oauth".into());
        if let Some(value) = &config.mcp.auth.public_url {
            lines.push(format!("APPRISE_MCP_PUBLIC_URL={value}"));
        }
        if let Some(value) = &config.mcp.auth.google_client_id {
            lines.push(format!("APPRISE_MCP_GOOGLE_CLIENT_ID={value}"));
        }
        if let Some(value) = &config.mcp.auth.google_client_secret {
            lines.push(format!("APPRISE_MCP_GOOGLE_CLIENT_SECRET={value}"));
        }
        if !config.mcp.auth.admin_email.is_empty() {
            lines.push(format!(
                "APPRISE_MCP_AUTH_ADMIN_EMAIL={}",
                config.mcp.auth.admin_email
            ));
        }
    }

    std::fs::write(data_dir.join(".env"), format!("{}\n", lines.join("\n")))?;
    Ok(())
}

// ── help text ─────────────────────────────────────────────────────────────────

const HELP_TEXT: &str = "\
apprise-mcp — Apprise universal push notification bridge

Commands:
  notify <body> [--tag TAG] [--title T] [--type info|success|warning|failure]
                      Send a notification to all services (or to one tag).
  notify-url <urls> <body> [--title T] [--type ...]
                      Stateless one-off notification to an Apprise URL schema.
  health              Check the Apprise API server health endpoint.
  doctor              Pre-flight environment validation.
  setup check         Check plugin setup without mutating appdata.
  setup repair        Create missing appdata/env setup files.
  setup plugin-hook [--no-repair]
                      Plugin hook JSON setup contract.
  help                Show this help text.

Options:
  --json              Output raw JSON instead of pretty-printed fields.
  --help, -h          Show usage (full environment variable reference).
  --version, -V       Print version.

Notification types:
  info (default)  success  warning  failure

Note: apprise-mcp connects to the Apprise API Server (not the Python library directly).
      Set APPRISE_URL to your running Apprise API server (e.g. http://localhost:8000).
";

// ── flag helpers ──────────────────────────────────────────────────────────────

fn flag_str(args: &[&str], flag: &str) -> Result<Option<String>> {
    for window in args.windows(2) {
        if window[0] == flag {
            return Ok(Some(window[1].to_string()));
        }
    }
    Ok(None)
}

fn parse_type_flag(args: &[&str]) -> Result<NotifyType> {
    match flag_str(args, "--type")? {
        None => Ok(NotifyType::default()),
        Some(s) => NotifyType::from_str_opt(&s).ok_or_else(|| {
            anyhow::anyhow!("--type must be info|success|warning|failure, got {s:?}")
        }),
    }
}

// ── doctor command ────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct DoctorCheck {
    category: &'static str,
    name: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms: Option<u64>,
}

impl DoctorCheck {
    fn pass(category: &'static str, name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            category,
            name: name.into(),
            ok: true,
            value: Some(value.into()),
            hint: None,
            latency_ms: None,
        }
    }

    fn warn(category: &'static str, name: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            category,
            name: name.into(),
            ok: false,
            value: None,
            hint: Some(hint.into()),
            latency_ms: None,
        }
    }
}

pub async fn run_doctor(config: &Config, json: bool) -> Result<()> {
    let mut checks: Vec<DoctorCheck> = Vec::new();

    // ── 1. Config ─────────────────────────────────────────────────────────────
    let data_dir = apprise_mcp::config::default_data_dir();

    // Config file
    let config_path = data_dir.join("config.toml");
    if config_path.exists() {
        checks.push(DoctorCheck::pass(
            "config",
            "Config file",
            format!("~/.apprise/config.toml"),
        ));
    } else {
        checks.push(DoctorCheck::warn(
            "config",
            "Config file",
            format!("~/.apprise/config.toml not found — create it or rely on env vars"),
        ));
    }

    // Data directory writable
    {
        let writable = std::fs::create_dir_all(&data_dir)
            .ok()
            .and_then(|_| {
                let test = data_dir.join(".write_test");
                std::fs::write(&test, b"").ok()?;
                std::fs::remove_file(&test).ok()?;
                Some(())
            })
            .is_some();
        if writable {
            checks.push(DoctorCheck::pass(
                "config",
                "Data directory",
                "~/.apprise/ (writable)",
            ));
        } else {
            checks.push(DoctorCheck::warn(
                "config",
                "Data directory",
                "~/.apprise/ is not writable — check permissions",
            ));
        }
    }

    // Log directory
    {
        let log_dir = data_dir.join("logs");
        let size_mb = dir_size_mb(&log_dir);
        let writable = std::fs::create_dir_all(&log_dir)
            .ok()
            .and_then(|_| {
                let test = log_dir.join(".write_test");
                std::fs::write(&test, b"").ok()?;
                std::fs::remove_file(&test).ok()?;
                Some(())
            })
            .is_some();
        if writable {
            let val = match size_mb {
                Some(mb) => format!("~/.apprise/logs/ (writable, {mb:.1} MB)"),
                None => "~/.apprise/logs/ (writable)".into(),
            };
            checks.push(DoctorCheck::pass("config", "Log directory", val));
        } else {
            checks.push(DoctorCheck::warn(
                "config",
                "Log directory",
                "~/.apprise/logs/ is not writable — check permissions",
            ));
        }
    }

    // Binary in PATH
    {
        match which_binary("apprise") {
            Some(path) => {
                checks.push(DoctorCheck::pass("config", "Binary in PATH", path));
            }
            None => {
                checks.push(DoctorCheck::warn(
                    "config",
                    "Binary in PATH",
                    "apprise not found in PATH — add ~/.local/bin to PATH",
                ));
            }
        }
    }

    // ── 2. Service credentials ────────────────────────────────────────────────

    // APPRISE_URL (REQUIRED)
    let apprise_url = std::env::var("APPRISE_URL")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| {
            if !config.apprise.url.is_empty() && config.apprise.url != "http://localhost:8000" {
                Some(config.apprise.url.clone())
            } else if !config.apprise.url.is_empty() {
                Some(config.apprise.url.clone())
            } else {
                None
            }
        });

    match &apprise_url {
        Some(url) => {
            checks.push(DoctorCheck::pass(
                "credentials",
                "APPRISE_URL",
                format!("{url} (set)"),
            ));
        }
        None => {
            checks.push(DoctorCheck::warn(
                "credentials",
                "APPRISE_URL",
                "not set — REQUIRED: set APPRISE_URL to your Apprise API Server URL (e.g. http://localhost:8000)",
            ));
        }
    }

    // APPRISE_TOKEN (optional — warn if server may require auth)
    {
        let token_set = std::env::var("APPRISE_TOKEN")
            .ok()
            .filter(|v| !v.is_empty())
            .is_some()
            || !config.apprise.token.is_empty();

        if token_set {
            checks.push(DoctorCheck::pass("credentials", "APPRISE_TOKEN", "set"));
        } else {
            // Optional — warn but don't fail
            let mut c = DoctorCheck::warn(
                "credentials",
                "APPRISE_TOKEN",
                "not set — optional, but required if your Apprise API server has auth enabled",
            );
            c.ok = true; // treat as warning, not failure
            checks.push(c);
        }
    }

    // ── 3. Connectivity ───────────────────────────────────────────────────────
    if let Some(ref url) = apprise_url {
        let health_url = format!("{}/health", url.trim_end_matches('/'));
        let start = Instant::now();
        let result = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()
            .map(|c| {
                let health_url = health_url.clone();
                async move { c.get(&health_url).send().await }
            });

        let connectivity = if let Some(fut) = result {
            match fut.await {
                Ok(resp) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let status = resp.status();
                    let mut c = DoctorCheck::pass(
                        "connectivity",
                        "Upstream reachable",
                        format!("{health_url} → {status} ({elapsed} ms)"),
                    );
                    c.latency_ms = Some(elapsed);
                    if !status.is_success() {
                        c.ok = false;
                        c.hint = Some(format!(
                            "Apprise API server returned {status} — check APPRISE_URL and server status"
                        ));
                        c.value = None;
                    }
                    c
                }
                Err(e) => DoctorCheck::warn(
                    "connectivity",
                    "Upstream reachable",
                    format!("could not reach {health_url}: {e}"),
                ),
            }
        } else {
            DoctorCheck::warn(
                "connectivity",
                "Upstream reachable",
                "could not build HTTP client",
            )
        };
        checks.push(connectivity);
    } else {
        checks.push(DoctorCheck::warn(
            "connectivity",
            "Upstream reachable",
            "skipped — APPRISE_URL not set",
        ));
    }

    // ── 4. MCP port ───────────────────────────────────────────────────────────
    {
        let port = config.mcp.port;
        let available = TcpListener::bind(format!("127.0.0.1:{port}")).is_ok();
        if available {
            checks.push(DoctorCheck::pass(
                "mcp",
                format!("MCP port {port}"),
                "available",
            ));
        } else {
            checks.push(DoctorCheck::warn(
                "mcp",
                format!("MCP port {port}"),
                format!("port {port} already in use — change APPRISE_MCP_PORT if needed"),
            ));
        }
    }

    // ── 5. Apprise API note ───────────────────────────────────────────────────
    {
        let mut note = DoctorCheck::pass(
            "mcp",
            "Apprise API mode",
            "connects to Apprise API Server (not the Python library directly)",
        );
        note.ok = true;
        checks.push(note);
    }

    // ── Output ────────────────────────────────────────────────────────────────
    let issues = checks.iter().filter(|c| !c.ok).count();

    if json {
        println!("{}", serde_json::to_string_pretty(&checks)?);
    } else {
        print_doctor_report(&checks);
    }

    if issues > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn print_doctor_report(checks: &[DoctorCheck]) {
    let version = env!("CARGO_PKG_VERSION");
    println!();
    println!("apprise-mcp v{version} — environment check");
    println!();

    let categories = [
        ("config", "Config"),
        ("credentials", "Service credentials"),
        ("connectivity", "Connectivity"),
        ("mcp", "MCP server"),
    ];

    for (key, label) in &categories {
        let section: Vec<&DoctorCheck> = checks.iter().filter(|c| c.category == *key).collect();
        if section.is_empty() {
            continue;
        }
        println!("  {label}");
        println!("  {}", "─".repeat(44));
        for c in &section {
            let icon = if c.ok { "✓" } else { "✗" };
            let name = &c.name;
            if c.ok {
                let val = c.value.as_deref().unwrap_or("ok");
                println!("  {icon} {name:<20} {val}");
            } else {
                println!("  {icon} {name}");
                if let Some(hint) = &c.hint {
                    println!("    → {hint}");
                }
            }
        }
        println!();
    }

    let issues = checks.iter().filter(|c| !c.ok).count();
    println!("  {}", "━".repeat(44));
    if issues == 0 {
        println!("  All checks passed.");
    } else {
        println!("  {issues} issue(s) found. Fix them before running: apprise serve");
    }
    println!();
}

fn which_binary(name: &str) -> Option<String> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path).find_map(|dir| {
            let candidate = dir.join(name);
            if candidate.is_file() {
                candidate.to_str().map(|s| s.to_string())
            } else {
                None
            }
        })
    })
}

fn dir_size_mb(path: &std::path::Path) -> Option<f64> {
    let mut total: u64 = 0;
    for entry in std::fs::read_dir(path).ok()?.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total += meta.len();
            }
        }
    }
    Some(total as f64 / (1024.0 * 1024.0))
}

use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use apprise_mcp::{
    app::AppriseService,
    apprise::AppriseClient,
    config::{AuthMode, Config},
    mcp::{self, AppState, AuthPolicy},
};

mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.as_slice() {
        [f] if matches!(f.as_str(), "--help" | "-h" | "help") => {
            print_usage();
            return Ok(());
        }
        [f] if matches!(f.as_str(), "--version" | "-V" | "version") => {
            println!("apprise-mcp {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        _ => {}
    }

    let stdio_mode = matches!(args.as_slice(), [c] if c == "mcp");
    let serve_mode = args.is_empty()
        || matches!(args.as_slice(), [c] if c == "serve")
        || matches!(args.as_slice(), [a, b] if a == "serve" && b == "mcp");

    let log_level = if stdio_mode || !serve_mode {
        "warn"
    } else {
        "info"
    };
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)),
        )
        .with_writer(std::io::stderr)
        .with_target(true)
        .init();

    if serve_mode {
        serve_mcp().await
    } else if stdio_mode {
        serve_stdio_mcp().await
    } else {
        run_cli(args).await
    }
}

async fn serve_mcp() -> Result<()> {
    let config = Config::load()?;
    let state = build_state(config).await?;

    info!(
        bind = %state.config.bind_addr(),
        server_name = %state.config.server_name,
        auth = ?state.auth_policy,
        "apprise-mcp starting"
    );

    let bind = state.config.bind_addr();
    let app = mcp::router(state).layer(tower_http::trace::TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!(bind = %bind, "MCP HTTP server listening");

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn serve_stdio_mcp() -> Result<()> {
    // Stdio is always LoopbackDev — trusted local pipe, no HTTP auth context.
    let config = Config::load()?;
    let service = AppriseService::new(
        AppriseClient::new(&config.apprise)?,
        config.apprise.url.clone(),
    );
    #[allow(clippy::useless_conversion)]
    let state = AppState {
        config: config.mcp,
        auth_policy: AuthPolicy::LoopbackDev,
        service,
        counters: Arc::new(apprise_mcp::observability::Counters::default()),
        clock: Arc::new(apprise_mcp::observability::ServerClock::new()),
    };
    let _ = (); // appease compiler
    let svc = mcp::rmcp_server(state).serve(stdio()).await?;
    svc.waiting().await?;
    Ok(())
}

async fn run_cli(args: Vec<String>) -> Result<()> {
    let json = args.iter().any(|a| a == "--json");
    let filtered: Vec<String> = args.into_iter().filter(|a| a != "--json").collect();

    // doctor runs before service construction — it validates the environment
    if matches!(filtered.as_slice(), [c] if c == "doctor") {
        let config = Config::load().unwrap_or_default();
        return cli::run_doctor(&config, json).await;
    }

    let config = Config::load()?;
    let service = AppriseService::new(
        AppriseClient::new(&config.apprise)?,
        config.apprise.url.clone(),
    );
    let cmd = cli::CliCommand::parse(&filtered)?;
    cli::run(&service, cmd, json).await
}

async fn build_state(config: Config) -> Result<AppState> {
    let service = AppriseService::new(
        AppriseClient::new(&config.apprise)?,
        config.apprise.url.clone(),
    );

    let auth_policy = if config.mcp.no_auth || config.mcp.host.starts_with("127.") {
        AuthPolicy::LoopbackDev
    } else if config.mcp.auth.mode == AuthMode::OAuth {
        // Build full OAuth auth state (Google flow + JWKS)
        let auth_state = build_oauth_state(&config).await?;
        AuthPolicy::Mounted {
            auth_state: Some(Arc::new(auth_state)),
        }
    } else {
        // Bearer-token only (static token in APPRISE_MCP_TOKEN)
        AuthPolicy::Mounted { auth_state: None }
    };

    let svc_counters = service.counters.clone();
    let svc_clock = service.clock.clone();
    Ok(AppState {
        config: config.mcp,
        auth_policy,
        service,
        counters: svc_counters,
        clock: svc_clock,
    })
}

async fn build_oauth_state(config: &Config) -> Result<lab_auth::state::AuthState> {
    let vars: Vec<(String, String)> = {
        let auth = &config.mcp.auth;
        let mut v = vec![("APPRISE_MCP_AUTH_MODE".into(), "oauth".into())];
        if let Some(url) = &auth.public_url {
            v.push(("APPRISE_MCP_PUBLIC_URL".into(), url.clone()));
        }
        if let Some(id) = &auth.google_client_id {
            v.push(("APPRISE_MCP_GOOGLE_CLIENT_ID".into(), id.clone()));
        }
        if let Some(secret) = &auth.google_client_secret {
            v.push(("APPRISE_MCP_GOOGLE_CLIENT_SECRET".into(), secret.clone()));
        }
        if !auth.admin_email.is_empty() {
            v.push((
                "APPRISE_MCP_AUTH_ADMIN_EMAIL".into(),
                auth.admin_email.clone(),
            ));
        }
        v.push((
            "APPRISE_MCP_AUTH_SQLITE_PATH".into(),
            auth.sqlite_path.clone(),
        ));
        v.push(("APPRISE_MCP_AUTH_KEY_PATH".into(), auth.key_path.clone()));
        v
    };

    let auth_config = lab_auth::config::AuthConfigBuilder::new()
        .env_prefix("APPRISE_MCP")
        .session_cookie_name("apprise_mcp_session")
        .scopes_supported(vec!["apprise:notify".into(), "apprise:admin".into()])
        .default_scope("apprise:notify")
        .resource_path("/mcp")
        .build_from_sources(vars)
        .map_err(|e| anyhow::anyhow!("failed to build auth config: {e}"))?;

    lab_auth::state::AuthState::new(auth_config)
        .await
        .map_err(|e| anyhow::anyhow!("failed to init auth state: {e}"))
}

fn print_usage() {
    eprintln!(
        "Usage:
  apprise [serve]                    Start MCP HTTP server (default)
  apprise mcp                        Start MCP stdio transport
  apprise doctor                     Pre-flight environment validation

Notification:
  apprise notify <body> [--tag TAG] [--title T] [--type info|success|warning|failure]
  apprise notify-url <urls> <body> [--title T] [--type ...]

Server:
  apprise health [--json]            Check Apprise server health
  apprise doctor [--json]            Validate environment before deployment

Options:
  --json                             Output raw JSON

Environment:
  APPRISE_URL                  Apprise API server URL (default: http://localhost:8000)
  APPRISE_TOKEN                API token (optional for open installs)
  APPRISE_MCP_HOST             Bind host (default: 0.0.0.0)
  APPRISE_MCP_PORT             Bind port (default: 8765)
  APPRISE_MCP_NO_AUTH          Disable MCP HTTP auth (loopback only)
  APPRISE_MCP_TOKEN            Static bearer token for MCP HTTP auth
  APPRISE_MCP_AUTH_MODE        Auth mode: bearer (default) | oauth
  APPRISE_MCP_PUBLIC_URL       Public URL for OAuth metadata endpoints
  APPRISE_MCP_GOOGLE_CLIENT_ID     Google OAuth client ID
  APPRISE_MCP_GOOGLE_CLIENT_SECRET Google OAuth client secret
  APPRISE_MCP_AUTH_ADMIN_EMAIL     Admin email for OAuth
  RUST_LOG                     Log filter"
    );
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "CTRL+C handler failed");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                tracing::error!(error = %e, "SIGTERM handler failed");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    tracing::info!("Shutdown signal received");
}

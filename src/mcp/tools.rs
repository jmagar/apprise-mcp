use serde_json::{json, Value};

use crate::apprise::NotifyType;

use super::AppState;

/// Thin shim — parse args, call service, return Value. No logic here.
pub(super) async fn execute_tool(
    state: &AppState,
    name: &str,
    args: Value,
) -> anyhow::Result<Value> {
    match name {
        "apprise" => dispatch(state, args).await,
        _ => Err(anyhow::anyhow!("unknown tool: {name}")),
    }
}

async fn dispatch(state: &AppState, args: Value) -> anyhow::Result<Value> {
    let action =
        string_arg(&args, "action").ok_or_else(|| anyhow::anyhow!("action is required"))?;

    match action.as_str() {
        "notify" => {
            let body = string_arg(&args, "body")
                .ok_or_else(|| anyhow::anyhow!("`body` is required for notify"))?;
            let tag = string_arg(&args, "tag");
            let title = string_arg(&args, "title");
            let notify_type = parse_notify_type(&args)?;

            match tag.as_deref() {
                Some(t) => {
                    state
                        .service
                        .notify(t, title.as_deref(), &body, &notify_type)
                        .await
                }
                None => {
                    state
                        .service
                        .notify_all(title.as_deref(), &body, &notify_type)
                        .await
                }
            }
        }
        "notify_url" => {
            let urls = string_arg(&args, "urls")
                .ok_or_else(|| anyhow::anyhow!("`urls` is required for notify_url"))?;
            let body = string_arg(&args, "body")
                .ok_or_else(|| anyhow::anyhow!("`body` is required for notify_url"))?;
            let title = string_arg(&args, "title");
            let notify_type = parse_notify_type(&args)?;

            state
                .service
                .notify_url(&urls, title.as_deref(), &body, &notify_type)
                .await
        }
        "health" => state.service.health().await,
        "help" => Ok(json!({ "help": HELP_TEXT })),
        other => Err(anyhow::anyhow!(
            "unknown apprise action: {other}; use action=help for documentation"
        )),
    }
}

fn string_arg(args: &Value, name: &str) -> Option<String> {
    args.get(name).and_then(|v| v.as_str()).map(String::from)
}

fn parse_notify_type(args: &Value) -> anyhow::Result<NotifyType> {
    match string_arg(args, "type") {
        None => Ok(NotifyType::default()),
        Some(s) => NotifyType::from_str_opt(&s).ok_or_else(|| {
            anyhow::anyhow!("`type` must be info|success|warning|failure, got {s:?}")
        }),
    }
}

const HELP_TEXT: &str = r#"# apprise MCP Tool

Send push notifications via the Apprise API server.
Set the required `action` argument to select the operation.

## Actions

### notify
Send a notification to one or all configured Apprise services.

Required: `body`
Optional:
  - `tag`   — send only to services under this tag (omit = send to all)
  - `title` — notification title
  - `type`  — info (default) | success | warning | failure

### notify_url
Stateless one-off notification to a specific Apprise URL schema.
No pre-configuration needed on the server.

Required: `urls`, `body`
Optional:
  - `title` — notification title
  - `type`  — info (default) | success | warning | failure

Example urls: "slack://tokenA/tokenB/tokenC"
              "discord://webhook_id/webhook_token"
              "mailto://user:pass@gmail.com"

### health
Check the Apprise server health endpoint.

### help
Show this documentation.

## Notification types
- `info`    — informational (default)
- `success` — successful operation
- `warning` — non-critical warning
- `failure` — critical failure or error
"#;

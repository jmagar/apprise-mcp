use serde_json::{json, Value};

pub(super) const APPRISE_ACTIONS: &[&str] = &["notify", "notify_url", "health", "help"];

pub(super) fn tool_definitions() -> Vec<Value> {
    vec![json!({
        "name": "apprise",
        "description": "Send push notifications via the Apprise API server. Use action=help for documentation.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform.",
                    "enum": APPRISE_ACTIONS
                },
                "body": {
                    "type": "string",
                    "description": "Notification body/message text (required for notify and notify_url)."
                },
                "tag": {
                    "type": "string",
                    "description": "Apprise tag — send only to services under this tag. Omit to send to all services."
                },
                "title": {
                    "type": "string",
                    "description": "Notification title (optional)."
                },
                "type": {
                    "type": "string",
                    "description": "Notification type.",
                    "enum": ["info", "success", "warning", "failure"]
                },
                "urls": {
                    "type": "string",
                    "description": "Apprise URL schema string for stateless notify_url action (e.g. 'slack://token')."
                }
            },
            "required": ["action"]
        }
    })]
}

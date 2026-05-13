use rmcp::model::{
    GetPromptRequestParams, GetPromptResult, ListPromptsResult, Prompt, PromptMessage,
    PromptMessageRole,
};

pub(super) fn list_prompts() -> ListPromptsResult {
    ListPromptsResult {
        prompts: vec![Prompt::new(
            "send_alert",
            Some("Guide for sending a critical alert notification via Apprise."),
            None,
        )],
        ..Default::default()
    }
}

pub(super) fn get_prompt(request: GetPromptRequestParams) -> anyhow::Result<GetPromptResult> {
    match request.name.as_str() {
        "send_alert" => Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            "Use the apprise tool with action=notify to send a critical alert. \
             Set type=failure to indicate urgency. Provide a clear, concise title \
             summarising the problem and a body with enough detail for the recipient \
             to understand the impact and next steps. \
             If a specific tag is relevant (e.g. 'ops', 'alerts'), include it; \
             otherwise omit the tag to broadcast to all configured services.",
        )])
        .with_description("Send a critical alert notification via Apprise")),
        other => Err(anyhow::anyhow!("unknown prompt: {other}")),
    }
}

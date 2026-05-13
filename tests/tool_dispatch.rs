use apprise_mcp::testing::stub_state;

#[tokio::test]
async fn stub_state_builds() {
    // Verifies that AppriseClient and AppriseService can be constructed
    // from a minimal (loopback) config without panicking.
    let _state = stub_state();
}

#[tokio::test]
async fn health_fails_gracefully_against_stub() {
    let state = stub_state();
    // Stub points at port 1 — request should fail with an error, not panic.
    let result = state.service.health().await;
    assert!(
        result.is_err(),
        "health against unreachable host should return Err"
    );
}

#[tokio::test]
async fn notify_fails_gracefully_against_stub() {
    use apprise_mcp::apprise::NotifyType;
    let state = stub_state();
    let result = state
        .service
        .notify("ops", Some("Test"), "body text", &NotifyType::Info)
        .await;
    assert!(
        result.is_err(),
        "notify against unreachable host should return Err"
    );
}

#[tokio::test]
async fn notify_all_fails_gracefully_against_stub() {
    use apprise_mcp::apprise::NotifyType;
    let state = stub_state();
    let result = state
        .service
        .notify_all(None, "broadcast body", &NotifyType::Warning)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn notify_url_fails_gracefully_against_stub() {
    use apprise_mcp::apprise::NotifyType;
    let state = stub_state();
    let result = state
        .service
        .notify_url(
            "slack://token/channel",
            Some("Alert"),
            "something broke",
            &NotifyType::Failure,
        )
        .await;
    assert!(result.is_err());
}

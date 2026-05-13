pub mod app;
pub mod apprise;
pub mod config;
pub mod logging;
pub mod mcp;
pub mod observability;
pub mod token_limit;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod testing {
    use std::sync::Arc;

    use crate::{
        app::AppriseService,
        apprise::AppriseClient,
        config::{AppriseConfig, McpConfig},
        mcp::{AppState, AuthPolicy},
        observability::{Counters, ServerClock},
    };

    fn stub_service() -> AppriseService {
        let client = AppriseClient::new(&AppriseConfig {
            url: "http://localhost:1".into(),
            token: String::new(),
        })
        .expect("stub client should build");
        AppriseService::new(client)
    }

    pub fn stub_state() -> AppState {
        AppState {
            config: McpConfig::default(),
            auth_policy: AuthPolicy::LoopbackDev,
            service: stub_service(),
            counters: Arc::new(Counters::default()),
            clock: Arc::new(ServerClock::new()),
        }
    }
}

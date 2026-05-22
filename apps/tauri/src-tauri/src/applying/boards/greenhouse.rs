use crate::applying::selectors::FormSelectors;
use crate::applying::types::{Applier, ApplyContext, ApplyResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct GreenhouseApplier;

#[async_trait]
impl Applier for GreenhouseApplier {
    fn board_id(&self) -> &'static str {
        "greenhouse"
    }
    fn display_name(&self) -> &'static str {
        "Greenhouse"
    }

    async fn apply(&self, posting_url: String, ctx: ApplyContext) -> Result<ApplyResult> {
        super::shared::navigate_and_assist(
            "greenhouse",
            "Greenhouse",
            posting_url,
            ctx,
            FormSelectors::greenhouse(),
            &["#submit_app", "a[href*='application']"],
        )
        .await
    }
}

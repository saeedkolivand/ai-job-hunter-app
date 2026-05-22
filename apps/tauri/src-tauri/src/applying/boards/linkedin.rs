use crate::applying::selectors::FormSelectors;
use crate::applying::types::{Applier, ApplyContext, ApplyResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct LinkedInApplier;

#[async_trait]
impl Applier for LinkedInApplier {
    fn board_id(&self) -> &'static str {
        "linkedin"
    }
    fn display_name(&self) -> &'static str {
        "LinkedIn"
    }

    async fn apply(&self, posting_url: String, ctx: ApplyContext) -> Result<ApplyResult> {
        super::shared::navigate_and_assist(
            "linkedin",
            "LinkedIn",
            posting_url,
            ctx,
            FormSelectors::linkedin(),
            &["button[aria-label*='Easy Apply']", "button.jobs-apply-button"],
        )
        .await
    }
}

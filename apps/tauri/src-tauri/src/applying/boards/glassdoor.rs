use crate::applying::selectors::FormSelectors;
use crate::applying::types::{Applier, ApplyContext, ApplyResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct GlassdoorApplier;

#[async_trait]
impl Applier for GlassdoorApplier {
    fn board_id(&self) -> &'static str {
        "glassdoor"
    }
    fn display_name(&self) -> &'static str {
        "Glassdoor"
    }

    async fn apply(&self, posting_url: String, ctx: ApplyContext) -> Result<ApplyResult> {
        super::shared::navigate_and_assist(
            "glassdoor",
            "Glassdoor",
            posting_url,
            ctx,
            FormSelectors::glassdoor(),
            &[
                "button[data-test='apply-button']",
                "a[data-test='apply-button']",
                ".apply-button",
                "button:has-text('Apply')",
            ],
        )
        .await
    }
}

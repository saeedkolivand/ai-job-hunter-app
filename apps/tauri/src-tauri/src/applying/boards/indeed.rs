use crate::applying::selectors::FormSelectors;
use crate::applying::types::{Applier, ApplyContext, ApplyResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct IndeedApplier;

#[async_trait]
impl Applier for IndeedApplier {
    fn board_id(&self) -> &'static str {
        "indeed"
    }
    fn display_name(&self) -> &'static str {
        "Indeed"
    }

    async fn apply(&self, posting_url: String, ctx: ApplyContext) -> Result<ApplyResult> {
        super::shared::navigate_and_assist(
            "indeed",
            "Indeed",
            posting_url,
            ctx,
            FormSelectors::indeed(),
            &["#indeedApplyButton", "button[aria-label*='Apply']"],
        )
        .await
    }
}

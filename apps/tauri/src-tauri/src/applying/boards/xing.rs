use crate::applying::selectors::FormSelectors;
use crate::applying::types::{Applier, ApplyContext, ApplyResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct XingApplier;

#[async_trait]
impl Applier for XingApplier {
    fn board_id(&self) -> &'static str {
        "xing"
    }
    fn display_name(&self) -> &'static str {
        "Xing"
    }

    async fn apply(&self, posting_url: String, ctx: ApplyContext) -> Result<ApplyResult> {
        super::shared::navigate_and_assist(
            "xing",
            "Xing",
            posting_url,
            ctx,
            FormSelectors::xing(),
            &["button[data-qa='apply-button']", "a[href*='apply']", ".apply-button"],
        )
        .await
    }
}

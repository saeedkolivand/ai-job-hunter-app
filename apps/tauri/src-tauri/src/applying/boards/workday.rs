use crate::applying::selectors::FormSelectors;
use crate::applying::types::{Applier, ApplyContext, ApplyResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct WorkdayApplier;

#[async_trait]
impl Applier for WorkdayApplier {
    fn board_id(&self) -> &'static str {
        "workday"
    }
    fn display_name(&self) -> &'static str {
        "Workday"
    }

    async fn apply(&self, posting_url: String, ctx: ApplyContext) -> Result<ApplyResult> {
        super::shared::navigate_and_assist(
            "workday",
            "Workday",
            posting_url,
            ctx,
            FormSelectors::workday(),
            &["[data-automation-id='applyManually']", "[data-automation-id='apply']"],
        )
        .await
    }
}

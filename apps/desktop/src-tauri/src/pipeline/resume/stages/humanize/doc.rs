//! One document's humanize pass, résumé and letter alike: locate the flagged
//! lines, ask the model for line patches, apply them, and hand the patched
//! document to [`humanize_one`]'s accept/revert decision.

use crate::error::AppResult;
use crate::pipeline::resume::prompts::{
    humanize_patch_schema, humanize_system, humanize_user, HumanizeTier, HUMANIZE_PATCH_EXAMPLE,
};
use crate::pipeline::resume::RunDeadline;
use crate::pipeline::Completer;
use crate::validate::content::ContentReport;

use super::attempt::{humanize_one, HumanizeAttempt};
use super::patches::{
    apply_patches, excerpt, findings_for_prompt, flagged_lines, FlaggedLine, PatchList,
};

/// What every document's pass shares.
pub(super) struct DocEnv<'a> {
    pub completer: &'a Completer,
    pub deadline: RunDeadline,
    /// Run before the structured call's one re-ask (see `Completer::complete_json`).
    pub guard: &'a (dyn Fn() -> AppResult<()> + Sync),
    pub lang: &'a str,
    /// The stage effort (`QualityCtx::stage_effort`), sent on every call.
    pub effort: Option<&'a str>,
}

/// Humanize one document by line patches. `normalize`/`revalidate` are the
/// same seams [`humanize_one`] takes. No flagged line means NO provider call.
pub(super) async fn humanize_doc<N, G, GFut>(
    env: &DocEnv<'_>,
    tier: HumanizeTier,
    text: String,
    report: ContentReport,
    normalize: N,
    revalidate: G,
) -> AppResult<HumanizeAttempt>
where
    N: Fn(&str) -> Option<String>,
    G: FnMut(String) -> GFut,
    GFut: std::future::Future<Output = AppResult<ContentReport>>,
{
    let flags = flagged_lines(&report, &text);
    let document_wide = flags.document_wide;
    humanize_one(
        env.deadline,
        text,
        report,
        flags.lines,
        |text, lines: Vec<FlaggedLine>| {
            let document_wide = document_wide.clone();
            async move {
                let user = humanize_user(
                    &excerpt(&text, &lines),
                    &findings_for_prompt(&lines, &document_wide),
                );
                let list: PatchList = env
                    .completer
                    .complete_json(
                        || (env.guard)(),
                        &humanize_system(tier, env.lang),
                        &user,
                        HUMANIZE_PATCH_EXAMPLE,
                        Some(&humanize_patch_schema()),
                        env.effort,
                    )
                    .await?;
                Ok(apply_patches(&text, &lines, &list.patches))
            }
        },
        normalize,
        revalidate,
        tier,
    )
    .await
}

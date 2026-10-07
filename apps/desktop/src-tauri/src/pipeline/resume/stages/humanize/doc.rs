//! One document's humanize pass, résumé and letter alike: locate the flagged
//! lines, ask the model for line patches, apply them, and hand the patched
//! document to [`humanize_one`]'s accept/revert decision.

use crate::error::AppResult;
use crate::pipeline::resume::prompts::{
    humanize_patch_schema, humanize_rewrite_system, humanize_system, humanize_user, HumanizeTier,
    HUMANIZE_PATCH_EXAMPLE,
};
use crate::pipeline::resume::RunDeadline;
use crate::pipeline::Completer;
use crate::validate::content::ContentReport;

use super::attempt::{humanize_one, HumanizeAttempt};
use super::patches::{
    apply_patches, excerpt, findings_for_prompt, flagged_lines, humanize_mode, FlaggedLine, Mode,
    PatchList,
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

/// Humanize one document: by line patches when any flag is line-locatable,
/// by whole-document rewrite when every flag is document-wide, with NO call
/// when nothing eligible is flagged. `normalize`/`revalidate` are the same
/// seams [`humanize_one`] takes. Returns the mode that ran.
pub(super) async fn humanize_doc<N, G, GFut>(
    env: &DocEnv<'_>,
    tier: HumanizeTier,
    text: String,
    report: ContentReport,
    normalize: N,
    revalidate: G,
) -> AppResult<(HumanizeAttempt, Option<Mode>)>
where
    N: Fn(&str) -> Option<String>,
    G: FnMut(String) -> GFut,
    GFut: std::future::Future<Output = AppResult<ContentReport>>,
{
    let mut flags = flagged_lines(&report, &text, env.lang);
    // Offer only the flagged lines that fit the excerpt whole (see `excerpt`).
    let (excerpt_text, shown) = excerpt(&text, &flags.lines);
    flags.lines = shown;
    let mode = humanize_mode(&flags);
    let document_wide = flags.document_wide;
    let attempt = if mode == Some(Mode::Rewrite) {
        humanize_one(
            env.deadline,
            text,
            report,
            document_wide,
            |text, findings: Vec<String>| async move {
                // `complete_with_effort` does not charge the daily ceiling
                // itself (`complete_json` does): charge here, so a refusal
                // lands in `humanize_one`'s `capped` arm.
                env.completer.charge_daily()?;
                env.completer
                    .complete_with_effort(
                        &humanize_rewrite_system(tier, env.lang),
                        &humanize_user(&text, &findings),
                        None,
                        env.effort,
                    )
                    .await
            },
            normalize,
            revalidate,
            tier,
            // The whole document goes out, so the size cap applies.
            true,
        )
        .await?
    } else {
        humanize_one(
            env.deadline,
            text,
            report,
            flags.lines,
            |text, lines: Vec<FlaggedLine>| {
                let document_wide = document_wide.clone();
                let excerpt_text = excerpt_text.clone();
                async move {
                    let user =
                        humanize_user(&excerpt_text, &findings_for_prompt(&lines, &document_wide));
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
            // Only a small excerpt goes out, so the document cap does not apply.
            false,
        )
        .await?
    };
    Ok((attempt, mode))
}

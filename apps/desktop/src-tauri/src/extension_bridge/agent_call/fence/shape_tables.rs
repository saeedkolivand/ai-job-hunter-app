//! Shape-anchor tables for the fence rules that key off an object's SHAPE rather than a single
//! field name — the non-name-keyed half of `fence_named_fields_recursive`.

use serde_json::Value;

/// `ai_generations::ApplicationAnswer`'s own always-present sibling key —
/// used to detect an `ApplicationAnswer`-shaped object (`{id, question,
/// answer}`, reachable through `applications_list`/`applications_get`/
/// `ai_generations_list`) so its [`APPLICATION_ANSWER_QUESTION_FIELD`] — a
/// THIRD-PARTY ATS form's own question label, captured from the page by
/// `extension_bridge::answers_save` — is fenced by SHAPE rather than by
/// name.
///
/// Deliberately NOT a [`FENCE_FIELD_NAMES`] entry: a flat name entry would
/// ALSO re-fence `ai_generations::InterviewQuestion.question` (`{id,
/// question, why, audience}`), which serializes under the EXACT same wire
/// key, rides the SAME command's response, and is this app's own AI
/// coaching output — the one thing that const's own doc says it excludes on
/// purpose (ADR-038 §5's separate axis). `answer` is the discriminator: an
/// `ApplicationAnswer` always carries one, an `InterviewQuestion` never
/// does.
///
/// Note `extension_bridge::answers_suggest::answers_suggest_reply` builds a
/// sibling `{question, answer}` object too, but it is a BRIDGE frame, not a
/// dispatched command response, so it never reaches this walk; were that
/// shape ever to move onto this surface it would simply be fenced the same
/// way — the safe direction.
pub(in crate::extension_bridge::agent_call) const APPLICATION_ANSWER_ANCHOR_FIELDS: [&str; 1] =
    ["answer"];

/// The single key [`APPLICATION_ANSWER_ANCHOR_FIELDS`] guards, named once so
/// [`fence_named_fields_recursive`] and `agent_call::reshape::unfence_named_fields_recursive`
/// can never disagree about which field the shape rule covers.
pub(in crate::extension_bridge::agent_call) const APPLICATION_ANSWER_QUESTION_FIELD: &str =
    "question";

/// `jobs::JobRecord`'s own always-present, distinctively-named fields
/// (`kind`, `progress`, `max_retries` → `maxRetries` under that struct's
/// `#[serde(rename_all = "camelCase")]`) — used to detect a
/// `JobRecord`-shaped object (`jobs_get`, `jobs_list`) so
/// [`JOB_RECORD_RESULT_FIELD`] can be EXEMPTED from the name-keyed walk.
///
/// A completed job's `result` is the app's OWN output — a generated draft or
/// a model answer under `{"done": true, "text": …}`
/// (`commands::ai_provider::stream`, `commands::resume_pipeline`) — while
/// `text` is on [`FENCE_FIELD_NAMES`] for `documents::DocumentRecord.text`,
/// so before this exemption every generation read back through `jobs_get`
/// reached the caller wrapped as a scraped posting. Verified no other struct
/// on this dispatch surface serializes all three anchors together
/// (`maxRetries` has exactly one producer in the crate).
///
/// The exemption is WHOLESALE for the NAME-keyed walk and audited, not
/// shape-inspected per value: no [`FENCE_FIELD_NAMES`] entry fires anywhere
/// under `result`, so a job kind that starts putting THIRD-PARTY text there
/// must fence it itself. The warning that says so lives on
/// `commands::jobs::job_complete` — the single mutator every completion
/// funnels through — rather than on each producer.
///
/// The scrape-diagnostics shapes are carved back out, because auditing the
/// producer list turned up a completion that already carried third-party
/// text: [`SCRAPE_SUMMARY_ANCHOR_FIELDS`] and [`BOARD_HEALTH_ANCHOR_FIELDS`]
/// fence a `BoardScrapeSummary`'s board-written strings wherever they sit
/// inside `result`. Those are shape rules with enumerated field sets, not a
/// reopening of the name walk — see [`fence_scrape_summaries_recursive`] for
/// why the distinction is load-bearing.
pub(in crate::extension_bridge::agent_call) const JOB_RECORD_ANCHOR_FIELDS: [&str; 3] =
    ["kind", "progress", "maxRetries"];

/// The one `JobRecord` field [`JOB_RECORD_ANCHOR_FIELDS`] exempts. Every
/// other field still recurses — `payload` included, since a dispatch payload
/// CAN carry a scraped posting.
pub(in crate::extension_bridge::agent_call) const JOB_RECORD_RESULT_FIELD: &str = "result";

/// `scraping::engine::BoardScrapeSummary`'s own always-present field pair
/// (`board`, `count` — both non-`Option`, and single words that its
/// `#[serde(rename_all = "camelCase")]` leaves unchanged) — used to detect a
/// summary-shaped object so [`SCRAPE_SUMMARY_UNTRUSTED_FIELDS`] can be fenced
/// by SHAPE.
///
/// Shape and never a [`FENCE_FIELD_NAMES`] row, for the same reason
/// [`APPLICATION_ANSWER_ANCHOR_FIELDS`] is: `error` is one of the most
/// generic keys on this whole surface — `jobs::JobRecord.error` itself, plus
/// every refusal envelope — and a flat name entry would wrap this app's own
/// already-sanitized error strings as though a job board had written them.
///
/// Verified distinctive on this dispatch surface: `board` occurs WITHOUT a
/// sibling `count` on `board_health::BoardHealthEntry` (`{board, health}`)
/// and on a cluster member (`{key, board?, url}`), and `count` occurs
/// without a `board` on the `scrape_*` completion envelopes themselves
/// (`{count, boards}` / `{count}`) — no other struct in the crate
/// serializes both together.
pub(in crate::extension_bridge::agent_call) const SCRAPE_SUMMARY_ANCHOR_FIELDS: [&str; 2] =
    ["board", "count"];

/// The board/provider-derived strings a [`SCRAPE_SUMMARY_ANCHOR_FIELDS`]-
/// detected object carries. Each is written by the REMOTE side of a scrape,
/// not by this app: `error` is a board's own failure text (an aggregator
/// provider prefixes its own name onto whatever the upstream API returned),
/// `skipped` its refusal reason, `truncated` a mid-run page failure. Its
/// siblings are not here on purpose — `board`/`count` are the anchors and
/// `notes` is a fixed engine vocabulary. `health` is not a string at all;
/// its own board-written carrier is covered by
/// [`BOARD_HEALTH_ANCHOR_FIELDS`] below.
///
/// These reach an agent through a completed `scrape_boards` job
/// (`jobs_get`/`jobs_list`, where [`JOB_RECORD_RESULT_FIELD`] otherwise
/// exempts the whole subtree) and through `Autopilot.last_run_summaries`
/// (`autopilot_list`/`autopilot_get`), so the rule is applied to the shape
/// wherever it appears rather than to either route.
pub(in crate::extension_bridge::agent_call) const SCRAPE_SUMMARY_UNTRUSTED_FIELDS: [&str; 3] =
    ["error", "skipped", "truncated"];

/// `scraping::board_health::BoardHealth`'s own always-present field pair
/// (`status`, `consecutive_failures` → `consecutiveFailures`) — the SECOND
/// shape carrying board-written text in the same payload, because
/// `board_health::fold` copies `BoardScrapeSummary.error` FORWARD into
/// `BoardHealth.last_error`. That copy runs through `clean_error`, which
/// redacts paths/hosts and caps the length — a redactor, not a controlled
/// vocabulary — so the board's own prose survives it intact and is exactly
/// as untrusted as the `error` it came from. Fencing one and not the other
/// would leave the same sentence reachable one level deeper, under
/// `summary.health.lastError`, and standalone on a `BoardHealthEntry.health`.
///
/// `consecutiveFailures` is the distinctive half: it is the only serialized
/// field of that name in the crate (verified), so no other struct on this
/// surface can be mistaken for this shape.
pub(in crate::extension_bridge::agent_call) const BOARD_HEALTH_ANCHOR_FIELDS: [&str; 2] =
    ["status", "consecutiveFailures"];

/// The one board-written string on a [`BOARD_HEALTH_ANCHOR_FIELDS`]-detected
/// object. Its siblings are counters, epoch-ms timestamps, a derived status
/// enum and this app's own scrape `job_id` — none of them third-party text.
pub(in crate::extension_bridge::agent_call) const BOARD_HEALTH_UNTRUSTED_FIELDS: [&str; 1] =
    ["lastError"];

/// True when `map` is an `ai_generations::ApplicationAnswer`-shaped object:
/// a STRING [`APPLICATION_ANSWER_QUESTION_FIELD`] plus every
/// [`APPLICATION_ANSWER_ANCHOR_FIELDS`] key. Shared by the fence and the
/// unfence walk so the two can never disagree about the shape.
pub(in crate::extension_bridge::agent_call) fn is_application_answer_shaped(
    map: &serde_json::Map<String, Value>,
) -> bool {
    map.get(APPLICATION_ANSWER_QUESTION_FIELD)
        .is_some_and(Value::is_string)
        && APPLICATION_ANSWER_ANCHOR_FIELDS
            .iter()
            .all(|f| map.contains_key(*f))
}

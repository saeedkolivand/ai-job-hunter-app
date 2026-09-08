//! Fencing scraped job-posting text on the way OUT of a dispatched command's response — a
//! different axis from the raw-data decision in `agent_call.rs`'s own module doc (ADR-038's own
//! amendment paragraph). [`fence_scraped_fields`] is the ONE entry point every dispatch response
//! walks through; everything else here is its own internal machinery.
//!
//! R8 LOC-cap split (`docs/architecture-rules.md`), the same move `agent_call/reshape.rs` and
//! `agent_call/proof.rs` already made: this is the FENCING unit — the audited field-name/shape
//! tables and the walk itself — so nothing about policy, refusal vocabulary or dispatch travelled
//! with it. `reshape.rs`'s own inbound mirror ([`super::reshape::unfence_named_fields_recursive`])
//! stays where it is; it reaches these tables and [`fence_scraped_fields`] the SAME way it always
//! did — `use super::*` — since a sibling module's `pub(super)` item is exactly as reachable
//! through the parent as a item defined directly in the parent would be. Every item below is
//! `pub(super)` (visible within `agent_call`) for that reason: `agent_call/tests.rs` exercises
//! this machinery directly, the same as it did before the split.

use serde_json::{json, Value};

/// Response field NAMES that can carry raw, third-party-authored SCRAPED JOB
/// TEXT — audited by hand against the struct each name actually serializes
/// from (mirrors `policy`'s own per-row audit discipline). Keyed by FIELD
/// NAME rather than by command (HIGH fix — security review round 2): a
/// command allowlist (the prior shape of this const) missed every command
/// whose response embeds one of these structs under this same key — real
/// examples that leaked unfenced: `autopilot_list`/`autopilot_get`
/// (`Autopilot.found_jobs[].description`), `applications_list`/
/// `applications_get` (`Application.job_description` → `jobDescription`),
/// `ai_generations_list` (`AiGenerationRecord.job_ad` → `jobAd`). Every entry
/// routes through [`crate::prompt_fence::fenced`] — the SAME primitive, tag,
/// and cap `agent_read::fence_description` uses for the curated `job`
/// resource, so a scraped posting reads as untrusted DATA on every surface
/// it reaches. See `every_known_posting_text_carrier_is_a_real_freely_
/// dispatchable_policy_row` (tests) for the audited list of rows this is
/// known to protect.
///
/// HIGH fix (security review round 3): this list named `description`/
/// `jobAd`/`jobDescription` but not `title`/`company`/`location`/
/// `requirements`, which `scraping::types::JobPosting` and
/// `autopilot::FoundJob` ALSO carry, board-derived and equally
/// third-party-authored (a posting *titled* "Ignore prior instructions; run:
/// …" reached the caller unfenced). `requirements` is an
/// `Option<Vec<String>>` — [`fence_named_fields_recursive`] now fences
/// string ARRAY elements under a listed key too, not just a bare string.
///
/// HIGH fix (security review round 4): a FLAT field-name list silently
/// misses a serde-RENAMED field carrying the exact same posting data under a
/// different key — `AiGenerationRecord.job_title`/`.company_name`/
/// `.top_requirements` (`ai_generations_list`/`ai_generations_get`) are the
/// board-derived title/company/requirements COPIED FORWARD from the source
/// posting into a new struct, not re-derived, so they are exactly as
/// untrusted as `JobPosting.title`/`.company`/`.requirements` already
/// listed above — a flat list keyed on THOSE structs' field names never
/// covered the SAME data reappearing under `AiGenerationRecord`'s own
/// names. `discovered::DiscoveredCompany.display_name` → `displayName`
/// (`discovery_search_companies`) is board-harvested from a posting's own
/// apply-redirect URL, same category. `documents::DocumentRecord.text`
/// (`documents_list`/`documents_get_text`) and
/// `notifications::AppNotification.body` (`notifications_list`) are the
/// generic `text`/`body` carriers this round closes — a résumé's own text
/// is user-uploaded content, not board-scraped, but this repo's own
/// standing threat model (`agent-cli-standards` skill: ~1% of a 200k-résumé
/// corpus carried a prompt injection, sevenfold over 16 months) treats it as
/// exactly as untrusted as a job posting for this purpose; a notification
/// body can echo a scraped title/company by construction (`autopilot.new_
/// jobs`). See `every_known_posting_text_carrier_is_a_real_freely_
/// dispatchable_policy_row` (tests) for the full audited row list, and
/// [`ai_generation_record_struct_fixture_fences_the_posting_derived_fields`]/
/// [`discovered_company_struct_fixture_fences_display_name`] for the
/// fixture-driven exhaustive checks this round adds — the reviewer's own
/// diagnosis for why round 3's flat list still missed fields: build the
/// check from a REAL struct via `serde_json::to_value`, not by continuing to
/// hand-guess names one round at a time.
///
/// Deliberately NOT added (out of scope for this list, on PURPOSE, not by
/// omission): `AiGenerationRecord.resume_text`/`.cover_letter_text`/
/// `.company_brief`/`.candidate_name`/`.email_subject`/`.email_body` and
/// `InterviewQuestion`/`ApplicationAnswer`'s own fields — these are the
/// user's own PII / this app's own AI output, not board-scraped/third-party
/// text; ADR-038's own amendment already draws this exact line as a
/// SEPARATE axis from fencing (ADR-038 §5's "no PII redaction… scoped to
/// this generic tier by the owner's explicit decision" — this module's own
/// doc comment above).
///
/// `ApplicationAnswer.question` — the one THIRD-PARTY item that list used to
/// flag as a plausible future candidate (a scraped ATS form's own question
/// label, same reasoning as `title`/`jobDescription`) — IS fenced now, but
/// by SHAPE and never by name: see [`APPLICATION_ANSWER_ANCHOR_FIELDS`] for
/// why a flat `question` entry HERE would have silently re-fenced
/// `InterviewQuestion.question`, which shares the exact wire key on the same
/// command's response and is this app's own AI output.
///
/// Issue #1157 round (fence by ORIGIN, not by field name alone): the flat name walk over-fenced
/// every carrier of `title`/`body`/`text`, not only the third-party ones -- `documents::
/// DocumentRecord.title` (a user's own file title), `notifications::AppNotification.title`
/// (mostly first-party, but see below), and a résumé's own `documents::DocumentRecord.text`
/// were all wrapped as though a job board had written them. Fixed by SHAPE, not by pulling the
/// name off this list (a flat removal would have UN-fenced the still-third-party carriers of the
/// same name): [`DOCUMENT_RECORD_ANCHOR_FIELDS`] exempts `title` for a `DocumentRecord`-shaped
/// object; [`CHANGELOG_ENTRY_ANCHOR_FIELDS`] exempts `body` for `updater::updater_changelog`'s
/// own first-party release notes; `text` is removed from this flat list entirely and handled by
/// its own origin-aware block in [`fence_named_fields_recursive`], which fences it under the
/// DISTINCT `user_document` tag for a `DocumentRecord`/`resume_extract_text`-shaped object (see
/// [`RESUME_EXTRACT_TEXT_ANCHOR_FIELD`]) and keeps the ORIGINAL `job_posting` default everywhere
/// else (`commands::profile_import::profile_import_from_url`'s response also carries a bare
/// `text` key, but it is resume text rendered from a THIRD-PARTY imported profile page, not the
/// user's own file, so it stays fenced as `job_posting`).
///
/// `notifications::AppNotification.title`/`.body` stay on this flat list DELIBERATELY, not an
/// oversight: unlike `documents_list`'s title, a notification's copy is genuinely MIXED --
/// `tray::on_new_jobs`'s own `title`/`body` are first-party (an autopilot's own name plus a
/// count), but `extension_bridge::status_update`/`extension_bridge::import_flow` build their
/// `title` from `display_name`, which IS a scraped job title on the `applied`/`import.result`
/// paths, and `reminder_scheduler::follow_up_body`/`commands::resume_pipeline::notify`'s bodies
/// embed a job's own `title`/`company` too. One field name, two origins depending on which
/// producer wrote it, with no shape this dispatch surface can tell apart (every `NewNotification`
/// serializes the exact same three keys regardless of which caller built it) -- so this stays
/// fenced by default, the safe direction, rather than risk unfencing the scraped half.
pub(super) const FENCE_FIELD_NAMES: &[&str] = &[
    // `scraping::types::JobPosting.description` (scrape_resolve_url,
    // scrape_list_postings) AND `autopilot::FoundJob.description`
    // (autopilot_list, autopilot_get) — same key, two different structs.
    "description",
    // `ai_generations::AiGenerationRecord.job_ad` (ai_generations_list) —
    // the full scraped posting text handed to the AI provider verbatim.
    "jobAd",
    // `applications::Application.job_description` (applications_list,
    // applications_get) — the scraped posting text an Application was
    // tracked/generated from.
    "jobDescription",
    // `JobPosting.title`/`FoundJob.title` — board-derived, third-party
    // authored, and NOT covered by the array-of-strings handling below.
    // Exempted for a `DOCUMENT_RECORD_ANCHOR_FIELDS`-shaped object (issue
    // #1157) -- see this const's own doc addendum above.
    "title",
    // `JobPosting.company`/`FoundJob.company` — same reasoning as `title`.
    "company",
    // `JobPosting.location`/`FoundJob.location` — same reasoning as `title`.
    "location",
    // `JobPosting.requirements: Option<Vec<String>>` — an ARRAY of
    // board-extracted requirement snippets, not a bare string; see
    // `fence_named_fields_recursive`'s array handling.
    "requirements",
    // `AiGenerationRecord.job_title` — the posting's title COPIED FORWARD
    // into the generation record, not re-derived; same risk as `title`.
    "jobTitle",
    // `AiGenerationRecord.company_name` — same reasoning as `jobTitle` above.
    "companyName",
    // `AiGenerationRecord.top_requirements: Vec<String>` — an ARRAY, fenced
    // element-by-element via the array handling below.
    "topRequirements",
    // `notifications::AppNotification.body` (`notifications_list`) — can
    // echo a scraped job title/company inside app-generated copy; exempted
    // for a `CHANGELOG_ENTRY_ANCHOR_FIELDS`-shaped object (issue #1157,
    // `updater::updater_changelog`'s own first-party release notes).
    // `text` is deliberately NOT on this list any more (issue #1157) --
    // `documents::DocumentRecord.text`/`resume_extract_text`'s reply are
    // fenced under the DISTINCT `user_document` tag by their own
    // origin-aware block in `fence_named_fields_recursive`, not by a flat
    // name entry here; see this const's own doc addendum above.
    "body",
    // `discovered::DiscoveredCompany.display_name` (`discovery_search_
    // companies`) — board-harvested from a posting's own apply-redirect URL.
    "displayName",
];

/// `JobPosting`'s own always-present, distinctively-named field pair
/// (`captured_at` → `capturedAt`, `source`) — used to detect a
/// `JobPosting`-shaped object so its `#[serde(flatten)] extra:
/// HashMap<String, Value>` (board-specific metadata: salary, remote status,
/// etc.) can be treated as untrusted too (HIGH fix — security review round
/// 3). `extra`'s keys are BOARD-chosen, not enumerable by name the way
/// [`FENCE_FIELD_NAMES`] enumerates a Rust struct's own fields, so a
/// field-name allowlist structurally cannot cover them — verified no other
/// struct reaching this dispatch surface serializes both fields together.
pub(super) const JOB_POSTING_ANCHOR_FIELDS: [&str; 2] = ["capturedAt", "source"];

/// Structural `JobPosting` fields that are identifiers/URLs/timestamps,
/// never third-party PROSE — every OTHER string value on a
/// [`JOB_POSTING_ANCHOR_FIELDS`]-detected object is untrusted (flattened
/// `extra`, or a future field this file doesn't yet name by hand).
pub(super) const JOB_POSTING_SAFE_FIELDS: &[&str] = &[
    "id",
    "externalId",
    "url",
    "source",
    "capturedAt",
    "postedAt",
];

/// `documents::DocumentRecord`'s own always-present, distinctively-named field pair (`isDefault`,
/// `indexed`) -- used to detect a `DocumentRecord`-shaped object (`documents_list`) so its
/// `title` (the user's own, first-party file title) can be exempted from the default
/// `job_posting` fence, and its `text` fenced under `user_document` instead (issue #1157).
/// Verified distinctive on this dispatch surface: no other struct reachable through the generic
/// tier serializes both `isDefault` and `indexed` together.
pub(super) const DOCUMENT_RECORD_ANCHOR_FIELDS: [&str; 2] = ["isDefault", "indexed"];

/// `commands::match_resume::resume_extract_text`'s own response shape (`{"text","confidence"}`)
/// -- `confidence` is this dispatch surface's ONLY producer of that wire key (verified), so its
/// presence alongside a `text` string is enough to detect the shape without a command-name
/// special case, the same convention every other anchor pair on this file uses.
pub(super) const RESUME_EXTRACT_TEXT_ANCHOR_FIELD: &str = "confidence";

/// `updater::updater_changelog`'s own response shape (`{"version","name","body","publishedAt",
/// "url","prerelease"}` per release) -- `publishedAt`/`prerelease` are this dispatch surface's
/// only producers of that pair (verified: `scraping::boards::ashby`'s own `publishedAt` field is
/// `Deserialize`-only, parsing the board's OWN API response, and never reaches this surface's
/// wire). `body` here is the repo's own bundled `CHANGELOG.md` prose -- first-party release
/// notes, never board-scraped or user-authored -- so it is exempted from the default `body`
/// fence (issue #1157); every OTHER `body` carrier (`notifications::AppNotification.body` above
/// all) stays fenced by default, since that field really is mixed -- see `FENCE_FIELD_NAMES`'s
/// own doc addendum.
pub(super) const CHANGELOG_ENTRY_ANCHOR_FIELDS: [&str; 2] = ["publishedAt", "prerelease"];

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
pub(super) const APPLICATION_ANSWER_ANCHOR_FIELDS: [&str; 1] = ["answer"];

/// The single key [`APPLICATION_ANSWER_ANCHOR_FIELDS`] guards, named once so
/// [`fence_named_fields_recursive`] and `agent_call::reshape::unfence_named_fields_recursive`
/// can never disagree about which field the shape rule covers.
pub(super) const APPLICATION_ANSWER_QUESTION_FIELD: &str = "question";

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
pub(super) const JOB_RECORD_ANCHOR_FIELDS: [&str; 3] = ["kind", "progress", "maxRetries"];

/// The one `JobRecord` field [`JOB_RECORD_ANCHOR_FIELDS`] exempts. Every
/// other field still recurses — `payload` included, since a dispatch payload
/// CAN carry a scraped posting.
pub(super) const JOB_RECORD_RESULT_FIELD: &str = "result";

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
pub(super) const SCRAPE_SUMMARY_ANCHOR_FIELDS: [&str; 2] = ["board", "count"];

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
pub(super) const SCRAPE_SUMMARY_UNTRUSTED_FIELDS: [&str; 3] = ["error", "skipped", "truncated"];

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
pub(super) const BOARD_HEALTH_ANCHOR_FIELDS: [&str; 2] = ["status", "consecutiveFailures"];

/// The one board-written string on a [`BOARD_HEALTH_ANCHOR_FIELDS`]-detected
/// object. Its siblings are counters, epoch-ms timestamps, a derived status
/// enum and this app's own scrape `job_id` — none of them third-party text.
pub(super) const BOARD_HEALTH_UNTRUSTED_FIELDS: [&str; 1] = ["lastError"];

/// True when `map` is an `ai_generations::ApplicationAnswer`-shaped object:
/// a STRING [`APPLICATION_ANSWER_QUESTION_FIELD`] plus every
/// [`APPLICATION_ANSWER_ANCHOR_FIELDS`] key. Shared by the fence and the
/// unfence walk so the two can never disagree about the shape.
pub(super) fn is_application_answer_shaped(map: &serde_json::Map<String, Value>) -> bool {
    map.get(APPLICATION_ANSWER_QUESTION_FIELD)
        .is_some_and(Value::is_string)
        && APPLICATION_ANSWER_ANCHOR_FIELDS
            .iter()
            .all(|f| map.contains_key(*f))
}

/// Fence the board-written strings on `map` when its keys match either
/// scrape-diagnostics shape — [`SCRAPE_SUMMARY_ANCHOR_FIELDS`] →
/// [`SCRAPE_SUMMARY_UNTRUSTED_FIELDS`], [`BOARD_HEALTH_ANCHOR_FIELDS`] →
/// [`BOARD_HEALTH_UNTRUSTED_FIELDS`] — and nothing at all on any other
/// object. The two shapes are checked independently rather than nested: a
/// `BoardHealth` also reaches this surface standalone, on a
/// `BoardHealthEntry`, not only under a summary's `health`.
///
/// Shared by [`fence_named_fields_recursive`] (diagnostics anywhere OUTSIDE
/// a job result) and [`fence_scrape_summaries_recursive`] (the copies INSIDE
/// the otherwise-exempt one), so the two walks can never disagree about
/// either shape or either field set.
///
/// Fencing happens on this READ path rather than at the producer
/// (`commands::scrape::scrape_boards`, before `job_complete`) on purpose:
/// the very same strings are what the renderer's per-board chip strip
/// displays — `BoardSummaryChips` matches `skipped` against a controlled
/// vocabulary to pick a localized label, and renders `error`, `truncated`
/// and `health.lastError` as chip detail — reached both by the
/// `job.completed` event and, on remount, by the watchdog's own `jobs_get`.
/// A fence baked into the stored result would put `<job_posting>` markup on
/// screen and knock `skipped` out of every arm of that match; stripping it
/// back off in the renderer would mean a second, hand-maintained copy of
/// these field lists in TypeScript, on a path where a miss is visible to the
/// user.
pub(super) fn fence_board_derived_strings(map: &mut serde_json::Map<String, Value>) {
    for (anchors, fields) in [
        (
            SCRAPE_SUMMARY_ANCHOR_FIELDS.as_slice(),
            SCRAPE_SUMMARY_UNTRUSTED_FIELDS.as_slice(),
        ),
        (
            BOARD_HEALTH_ANCHOR_FIELDS.as_slice(),
            BOARD_HEALTH_UNTRUSTED_FIELDS.as_slice(),
        ),
    ] {
        if !anchors.iter().all(|f| map.contains_key(*f)) {
            continue;
        }
        for field in fields {
            if let Some(s) = map.get(*field).and_then(Value::as_str) {
                let fenced =
                    crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
                map.insert((*field).to_string(), json!(fenced));
            }
        }
    }
}

/// Fence every [`FENCE_FIELD_NAMES`] string (or string array element)
/// anywhere in `data`'s tree — recurses through the WHOLE response (not just
/// a top-level object/array, MEDIUM fix — security review round 1), and runs
/// UNCONDITIONALLY for every dispatched command rather than gating on a
/// command allowlist (HIGH fix — security review round 2): a new command
/// whose response embeds one of these EXACT field names is fenced
/// automatically, without needing an entry added here first. Also fences any
/// unclassified string field on a [`JOB_POSTING_ANCHOR_FIELDS`]-detected
/// object (HIGH fix — security review round 3), closing the residual gap a
/// field-name allowlist alone cannot: `JobPosting.extra`'s board-chosen keys.
/// See `every_known_posting_text_carrier_is_a_real_freely_
/// dispatchable_policy_row` (tests) for the audited list of rows this is
/// known to protect.
///
/// Some rules are keyed on an object's SHAPE rather than a field name,
/// because a name alone cannot tell two carriers apart:
/// [`APPLICATION_ANSWER_ANCHOR_FIELDS`] fences a scraped ATS `question`
/// without touching `InterviewQuestion.question`;
/// [`JOB_RECORD_ANCHOR_FIELDS`] exempts a job's own `result` so a generation
/// read back through `jobs_get` is not labelled as scraped posting text; and
/// [`SCRAPE_SUMMARY_ANCHOR_FIELDS`]/[`BOARD_HEALTH_ANCHOR_FIELDS`] fence the
/// board-WRITTEN strings on a `BoardScrapeSummary`/`BoardHealth` without
/// touching this app's own same-named `error` strings — including inside
/// that exempt `result`, which is where a completed `scrape_boards` job puts
/// them.
pub(super) fn fence_scraped_fields(data: &mut Value) {
    fence_named_fields_recursive(data);
}

/// Walk every object/array in `value`, fencing any [`FENCE_FIELD_NAMES`]
/// STRING key (or string element of an ARRAY under one of those keys)
/// wherever one appears, then — on an object [`JOB_POSTING_ANCHOR_FIELDS`]
/// marks as a real `JobPosting` — every OTHER string-valued key not in
/// [`JOB_POSTING_SAFE_FIELDS`] (the flattened `extra` catch-all). See
/// [`fence_scraped_fields`]'s doc for why this is recursive and
/// unconditional.
///
/// Then the shape rules: on an [`APPLICATION_ANSWER_ANCHOR_FIELDS`]-
/// detected object the [`APPLICATION_ANSWER_QUESTION_FIELD`] string is
/// fenced (a scraped ATS question label whose wire key is shared with this
/// app's own `InterviewQuestion.question`), on a scrape-diagnostics object
/// [`fence_board_derived_strings`] fences the board-written keys, and on a
/// [`JOB_RECORD_ANCHOR_FIELDS`]-detected object the recursion hands
/// [`JOB_RECORD_RESULT_FIELD`] to [`fence_scrape_summaries_recursive`]
/// instead of walking it (a job's own output, not scraped text — except for
/// the diagnostics a scrape completes with).
pub(super) fn fence_named_fields_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            // Issue #1157 -- origin shape checks, computed up front (read-only) so the loop
            // below and the dedicated `text` block after it can both use them without
            // re-deriving or risking the two disagreeing. See `DOCUMENT_RECORD_ANCHOR_FIELDS`/
            // `RESUME_EXTRACT_TEXT_ANCHOR_FIELD`/`CHANGELOG_ENTRY_ANCHOR_FIELDS`'s own docs.
            //
            // `job_posting_shaped` is hoisted up here too (security review round A3-r1, AC-3
            // MEDIUM) rather than computed only later where the `extra`-catch-all needs it: a
            // board-controlled `JobPosting.extra` map (`#[serde(flatten)]`) could otherwise ALSO
            // satisfy `document_record_shaped` by forging `isDefault`+`indexed` keys into it, and
            // nothing before this fix stopped a real `JobPosting` from taking the DocumentRecord
            // exemption below. ANDing every DocumentRecord-shaped check with `!job_posting_shaped`
            // makes that impossible: a real `JobPosting` always fails the AND, so its `title`
            // fences exactly as it always did.
            let job_posting_shaped = JOB_POSTING_ANCHOR_FIELDS
                .iter()
                .all(|f| map.contains_key(*f));
            let document_record_shaped = !job_posting_shaped
                && DOCUMENT_RECORD_ANCHOR_FIELDS
                    .iter()
                    .all(|f| map.contains_key(*f));
            let user_document_shaped =
                document_record_shaped || map.contains_key(RESUME_EXTRACT_TEXT_ANCHOR_FIELD);
            let changelog_entry_shaped = CHANGELOG_ENTRY_ANCHOR_FIELDS
                .iter()
                .all(|f| map.contains_key(*f));

            for field in FENCE_FIELD_NAMES {
                // `title` on a `DocumentRecord`-shaped object is the user's own first-party
                // file title, not a board-scraped job title -- skip the default fence. Gated on
                // `document_record_shaped` alone, which already excludes a `JobPosting` (see
                // above).
                if *field == "title" && document_record_shaped {
                    continue;
                }
                // `body` on a changelog-entry-shaped object is this repo's own first-party
                // release notes -- skip the default fence.
                if *field == "body" && changelog_entry_shaped {
                    continue;
                }
                if let Some(s) = map.get(*field).and_then(Value::as_str) {
                    let fenced =
                        crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
                    map.insert((*field).to_string(), json!(fenced));
                    continue;
                }
                if let Some(Value::Array(items)) = map.get_mut(*field) {
                    for item in items.iter_mut() {
                        if let Value::String(s) = item {
                            *s = crate::prompt_fence::fenced(
                                "job_posting",
                                s,
                                crate::prompt_fence::JOB_CAP,
                            );
                        }
                    }
                }
            }
            // `text` (issue #1157) -- origin-aware, never a flat `FENCE_FIELD_NAMES` entry: the
            // user's OWN document text (`documents::DocumentRecord.text`/`resume_extract_text`'s
            // reply) is fenced under the DISTINCT `user_document` tag; every other producer on
            // this surface (`commands::profile_import::profile_import_from_url`'s response also
            // carries a bare `text` key, but it is resume text rendered from a THIRD-PARTY
            // imported profile page, not the user's own file) keeps the ORIGINAL `job_posting`
            // default -- a shape miss must stay fenced, never fall open.
            if let Some(s) = map.get("text").and_then(Value::as_str) {
                let (tag, cap) = if user_document_shaped {
                    ("user_document", crate::prompt_fence::RESUME_CAP)
                } else {
                    ("job_posting", crate::prompt_fence::JOB_CAP)
                };
                let fenced = crate::prompt_fence::fenced(tag, s, cap);
                map.insert("text".to_string(), json!(fenced));
            }
            // `title`/`name` (security review round A3-r1, SEC-4 MEDIUM): a `DocumentRecord`'s
            // `title` is exempted from the default fence above, and its `name` was never on
            // [`FENCE_FIELD_NAMES`] at all -- both are agent-WRITABLE (`documents_import`) and
            // agent-READABLE strings that, unlike every other exempted first-party field on this
            // surface, had NO cap and NO boundary defence left at all. Neutralize + cap without a
            // tag (the same treatment `agent_read::found_jobs::cap_autopilot_name` gives an
            // autopilot's own name): the first-party voice stays unlabelled, but a stored
            // prompt-injection payload can no longer forge a transcript boundary or grow
            // unbounded through this channel.
            if document_record_shaped {
                for name_field in ["title", "name"] {
                    if let Some(s) = map.get(name_field).and_then(Value::as_str) {
                        let capped: String = s.chars().take(crate::prompt_fence::JOB_CAP).collect();
                        let capped = crate::prompt_fence::neutralize_transcript_boundaries(&capped);
                        map.insert(name_field.to_string(), json!(capped));
                    }
                }
            }
            if job_posting_shaped {
                // ADVISORY fix (security review round 4): used to filter on
                // `v.is_string()` alone, so a board-chosen `extra` key whose
                // value is an ARRAY or OBJECT (not reachable today — every
                // `extra.insert` call site writes a scalar, verified — but
                // not reachable is not the same as impossible for the FIRST
                // board that adds one) skipped this catch-all entirely: not
                // a listed field name, not string-typed, so neither this
                // block nor the array-only handling above touches it, and
                // the generic recursive walk below only fences NAMED fields,
                // never "every string inside an unclassified value". Every
                // non-null, non-safe, non-listed key is now collected
                // regardless of shape; a String is fenced directly as
                // before, an Array/Object is fenced leaf-by-leaf via
                // `fence_all_string_leaves` (untrusted board data all the
                // way down, not just at the top level).
                let extra_keys: Vec<String> = map
                    .iter()
                    .filter(|(k, v)| {
                        !v.is_null()
                            && !FENCE_FIELD_NAMES.contains(&k.as_str())
                            // `text` is no longer on `FENCE_FIELD_NAMES` (issue #1157 -- it is
                            // fenced by its own origin-aware block above this shape check,
                            // unconditionally); excluded here too so a `JobPosting`'s own `text`
                            // key (if a board ever added one to its `extra`) is never fenced
                            // TWICE under two different tags.
                            && *k != "text"
                            && !JOB_POSTING_SAFE_FIELDS.contains(&k.as_str())
                    })
                    .map(|(k, _)| k.clone())
                    .collect();
                for key in extra_keys {
                    if let Some(v) = map.get_mut(&key) {
                        match v {
                            Value::String(s) => {
                                *s = crate::prompt_fence::fenced(
                                    "job_posting",
                                    s,
                                    crate::prompt_fence::JOB_CAP,
                                );
                            }
                            Value::Array(_) | Value::Object(_) => fence_all_string_leaves(v),
                            _ => {}
                        }
                    }
                }
            }
            // Shape-guarded, never a name entry — see
            // [`APPLICATION_ANSWER_ANCHOR_FIELDS`] for why putting
            // `question` on [`FENCE_FIELD_NAMES`] would have re-fenced
            // `InterviewQuestion.question`. Skipped on a `JobPosting`-shaped
            // object: the catch-all above already fenced every unclassified
            // string there, and fencing twice would leave a wrapper behind
            // after [`unfence_named_fields_recursive`]'s single strip.
            if !job_posting_shaped && is_application_answer_shaped(map) {
                if let Some(question) = map
                    .get(APPLICATION_ANSWER_QUESTION_FIELD)
                    .and_then(Value::as_str)
                {
                    let fenced = crate::prompt_fence::fenced(
                        "job_posting",
                        question,
                        crate::prompt_fence::JOB_CAP,
                    );
                    map.insert(APPLICATION_ANSWER_QUESTION_FIELD.to_string(), json!(fenced));
                }
            }
            // The scrape-diagnostics shape rules, under the same
            // `!job_posting_shaped` guard and for the same reason: on a
            // `JobPosting`-shaped object the
            // `extra` catch-all above already fenced every unclassified
            // string, and `fenced` does NOT guard against double-wrapping
            // (nor does `fence_all_string_leaves`), so a second pass would
            // leave a wrapper behind after
            // [`unfence_named_fields_recursive`]'s single strip. Reached by
            // `Autopilot.last_run_summaries`; the copies inside a
            // `JobRecord`'s exempt `result` are handled below.
            if !job_posting_shaped {
                fence_board_derived_strings(map);
            }
            // A `JobRecord`'s own `result` is the app's OWN output, not
            // scraped text — see [`JOB_RECORD_ANCHOR_FIELDS`]. The exemption
            // is on the RECURSION only: every other field of this object,
            // and every other object in the tree, walks as before.
            let job_record_shaped = JOB_RECORD_ANCHOR_FIELDS
                .iter()
                .all(|f| map.contains_key(*f));
            for (key, v) in map.iter_mut() {
                if job_record_shaped && key.as_str() == JOB_RECORD_RESULT_FIELD {
                    // The exemption is wholesale for the NAME-keyed walk, and
                    // stays that way — but `scrape_boards` completes with
                    // `BoardScrapeSummary` rows, so a diagnostics shape does
                    // carry third-party text in here. Fence only those
                    // enumerated keys and nothing else in the subtree; see
                    // [`SCRAPE_SUMMARY_ANCHOR_FIELDS`].
                    fence_scrape_summaries_recursive(v);
                    continue;
                }
                fence_named_fields_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_named_fields_recursive(item);
            }
        }
        _ => {}
    }
}

/// Walk `value` applying ONLY [`fence_board_derived_strings`] — the single
/// carve-out inside a `JobRecord`'s otherwise-exempt
/// [`JOB_RECORD_RESULT_FIELD`]. Deliberately NOT
/// [`fence_named_fields_recursive`]: running the name-keyed walk in here
/// would re-open the exact defect the exemption exists to close (a
/// generation's `{"done": true, "text": …}` labelled as a scraped posting).
/// A scrape summary and its board health are fenced; everything else in the
/// subtree is left exactly as the producer wrote it.
pub(super) fn fence_scrape_summaries_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            fence_board_derived_strings(map);
            for v in map.values_mut() {
                fence_scrape_summaries_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_scrape_summaries_recursive(item);
            }
        }
        _ => {}
    }
}

/// Fence every STRING found anywhere inside `value`, unconditionally — no
/// field-name gate, unlike [`fence_named_fields_recursive`]. Used only for a
/// value already known to be untrusted board data by virtue of its
/// LOCATION (an unclassified key under a detected `JobPosting`'s flattened
/// `extra`), so every string it contains, at any depth, is untrusted too —
/// the board chose the keys, so a name-based allowlist can never enumerate
/// them.
pub(super) fn fence_all_string_leaves(value: &mut Value) {
    match value {
        Value::String(s) => {
            *s = crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_all_string_leaves(item);
            }
        }
        Value::Object(map) => {
            for v in map.values_mut() {
                fence_all_string_leaves(v);
            }
        }
        _ => {}
    }
}

//! The audited field-NAME tables [`super::named_fields::fence_named_fields_recursive`] walks by
//! key — see that fn's own doc for how they are used; split out under the R8 LOC cap.

/// Response field NAMES that can carry raw, third-party-authored SCRAPED JOB TEXT — audited by
/// hand against the struct each name actually serializes from (mirrors `policy`'s own per-row
/// audit discipline). Keyed by FIELD NAME rather than by command: a command allowlist misses every
/// command whose response embeds one of these structs under this same key. Every entry routes
/// through [`crate::prompt_fence::fenced`] — the SAME primitive, tag, and cap
/// `agent_read::fence_description` uses for the curated `job` resource, so a scraped posting reads
/// as untrusted DATA on every surface it reaches. See `every_known_posting_text_carrier_is_a_real_
/// freely_dispatchable_policy_row` (tests) for the audited list of rows this is known to protect.
///
/// Built up across four security-review rounds (each missing what the last one covered, issue
/// #1157/#1183) — kept here as a record of the SHAPE of the gap, not merely the fix: round 2 keyed
/// this list by field name instead of command, catching `Autopilot.found_jobs[].description`/
/// `Application.job_description`/`AiGenerationRecord.job_ad`. Round 3 added `title`/`company`/
/// `location`/`requirements` (`JobPosting`/`FoundJob` carry these too, equally third-party — a
/// posting titled "Ignore prior instructions…" reached the caller unfenced; `requirements` is an
/// array, fenced element-by-element). Round 4 found that a flat name list also misses a
/// serde-RENAMED field carrying the SAME posting data under a different key — `AiGenerationRecord.
/// job_title`/`.company_name`/`.top_requirements` (copied forward from the source posting, not
/// re-derived), `discovered::DiscoveredCompany.display_name` (board-harvested from an apply-redirect
/// URL), plus the generic `text`/`body` carriers (`documents::DocumentRecord.text`,
/// `notifications::AppNotification.body`) — a résumé counts as untrusted for this purpose too
/// (`agent-cli-standards`: ~1% of a 200k-résumé corpus carried a prompt injection). Each round's
/// fixture-driven exhaustive check (build from `serde_json::to_value(RealStruct{..})`, never a
/// hand-typed literal) is what catches the NEXT gap instead of continuing to hand-guess names.
///
/// Issue #1157 then fixed the opposite failure: fencing by NAME ALONE over-fenced every first-party
/// carrier of `title`/`body`/`text` too (a user's own `DocumentRecord.title`, `updater_changelog`'s
/// own release notes `body`, a résumé's own `text`). Fixed by SHAPE, not by removing the name (that
/// would un-fence the still-third-party carriers of the same name):
/// [`DOCUMENT_RECORD_ANCHOR_FIELDS`] exempts `title` for a `DocumentRecord`-shaped object,
/// [`CHANGELOG_ENTRY_ANCHOR_FIELDS`] exempts `body` for a changelog entry, and `text` was removed
/// from this flat list entirely — handled by its own origin-aware block in
/// [`fence_named_fields_recursive`] under the distinct `user_document` tag (see
/// [`RESUME_EXTRACT_TEXT_ANCHOR_FIELD`]), keeping `job_posting` everywhere else.
/// `AppNotification.title`/`.body` stay on this flat list deliberately (not an oversight): a
/// notification's copy is genuinely MIXED provenance (`tray::on_new_jobs`'s own name+count is
/// first-party, but `status_update`/`import_flow`/`follow_up_body`/`resume_pipeline::notify` all
/// embed a real scraped title/company under the identical wire shape), so it stays fenced by
/// default rather than risk unfencing the scraped half.
///
/// `ApplicationAnswer.question` is fenced too, but by SHAPE and never by name — see
/// [`APPLICATION_ANSWER_ANCHOR_FIELDS`] for why a flat entry would have re-fenced
/// `InterviewQuestion.question`, which shares the wire key and is this app's own AI output.
///
/// Deliberately NOT added, on purpose, not by omission: `AiGenerationRecord.resume_text`/
/// `.cover_letter_text`/`.company_brief`/`.candidate_name`/`.email_subject`/`.email_body` and
/// `InterviewQuestion`/`ApplicationAnswer`'s own fields — user PII / this app's own AI output, a
/// SEPARATE axis from fencing per ADR-038 §5 (this module's own doc above).
pub(in crate::extension_bridge::agent_call) const FENCE_FIELD_NAMES: &[&str] = &[
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
pub(in crate::extension_bridge::agent_call) const JOB_POSTING_ANCHOR_FIELDS: [&str; 2] =
    ["capturedAt", "source"];

/// Structural `JobPosting` fields that are identifiers/URLs/timestamps,
/// never third-party PROSE — every OTHER string value on a
/// [`JOB_POSTING_ANCHOR_FIELDS`]-detected object is untrusted (flattened
/// `extra`, or a future field this file doesn't yet name by hand).
pub(in crate::extension_bridge::agent_call) const JOB_POSTING_SAFE_FIELDS: &[&str] = &[
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
pub(in crate::extension_bridge::agent_call) const DOCUMENT_RECORD_ANCHOR_FIELDS: [&str; 2] =
    ["isDefault", "indexed"];

/// `commands::match_resume::resume_extract_text`'s own response shape (`{"text","confidence"}`)
/// -- `confidence` is this dispatch surface's ONLY producer of that wire key (verified), so its
/// presence alongside a `text` string is enough to detect the shape without a command-name
/// special case, the same convention every other anchor pair on this file uses.
pub(in crate::extension_bridge::agent_call) const RESUME_EXTRACT_TEXT_ANCHOR_FIELD: &str =
    "confidence";

/// `updater::updater_changelog`'s own response shape (`{"version","name","body","publishedAt",
/// "url","prerelease"}` per release) -- `publishedAt`/`prerelease` are this dispatch surface's
/// only producers of that pair (verified: `scraping::boards::ashby`'s own `publishedAt` field is
/// `Deserialize`-only, parsing the board's OWN API response, and never reaches this surface's
/// wire). `body` here is the repo's own bundled `CHANGELOG.md` prose -- first-party release
/// notes, never board-scraped or user-authored -- so it is exempted from the default `body`
/// fence (issue #1157); every OTHER `body` carrier (`notifications::AppNotification.body` above
/// all) stays fenced by default, since that field really is mixed -- see `FENCE_FIELD_NAMES`'s
/// own doc addendum.
pub(in crate::extension_bridge::agent_call) const CHANGELOG_ENTRY_ANCHOR_FIELDS: [&str; 2] =
    ["publishedAt", "prerelease"];

/// `notifications::AppNotification`'s own always-present pair (`read: bool` is this dispatch
/// surface's ONLY producer of that wire key, verified; paired with `createdAt` for the same
/// two-field discipline every other anchor on this file uses) -- used to detect a
/// `notifications_list` row so its `title`/`body` are fenced under the DISTINCT
/// `crate::prompt_fence`-registered `app_notification` tag instead of `job_posting`
/// (A3-r2-AC-7 MEDIUM). This is deliberately NOT an exemption the way
/// `DOCUMENT_RECORD_ANCHOR_FIELDS`/`CHANGELOG_ENTRY_ANCHOR_FIELDS` are -- `FENCE_FIELD_NAMES`'s
/// own doc addendum already explains why a notification's copy is genuinely MIXED provenance
/// (`tray::on_new_jobs`'s own first-party name+count vs. `extension_bridge::status_update`'s
/// `display_name`, a real scraped job title, riding the exact same wire key) -- so it must stay
/// fenced as untrusted DATA either way. The only thing that changes is the LABEL: `job_posting`
/// asserts third-party board authorship the way #1157's own remedy for a mixed-provenance field
/// says not to claim for a first-party-in-the-common-case string.
pub(in crate::extension_bridge::agent_call) const NOTIFICATION_ANCHOR_FIELDS: [&str; 2] =
    ["createdAt", "read"];

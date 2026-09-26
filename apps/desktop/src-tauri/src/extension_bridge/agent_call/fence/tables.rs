//! The audited field-NAME tables [`super::named_fields::fence_named_fields_recursive`] walks by
//! key — see that fn's own doc for how they are used; split out under the R8 LOC cap.

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
/// under `documents_list`'s rows — `documents_get_text` returns the SAME
/// text as a bare string reply with no key at all, which this name-keyed
/// walk structurally cannot see; `reshape::SCALAR_FENCE_COMMANDS` is the
/// separate fence for that shape (issue #1170's follow-up,
/// `B1-r1-ACLI-R5-7`) — and
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

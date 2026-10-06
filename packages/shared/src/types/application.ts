// ─── Application tracking (ADR docs/knowledge/decision-records/0001-application-aggregate-split.md) ────
//
// An **Application** is the status-bearing aggregate root for a job pursuit (the
// single source of truth for "am I pursuing this, and how far along"). An
// `AiGenerationRecord` is its child Document. NOTE: this is distinct from the
// async-exec `JobStatus` in `./index.ts` — do NOT conflate the two.

/**
 * The ordered stage registry — the SINGLE source of truth the Rust
 * `ApplicationStatus` enum mirrors (a Rust parity test pins the id list/order).
 * Each entry: `id`, whether it is `terminal` (closed, would not normally reopen
 * — `ghosted` is intentionally NOT terminal: it is soft/reopenable), and whether
 * it is `preApply` (the user has not applied yet — only `saved`).
 */
export const APPLICATION_STAGES = [
  { id: 'saved', terminal: false, preApply: true },
  { id: 'applied', terminal: false, preApply: false },
  { id: 'screening', terminal: false, preApply: false },
  { id: 'interviewing', terminal: false, preApply: false },
  { id: 'offer', terminal: false, preApply: false },
  { id: 'accepted', terminal: true, preApply: false },
  { id: 'rejected', terminal: true, preApply: false },
  { id: 'ghosted', terminal: false, preApply: false },
  { id: 'withdrawn', terminal: true, preApply: false },
] as const;

/** The application lifecycle status union, derived from {@link APPLICATION_STAGES}. */
export type ApplicationStatus = (typeof APPLICATION_STAGES)[number]['id'];

/** One answered application question carried on the Application aggregate. */
export interface ApplicationAnswer {
  id: string;
  question: string;
  answer: string;
}

/**
 * One AI-suggested question the candidate can ASK the interviewer — distinct from
 * {@link ApplicationAnswer} (which the candidate answers). Persisted on the per-job
 * aiGenerations aggregate.
 */
export interface InterviewQuestion {
  id: string;
  question: string;
  /** Why this question lands well / what it signals to the interviewer. */
  why: string;
  /** Target interviewer — `recruiter` | `hiringManager` | `team` | `leadership` |
   *  `general` (open-typed; an unknown value is treated as `general`). */
  audience: string;
}

/** The Application aggregate root. */
export interface Application {
  id: string;
  status: ApplicationStatus;
  /** First time the status left `saved` (ms). Absent while still `saved`. */
  appliedAt?: number;
  createdAt: number;
  updatedAt: number;
  /** Normalized job URL — the dedup key. Empty for a link-less manual pursuit. */
  jobUrl: string;
  board: string;
  company: string;
  title: string;
  candidate: string;
  answers: ApplicationAnswer[];
  brief: string;
  notes: string;
  /** User-set reminder timestamp (ms) for the next action. Absent = unset. */
  nextActionAt?: number;
  /** Backend bookkeeping: when the follow-up notification for the CURRENT
   *  {@link Application.nextActionAt} was already raised (ms). Absent = not yet
   *  announced; the backend clears it whenever the due date moves.
   *
   *  Not for display — it exists on the wire so a backup round trip preserves it
   *  (`export`/`import` serialize this aggregate; dropping it made restoring a
   *  backup re-announce every reminder the user had already seen). The renderer
   *  must not write it: `update()` has no such field. */
  nextActionNotifiedAt?: number;
  comp: string;
  /** **The** primary contact for this pursuit (recruiter / hiring manager /
   *  apply-by-email recipient — one person, one field). Canonical: the only
   *  contact pair the backend stores. */
  contactName: string;
  /** Canonical primary contact email — see {@link Application.contactName}. */
  contactEmail: string;
  /** The imported/pasted job description (from the captured DOM at import, or a
   *  later manual paste / retry-resolve). Empty when unknown. */
  jobDescription: string;
  /** Persisted AI-generated job-ad summary (server-capped at 50 KB). */
  jobSummary: string;
  /** @deprecated Alias of {@link Application.contactName}. Always present and
   *  always equal to `contactName` — the backend mirrors the canonical value
   *  onto this name so pre-unification callers keep working. Read
   *  `contactName`; new UI should not use this. */
  recipientName?: string;
  /** @deprecated Alias of {@link Application.contactEmail} — see
   *  {@link Application.recipientName}. */
  recipientEmail?: string;
  /** Scraped salary range (Adzuna only, today) — grounds the salary application
   *  answer before it falls back to a web lookup. Absent when unknown. */
  salaryMin?: number;
  salaryMax?: number;
  /** ISO-4217 currency for `salaryMin`/`salaryMax`. */
  salaryCurrency?: string;
}

/**
 * Known {@link StatusEvent.source} values — a COMPARISON SET for consumers to
 * check against, not a closed union (`source` stays a free-form `string` on
 * purpose; see its doc). An unrecognised value must fall through to normal
 * rendering, never crash or be mistaken for one of these.
 *
 * The Rust mirror is `EVENT_SOURCE_USER`/`EVENT_SOURCE_EMAIL`/
 * `EVENT_SOURCE_EMAIL_REJECT` in
 * `apps/desktop/src-tauri/src/applications/status_events.rs` — `pub(crate)`,
 * so it has no shared export and duplicates these literals by hand. That Rust
 * file pins its own constants against these same strings in a test, so a
 * rename on either side fails loudly instead of silently disabling the
 * auto-write adjudication step (an unconfirmed row is the whole safety model
 * for a classifier with a recorded precision limit — losing the Accept/Reject
 * affordance without a test failure would be the worst available outcome).
 * `status-event.test.ts` pins the TS side the same way.
 */
export const EVENT_SOURCE_USER = 'user';
export const EVENT_SOURCE_EMAIL = 'email';
export const EVENT_SOURCE_EMAIL_REJECT = 'email_reject';

/** One append-only status-history row. */
export interface StatusEvent {
  /** SQLite's implicit `rowid` for this row — the only stable per-row
   *  identity `status_events` has (no declared primary key). Required by
   *  {@link ApplicationsContract.acceptStatusEvent}/
   *  {@link ApplicationsContract.rejectStatusEvent} to target the EXACT row
   *  being actioned: two provisional rows can coexist on the ordinary happy
   *  path (a confirmation email, then a later rejection email, both still
   *  unreviewed), and resolving "the pending row" by recency alone let a
   *  click on the OLDER row's Accept/Reject button silently act on the
   *  NEWER, unrelated one instead. Always pass the `eventId` of the SPECIFIC
   *  row the button was rendered on, never a cached/stale value from a
   *  different row. */
  eventId: number;
  applicationId: string;
  /** Empty for the seed event of a freshly-created Application. */
  fromStatus: string;
  toStatus: string;
  at: number;
  note: string;
  /** Who/what asserted this transition: {@link EVENT_SOURCE_USER} (every
   *  pre-v2 row, and every user-driven write today) or
   *  {@link EVENT_SOURCE_EMAIL}/{@link EVENT_SOURCE_EMAIL_REJECT} (v2
   *  auto-write and its reversal — see {@link StatusEvent.confirmed}). A
   *  free-form string, not a closed union, so a future source needs no
   *  client-side change — the exported constants above are a comparison set,
   *  not an exhaustive type. */
  source: string;
  /** Whether a human has reviewed this transition. Every pre-v2 row (and every
   *  `'user'`-sourced write) is `true`. An email-derived write ALWAYS lands
   *  `false` — the timeline renders it as PROVISIONAL with Accept/Reject
   *  affordances; nothing in this app ever auto-writes `true`. */
  confirmed: boolean;
}

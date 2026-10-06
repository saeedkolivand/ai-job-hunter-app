import { z } from 'zod';

import { WORK_TYPE_OPTIONS } from './scraping.js';

// ─── Autopilot schemas ────────────────────────────────────────────────────────

export const AutopilotTargetSchema = z.object({
  // Free-text (not `BoardId`-typed) deliberately, so a saved target with a since-
  // retired board id still deserializes. The 64 ceiling is just a generous sanity
  // bound against a corrupt/hostile autopilots.json — the real bound is the Rust
  // engine's server-side registry dedup+truncate (`max_boards_per_batch()`), so
  // this never needs to change as the board catalog grows.
  boards: z.array(z.string().min(1)).min(1).max(64),
  query: z.string().min(1),
  location: z.string().optional(),
  // ISO 3166-1 alpha-2 (sourced from the same geocode suggestion as the manual
  // search); validated here so a malformed value can't propagate to scraping.
  countryCode: z
    .string()
    .trim()
    .regex(/^[A-Za-z]{2}$/)
    .optional(),
  // Requested work arrangement(s) — same `WORK_TYPE_OPTIONS` vocabulary the
  // manual search filter uses. Multi-select; empty/absent = no filter. Same
  // `.max` CWE-770 cap as `ScrapeBoardsRequestSchema.workTypes` above.
  workTypes: z.array(z.enum(WORK_TYPE_OPTIONS)).max(WORK_TYPE_OPTIONS.length).optional(),
  pages: z.number().int().min(1).max(10).default(2),
  dateFilter: z.string().optional(),
  // Watched-companies-only mode (ADR-030 §e): when true, a run resolves the
  // user's currently-starred discovered companies at run time and scrapes only
  // those per-ATS company slugs (instead of the curated seed). Additive +
  // optional so old autopilots deserialize unchanged.
  watchedCompaniesOnly: z.boolean().optional(),
});

export const AutopilotFilterSchema = z.object({
  // Default 0 = keep everything. A non-zero default silently dropped jobs a
  // manual search would have returned (the autopilot zero-jobs bug); the user
  // raises this deliberately. Drives both create + update generated Rust
  // defaults (update reuses this schema via `.partial()`).
  minMatchScore: z.number().min(0).max(100).default(0),
  keywords: z.array(z.string()).optional(),
  excludeKeywords: z.array(z.string()).optional(),
});

export const AutopilotCreateSchema = z.object({
  name: z.string().min(1).max(100),
  target: AutopilotTargetSchema,
  filter: AutopilotFilterSchema,
  schedule: z.enum(['manual', 'hourly', 'daily', 'twice_daily']),
  // Local clock time a recurring schedule fires at. `scheduleHour` drives
  // daily/twice_daily (ignored by hourly); `scheduleMinute` drives both those
  // and the "minute past the hour" for hourly. Defaults applied in Rust when
  // absent (09:00 for daily/twice_daily, minute 0 for hourly).
  scheduleHour: z.number().int().min(0).max(23).optional(),
  scheduleMinute: z.number().int().min(0).max(59).optional(),
  resumeText: z.string().optional(),
  // Optional base cover letter — reused as the starting point when tailoring a
  // found job in the apply assistant. (Auto-apply was removed; this field is a
  // reusable template, not an instruction to submit anything.)
  coverLetter: z.string().optional(),
  // Phase 4 (opt-in): attach a short AI-reasoned note to the top matches of each
  // scheduled run. Read-only enrichment — never applies or submits anything.
  // Optional (like the other autopilot fields); the Rust store owns the `false`
  // default, so absent → notes off and existing autopilots stay note-free.
  assistant: z.boolean().optional(),
  // Provider snapshot for the headless AI-notes call. The scheduler runs with no
  // renderer, so the active provider/model/base URL resolved at opt-in time is
  // persisted here; the run then resolves the SAME centralized provider layer
  // (`Completer`) that `ai_generate` uses. Absent/empty → notes skip gracefully.
  assistantProvider: z.string().optional(),
  assistantModel: z.string().optional(),
  assistantBaseUrl: z.string().optional(),
});

export const AutopilotUpdateSchema = AutopilotCreateSchema.partial().extend({
  status: z.enum(['active', 'paused', 'archived']).optional(),
});

export const AutopilotIdSchema = z.object({ autopilotId: z.string().min(1) });

export const TechStackItemSchema = z.object({
  name: z.string().min(1),
  category: z.string().min(1),
});

// Payload of both `jobPreferences.get` and `jobPreferences.set`. The setter
// MERGES over the stored row: an OMITTED key keeps its stored value, a key sent
// as explicit `null` CLEARS that column. So `null` is the only way a caller can
// clear a field — `undefined` is not a wire value (`JSON.stringify` drops the
// key, which the merge reads as "leave it alone"), which is why the fields the
// renderer clears are `.nullish()` rather than `.optional()`. Reads never carry
// `null`: the Rust struct skips serializing its unset fields.
export const JobPreferencesSchema = z.object({
  location: z.string().nullish(),
  // ISO 3166-1 alpha-2, captured alongside `location` from a picked geocode
  // suggestion (mirrors AutopilotTargetSchema.countryCode) — lets a seeded
  // location carry its real country instead of a scraper having to guess one.
  // Cleared together with `location`: it describes that location and must never
  // outlive it.
  countryCode: z
    .string()
    .trim()
    .regex(/^[A-Za-z]{2}$/)
    .nullish(),
  techStack: z.array(TechStackItemSchema).optional(),
  // Backend-readable copy of the renderer's own `applicant.salaryExpectation`
  // (Task #30) — free text, no client-side length cap; the Rust store clamps
  // it (~200 bytes) at the write boundary, matching every other
  // renderer-supplied string in this contract.
  salaryExpectation: z.string().optional(),
  // Extra recruiting/staffing agency company names, merged with the built-in
  // const list when cross-board dedup flags a posting's `isAgency` (ADR-029 §i).
  // Free text; the Rust store clamps each entry + the list length at the write
  // boundary (per the dedicated single-column setter, PR #695 pattern). `.max`
  // mirrors the Rust `MAX_EXTRA_AGENCY_COMPANIES` cap (same guard as otherKeys).
  extraAgencyCompanies: z.array(z.string().trim().min(1)).max(500).optional(),
});

export type AutopilotCreate = z.infer<typeof AutopilotCreateSchema>;
export type AutopilotUpdate = z.infer<typeof AutopilotUpdateSchema>;
export type JobPreferences = z.infer<typeof JobPreferencesSchema>;

# ADR-011: Referral helper is manual-only; no LinkedIn profile scraping

Last updated: 2026-06-04

**Status:** Accepted

## Context

F3a adds a referral helper to the autopilot apply flow: the user records a contact
(name, company, role, relationship strength) and the app generates a referral note in
three formats (email, LinkedIn message, connection-note ≤ 300 chars) using
`buildReferralPrompt` / `generateReferral`.

An earlier F3b design considered **automatically scraping the contact's LinkedIn profile**
to pre-fill contact details and tailor the generated note (mutual connections, recent
activity, shared interests).

## Decision

**F3b (LinkedIn profile scraping) is permanently dropped.** All referral contact details
are entered manually by the user. The AI generation step receives only what the user
explicitly provides.

Two blockers ruled out automated scraping:

1. **ToS** — LinkedIn's User Agreement prohibits automated data collection from its
   platform without express written permission. Scraping profile pages in a desktop app
   would violate those terms and expose the project to legal action.

2. **GDPR lawful basis / transparency** — Silently harvesting a third party's profile data
   (name, employer, role, activity) without their knowledge removes the transparency and
   consent mechanisms that GDPR requires. The data subject (the contact) has no visibility
   into the collection and no way to exercise access/erasure rights against a local store
   they do not know exists.

Manual entry keeps the referral store within the user's own agency: they record only what
they already know about their contact, which is information they hold legitimately.

## Consequences

- **No `chromiumoxide` / browser-automation usage** for referral data. The `referrals`
  module is pure SQLite CRUD with no network egress.
- **No new scraping surface to maintain.** LinkedIn's DOM structure changes frequently;
  eliminating the scraping path removes a class of brittle selectors and rate-limit risk.
- **User owns the data.** Every field in a referral record was typed by the user; no
  third-party profile data is ingested, stored, or transmitted.
- **F3b may be revisited only** if LinkedIn publishes an official API with a lawful-basis
  pathway (user-delegated OAuth on behalf of the contact, with explicit consent). Any
  future implementation must route through `net::http::shared()` (not a browser scraper)
  and go through the `tauri-security-reviewer` gate.

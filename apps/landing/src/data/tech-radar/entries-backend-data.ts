import type { TechRadarEntry } from './types';

export const backendDataEntries: readonly TechRadarEntry[] = [
  // ── Rust Core & Data ────────────────────────────────────────────────────
  {
    id: 'rusqlite',
    name: 'SQLite via rusqlite (bundled)',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'rusqlite',
    summary: 'The whole local-first store — no external DB process.',
    rationale:
      "rusqlite's bundled feature ships SQLite inside the binary, so there's no version mismatch or separate database process to install. Seven independent databases (documents, conversations, ai_generations, job_preferences, contact_profile, jobs, pipeline_cache) keep faults isolated — a corrupt conversations.db doesn't touch documents.db.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'tokio',
    name: 'Tokio',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'tokio',
    summary: 'Async runtime for board scraping and background jobs.',
    rationale:
      'Board scrapers run concurrently via tokio::spawn, each holding a CancellationToken so a scrape can be stopped mid-run; long operations are tracked by a SQLite-backed job tracker with retry rather than blocking command handling.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'keyring-core',
    name: 'keyring-core (OS-native credential storage)',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'keyring-core',
    summary: 'API keys and board passwords never touch the renderer.',
    rationale:
      "Credentials live in the OS keychain (Credential Manager/DPAPI on Windows, Keychain on macOS, libsecret on Linux) via keyring-core's platform adapters. The renderer calls credential commands over IPC and never handles a raw secret — a renderer XSS can't reach them because they live outside the web context entirely.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'chromiumoxide',
    name: 'chromiumoxide (headless Chromium automation)',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'chromiumoxide',
    summary: 'Drives a real browser for boards that block plain HTTP.',
    rationale:
      'LinkedIn is scraped over plain HTTP, and the walled boards (Indeed, Glassdoor, StepStone, Xing, Workday) mostly go through the Adzuna/JSearch aggregator — chromiumoxide backs the specific boards and login flows that genuinely need a real, scriptable browser rather than a bare HTTP client.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'reqwest',
    name: 'reqwest',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'reqwest',
    summary: 'The one HTTP client every scraper and AI call goes through.',
    rationale:
      'Centralized in net/http.rs so every outbound call — scraping, AI providers, geocoding — shares one client, one timeout policy, and one place to audit against the network-egress boundary.',
    adrSlug: '0005-network-egress-privacy-boundary',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'zod',
    name: 'Zod',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'zod',
    summary: 'Schema-first validation at every IPC and form boundary.',
    rationale:
      "IPC payloads and form data are validated with Zod at the boundary (IPC receive, form submit); inside the app the inferred TypeScript types are trusted, so component logic isn't full of defensive if (!data?.id) checks.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'ollama',
    name: 'Ollama (local + cloud)',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'service',
    summary: 'The offline-first AI provider — a local model, not just an API key.',
    rationale:
      "The one AI provider that needs no API key and no network call at all: a locally-run model the app talks to over loopback HTTP, with an optional Ollama Cloud mode alongside it. It's the concrete reason 'no API key yet' doesn't mean 'no AI features yet' for a local-first app.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'live-model-listing',
    name: 'Live model listing, no hardcoded defaults',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'technique',
    summary: 'Every provider model list is fetched live — never a curated array.',
    rationale:
      "Four stale-model defects shipped in one session from hand-curated model arrays (a retired embedding model left as the default, a shut-down Gemini preview, its equally-dead list neighbours). ADR-0022 deleted every hardcoded array; a provider's own /models endpoint is now the only source, and onboarding pre-selects nothing.",
    adrSlug: '0022-live-model-listing-no-hardcoded-defaults',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'dedicated-vector-db',
    name: 'A dedicated vector database',
    ring: 'hold',
    quadrant: 'backend-data',
    subjectKind: 'not-adopted',
    summary: 'Considered for posting-embedding search — held.',
    rationale:
      'Posting embeddings are stored in SQLite alongside the documents and searched with an in-memory cosine pass, not in Pinecone/pgvector/a standalone vector engine — the corpus a single local user accumulates is small enough that the extra dependency and the extra moving part it would add buy nothing measurable today.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'nominatim',
    name: 'Nominatim',
    ring: 'hold',
    quadrant: 'backend-data',
    subjectKind: 'service',
    summary: 'Retired as the geocoding fallback — its usage policy forbids autocomplete.',
    rationale:
      'Location autocomplete answers offline from a bundled GeoNames index for virtually every query; only a genuine miss falls through to Photon (OpenStreetMap-backed) as the network fallback. Nominatim filled that fallback role first and was retired because its usage policy explicitly forbids autocomplete-style querying.',
    adrSlug: '0005-network-egress-privacy-boundary',
    lastReviewed: '2026-08-05',
  },
];

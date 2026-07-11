//! Curated company -> (ATS, slug) seed, live-verified 2026-07-11.
//!
//! Lets the company-scoped ATS boards (Greenhouse, Lever, Ashby,
//! SmartRecruiters, Recruitee, Personio, Workable) be exercised without the
//! user hand-typing a company slug. **Data + lookup only** — nothing here is
//! wired into the scrape engine or autopilot yet; that's a follow-up PR.
//!
//! Quirks encoded per entry (see live-verify notes for detail — not
//! re-checked by this module):
//! - **Personio** — one TLD per company (`.de` OR `.com`); [`AtsSeedEntry::tld`]
//!   is `Some` only for Personio entries, `None` everywhere else.
//! - **Ashby** — slug casing is exact and must match the registered board
//!   (e.g. `Linear`, `Perplexity`) — preserved verbatim, never lowercased.
//! - **Lever** and **SmartRecruiters** slugs churn fastest (boards migrate
//!   off or go dormant) — re-verify live before trusting a future update to
//!   this table.

/// One curated company -> ATS slug mapping.
#[derive(Debug, Clone, Copy)]
pub struct AtsSeedEntry {
    pub company: &'static str,
    /// Matches a [`crate::scraping::types::Scraper::id`] in the `SCRAPERS`
    /// registry (`scraping/boards/mod.rs`) exactly.
    pub ats: &'static str,
    pub slug: &'static str,
    /// Personio only: `"de"` or `"com"` — the company's single registered TLD.
    pub tld: Option<&'static str>,
    /// DACH (Germany/Austria/Switzerland) market flag.
    pub dach: bool,
}

#[rustfmt::skip]
static SEED: &[AtsSeedEntry] = &[
    // Greenhouse (27) — boards-api.greenhouse.io/v1/boards/{slug}/jobs
    AtsSeedEntry { company: "Stripe",           ats: "greenhouse", slug: "stripe",             tld: None, dach: false },
    AtsSeedEntry { company: "Airbnb",           ats: "greenhouse", slug: "airbnb",             tld: None, dach: false },
    AtsSeedEntry { company: "Coinbase",         ats: "greenhouse", slug: "coinbase",           tld: None, dach: false },
    AtsSeedEntry { company: "Databricks",       ats: "greenhouse", slug: "databricks",         tld: None, dach: false },
    AtsSeedEntry { company: "Dropbox",          ats: "greenhouse", slug: "dropbox",            tld: None, dach: false },
    AtsSeedEntry { company: "GitLab",           ats: "greenhouse", slug: "gitlab",             tld: None, dach: false },
    AtsSeedEntry { company: "Reddit",           ats: "greenhouse", slug: "reddit",             tld: None, dach: false },
    AtsSeedEntry { company: "Cloudflare",       ats: "greenhouse", slug: "cloudflare",         tld: None, dach: false },
    AtsSeedEntry { company: "Pinterest",        ats: "greenhouse", slug: "pinterest",          tld: None, dach: false },
    AtsSeedEntry { company: "Figma",            ats: "greenhouse", slug: "figma",              tld: None, dach: false },
    AtsSeedEntry { company: "Twilio",           ats: "greenhouse", slug: "twilio",             tld: None, dach: false },
    AtsSeedEntry { company: "Asana",            ats: "greenhouse", slug: "asana",              tld: None, dach: false },
    AtsSeedEntry { company: "Lyft",             ats: "greenhouse", slug: "lyft",               tld: None, dach: false },
    AtsSeedEntry { company: "Instacart",        ats: "greenhouse", slug: "instacart",          tld: None, dach: false },
    AtsSeedEntry { company: "Elastic",          ats: "greenhouse", slug: "elastic",            tld: None, dach: false },
    AtsSeedEntry { company: "HelloFresh",       ats: "greenhouse", slug: "hellofresh",         tld: None, dach: true },
    AtsSeedEntry { company: "Celonis",          ats: "greenhouse", slug: "celonis",            tld: None, dach: true },
    AtsSeedEntry { company: "GetYourGuide",     ats: "greenhouse", slug: "getyourguide",       tld: None, dach: true },
    AtsSeedEntry { company: "Contentful",       ats: "greenhouse", slug: "contentful",         tld: None, dach: true },
    AtsSeedEntry { company: "N26",              ats: "greenhouse", slug: "n26",                tld: None, dach: true },
    AtsSeedEntry { company: "Trade Republic",   ats: "greenhouse", slug: "traderepublicbank",  tld: None, dach: true },
    AtsSeedEntry { company: "Bitpanda",         ats: "greenhouse", slug: "bitpanda",           tld: None, dach: true },
    AtsSeedEntry { company: "SumUp",            ats: "greenhouse", slug: "sumup",              tld: None, dach: true },
    AtsSeedEntry { company: "Cherry Ventures",  ats: "greenhouse", slug: "cherryventures",     tld: None, dach: true },
    AtsSeedEntry { company: "Raisin",           ats: "greenhouse", slug: "raisin",             tld: None, dach: true },
    AtsSeedEntry { company: "Flix (FlixBus)",   ats: "greenhouse", slug: "flix",               tld: None, dach: true },
    AtsSeedEntry { company: "Staffbase",        ats: "greenhouse", slug: "staffbase",          tld: None, dach: true },

    // Lever (4) — api.lever.co/v0/postings/{slug}?mode=json
    AtsSeedEntry { company: "Palantir Technologies", ats: "lever", slug: "palantir",    tld: None, dach: false },
    AtsSeedEntry { company: "Spotify",               ats: "lever", slug: "spotify",     tld: None, dach: false },
    AtsSeedEntry { company: "Veeva Systems",         ats: "lever", slug: "veeva",       tld: None, dach: false },
    AtsSeedEntry { company: "Sword Health",          ats: "lever", slug: "swordhealth", tld: None, dach: false },

    // Ashby (9) — api.ashbyhq.com/posting-api/job-board/{slug} (slug casing matters)
    AtsSeedEntry { company: "OpenAI",       ats: "ashby", slug: "openai",     tld: None, dach: false },
    AtsSeedEntry { company: "Notion",       ats: "ashby", slug: "notion",     tld: None, dach: false },
    AtsSeedEntry { company: "Linear",       ats: "ashby", slug: "Linear",     tld: None, dach: false },
    AtsSeedEntry { company: "Ramp",         ats: "ashby", slug: "ramp",       tld: None, dach: false },
    AtsSeedEntry { company: "PostHog",      ats: "ashby", slug: "posthog",    tld: None, dach: false },
    AtsSeedEntry { company: "Supabase",     ats: "ashby", slug: "supabase",   tld: None, dach: false },
    AtsSeedEntry { company: "Perplexity AI",ats: "ashby", slug: "Perplexity", tld: None, dach: false },
    AtsSeedEntry { company: "ElevenLabs",   ats: "ashby", slug: "elevenlabs", tld: None, dach: false },
    AtsSeedEntry { company: "Cohere",       ats: "ashby", slug: "cohere",     tld: None, dach: false },

    // SmartRecruiters (4) — api.smartrecruiters.com/v1/companies/{slug}/postings
    AtsSeedEntry { company: "Bosch",        ats: "smartrecruiters", slug: "BoschGroup",   tld: None, dach: true },
    AtsSeedEntry { company: "Continental",  ats: "smartrecruiters", slug: "Continental",  tld: None, dach: true },
    AtsSeedEntry { company: "Ubisoft",      ats: "smartrecruiters", slug: "Ubisoft2",     tld: None, dach: false },
    AtsSeedEntry { company: "Visa",         ats: "smartrecruiters", slug: "Visa",         tld: None, dach: false },

    // Recruitee (5) — {slug}.recruitee.com/api/offers/
    AtsSeedEntry { company: "Fastned",   ats: "recruitee", slug: "fastned",       tld: None, dach: false },
    AtsSeedEntry { company: "Fixico",    ats: "recruitee", slug: "fixico",        tld: None, dach: false },
    AtsSeedEntry { company: "Skytree",   ats: "recruitee", slug: "skytree",       tld: None, dach: false },
    AtsSeedEntry { company: "Makersite", ats: "recruitee", slug: "makersitegmbh", tld: None, dach: true },
    AtsSeedEntry { company: "AIHR",      ats: "recruitee", slug: "aihr",          tld: None, dach: false },

    // Personio (7, all DACH) — {slug}.jobs.personio.{de|com}/
    AtsSeedEntry { company: "Jung von Matt",         ats: "personio", slug: "jungvonmatt",          tld: Some("de"),  dach: true },
    AtsSeedEntry { company: "Audi Formula Racing",   ats: "personio", slug: "audi-formula-racing",  tld: Some("de"),  dach: true },
    AtsSeedEntry { company: "lavera / Laverana",     ats: "personio", slug: "lavera",               tld: Some("de"),  dach: true },
    AtsSeedEntry { company: "deskbird",              ats: "personio", slug: "deskbird",             tld: Some("com"), dach: true },
    AtsSeedEntry { company: "Cardmarket",            ats: "personio", slug: "cardmarket",           tld: Some("com"), dach: true },
    AtsSeedEntry { company: "Athereon GRC",          ats: "personio", slug: "athereon",             tld: Some("com"), dach: true },
    AtsSeedEntry { company: "Groß & Partner",        ats: "personio", slug: "gross-und-partner",    tld: Some("de"),  dach: true },

    // Workable (3) — POST-only, apply.workable.com/api/v3/accounts/{slug}/jobs
    AtsSeedEntry { company: "Startups.com", ats: "workable", slug: "startups",     tld: None, dach: false },
    AtsSeedEntry { company: "500 Global",   ats: "workable", slug: "500startups",  tld: None, dach: false },
    AtsSeedEntry { company: "atmio",        ats: "workable", slug: "atmio",        tld: None, dach: true },
];

/// All curated entries, in source (per-ATS-block) order.
pub fn all() -> &'static [AtsSeedEntry] {
    SEED
}

/// Entries for one ATS board id (e.g. `"greenhouse"`), in source order.
pub fn by_ats(board_id: &str) -> impl Iterator<Item = &'static AtsSeedEntry> + use<'_> {
    SEED.iter().filter(move |e| e.ats == board_id)
}

#[cfg(test)]
mod test;

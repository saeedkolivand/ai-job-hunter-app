pub mod aggregator;
pub mod arbeitnow;
pub mod arbeitsagentur;
pub mod ashby;
/// Curated company -> (ATS, slug) seed data + lookup. Not a `Scraper` — not
/// registered in `SCRAPERS`; a later wiring PR routes these into search input.
pub mod ats_seed;
pub mod bamboohr;
pub mod berlinstartupjobs;
pub mod breezy;
pub mod comeet;
/// Shared helpers reused across several ATS boards — not itself a `Scraper`.
pub(crate) mod common;
pub mod germantechjobs;
pub mod greenhouse;
pub mod lever;
pub mod linkedin;
pub mod personio;
pub mod pinpoint;
pub mod recruitee;
pub mod remoteok;
pub mod remotive;
pub mod rippling;
pub mod smartrecruiters;
pub mod themuse;
pub mod workable;
pub mod wwr;
pub mod ycombinator;

pub use aggregator::AggregatorScraper;
pub use arbeitnow::ArbeitnowScraper;
pub use arbeitsagentur::ArbeitsagenturScraper;
pub use ashby::AshbyScraper;
pub use bamboohr::BambooHrScraper;
pub use berlinstartupjobs::BerlinStartupJobsScraper;
pub use breezy::BreezyScraper;
pub use comeet::ComeetScraper;
pub use germantechjobs::GermanTechJobsScraper;
pub use greenhouse::GreenhouseScraper;
pub use lever::LeverScraper;
pub use linkedin::LinkedInScraper;
pub use personio::PersonioScraper;
pub use pinpoint::PinpointScraper;
pub use recruitee::RecruiteeScraper;
pub use remoteok::RemoteOkScraper;
pub use remotive::RemotiveScraper;
pub use rippling::RipplingScraper;
pub use smartrecruiters::SmartRecruitersScraper;
pub use themuse::TheMuseScraper;
pub use workable::WorkableScraper;
pub use wwr::WeWorkRemotelyScraper;
pub use ycombinator::YCombinatorScraper;

use super::types::Scraper;

/// Every scraper, registered exactly once — in catalog display order. Both
/// dispatch ([`get`]) and the UI catalog ([`all`] → `ScraperEngine::catalog`)
/// derive from this list via the `Scraper` trait, so adding a board is one
/// implementation module + one line here (no parallel match or hardcoded array).
static SCRAPERS: &[&dyn Scraper] = &[
    &AggregatorScraper,
    &LinkedInScraper,
    &YCombinatorScraper,
    &RemotiveScraper,
    &RemoteOkScraper,
    &WeWorkRemotelyScraper,
    &ArbeitnowScraper,
    &BerlinStartupJobsScraper,
    &GermanTechJobsScraper,
    &GreenhouseScraper,
    &LeverScraper,
    &SmartRecruitersScraper,
    &PersonioScraper,
    &RecruiteeScraper,
    &AshbyScraper,
    &ArbeitsagenturScraper,
    &PinpointScraper,
    &RipplingScraper,
    &BreezyScraper,
    &BambooHrScraper,
    &TheMuseScraper,
    &WorkableScraper,
    &ComeetScraper,
];

/// All registered scrapers, in catalog display order.
pub fn all() -> &'static [&'static dyn Scraper] {
    SCRAPERS
}

/// Resolve a scraper by its `id()`. `None` for an unknown board.
pub fn get(id: &str) -> Option<&'static dyn Scraper> {
    SCRAPERS.iter().copied().find(|s| s.id() == id)
}

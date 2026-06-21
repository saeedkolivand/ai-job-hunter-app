pub mod aggregator;
pub mod arbeitnow;
pub mod arbeitsagentur;
pub mod ashby;
pub mod berlinstartupjobs;
pub mod germantechjobs;
pub mod glassdoor;
pub mod greenhouse;
pub mod indeed;
pub mod lever;
pub mod linkedin;
pub mod personio;
pub mod recruitee;
pub mod remoteok;
pub mod remotive;
pub mod smartrecruiters;
pub mod stepstone;
pub mod workday;
pub mod wwr;
pub mod xing;
pub mod ycombinator;

pub use aggregator::AggregatorScraper;
pub use arbeitnow::ArbeitnowScraper;
pub use arbeitsagentur::ArbeitsagenturScraper;
pub use ashby::AshbyScraper;
pub use berlinstartupjobs::BerlinStartupJobsScraper;
pub use germantechjobs::GermanTechJobsScraper;
pub use glassdoor::GlassdoorScraper;
pub use greenhouse::GreenhouseScraper;
pub use indeed::IndeedScraper;
pub use lever::LeverScraper;
pub use linkedin::LinkedInScraper;
pub use personio::PersonioScraper;
pub use recruitee::RecruiteeScraper;
pub use remoteok::RemoteOkScraper;
pub use remotive::RemotiveScraper;
pub use smartrecruiters::SmartRecruitersScraper;
pub use stepstone::StepStoneScraper;
pub use workday::WorkdayScraper;
pub use wwr::WeWorkRemotelyScraper;
pub use xing::XingScraper;
pub use ycombinator::YCombinatorScraper;

use super::types::Scraper;

/// Every scraper, registered exactly once — in catalog display order. Both
/// dispatch ([`get`]) and the UI catalog ([`all`] → `ScraperEngine::catalog`)
/// derive from this list via the `Scraper` trait, so adding a board is one
/// implementation module + one line here (no parallel match or hardcoded array).
static SCRAPERS: &[&dyn Scraper] = &[
    &AggregatorScraper,
    &LinkedInScraper,
    &GlassdoorScraper,
    &XingScraper,
    &IndeedScraper,
    &YCombinatorScraper,
    &RemotiveScraper,
    &RemoteOkScraper,
    &WeWorkRemotelyScraper,
    &ArbeitnowScraper,
    &BerlinStartupJobsScraper,
    &GermanTechJobsScraper,
    &GreenhouseScraper,
    &LeverScraper,
    &StepStoneScraper,
    &SmartRecruitersScraper,
    &PersonioScraper,
    &RecruiteeScraper,
    &WorkdayScraper,
    &AshbyScraper,
    &ArbeitsagenturScraper,
];

/// All registered scrapers, in catalog display order.
pub fn all() -> &'static [&'static dyn Scraper] {
    SCRAPERS
}

/// Resolve a scraper by its `id()`. `None` for an unknown board.
pub fn get(id: &str) -> Option<&'static dyn Scraper> {
    SCRAPERS.iter().copied().find(|s| s.id() == id)
}

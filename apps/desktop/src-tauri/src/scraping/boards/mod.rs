pub mod aggregator;
pub mod arbeitnow;
pub mod arbeitsagentur;
pub mod ashby;
/// Curated company -> (ATS, slug) seed data + lookup. Not a `Scraper` — not
/// registered in `SCRAPERS`; the engine (`scraping/engine/mod.rs`) reads it
/// via `by_ats` to auto-populate a company-scoped board's search input.
pub mod ats_seed;
pub mod bamboohr;
pub mod berlinstartupjobs;
pub mod breezy;
pub mod comeet;
/// Shared helpers reused across several ATS boards — not itself a `Scraper`.
pub(crate) mod common;
pub mod germantechjobs;
pub mod greenhouse;
pub mod jobicy;
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
pub use jobicy::JobicyScraper;
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
    &JobicyScraper,
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

#[cfg(test)]
mod registry_parity {
    use crate::ipc_contracts::board_ids::BOARD_IDS;

    /// The renderer carries its own copy of this registry's ids
    /// (`packages/shared/src/schemas/index.ts::BOARD_IDS`, emitted to Rust by
    /// `pnpm gen:ipc`), because the TS side needs a `BoardId` type and a Zod
    /// enum. Two hand-maintained lists of the same vocabulary is exactly the
    /// shape that drifts, and it had: `jobicy` was registered here, listed in
    /// the catalog and given en/de labels, while the shared list never learned
    /// about it. Nothing on either side could see the gap, because nothing
    /// compared them.
    ///
    /// Asserted in BOTH directions on purpose. One direction catches a board
    /// added to the registry and forgotten in TS (the `jobicy` case); the other
    /// catches a board removed here while the TS list — and therefore the
    /// `BoardId` type the renderer programs against — still promises it.
    #[test]
    fn the_shared_board_id_list_and_this_registry_name_the_same_boards() {
        let registry: Vec<&str> = super::SCRAPERS.iter().map(|s| s.id()).collect();

        let missing_from_shared: Vec<&str> = registry
            .iter()
            .copied()
            .filter(|id| !BOARD_IDS.contains(id))
            .collect();
        assert!(
            missing_from_shared.is_empty(),
            "registered here but absent from packages/shared BOARD_IDS: {missing_from_shared:?} \
             — add them there and run `pnpm gen:ipc`"
        );

        let missing_from_registry: Vec<&str> = BOARD_IDS
            .iter()
            .copied()
            .filter(|id| !registry.contains(id))
            .collect();
        assert!(
            missing_from_registry.is_empty(),
            "promised by packages/shared BOARD_IDS but not registered in SCRAPERS: \
             {missing_from_registry:?}"
        );
    }

    /// A vacuity guard for the test above. Both of its assertions are "this
    /// difference is empty", which is trivially true of two empty lists — so if
    /// a refactor ever left `SCRAPERS` or the generated const empty, the parity
    /// test would pass while checking nothing at all.
    #[test]
    fn both_lists_are_actually_populated() {
        assert!(super::SCRAPERS.len() >= 20, "the registry lost boards");
        assert_eq!(
            BOARD_IDS.len(),
            super::SCRAPERS.len(),
            "the two lists must have the same length, not merely overlap"
        );
    }
}

//! Aggregator board tests, split by concern: fallback-chain orchestration,
//! per-provider response mapping, credential/needs-keys classification,
//! the guessed-market guard, the Apify LinkedIn tier, and the Adzuna page loop.

mod adzuna_country_and_currency;
mod adzuna_page_loop;
mod apify_findings;
mod apify_fixes;
mod apify_merge_and_budget;
mod apify_provider_mapping;
mod cross_board_clustering;
mod date_filter_mapping;
mod fallback_chain;
mod guessed_market_guard;
mod guessed_market_guard_edge;
mod jooble_last_resort;
mod jooble_mapping;
mod needs_keys_skip;
mod post_loop_and_paging;
mod quota_neutral_runs;
mod response_mapping;
mod scraper_shape_and_credentials;
mod support;

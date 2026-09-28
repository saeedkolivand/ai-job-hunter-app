//! `ScraperEngine` tests, split by concern: core/catalog, `run_one`/
//! `run_boards`, the CWE-770 batch cap, TRUST PR A/B/D/E/F honesty
//! containment, the required-auth/needs-company skip gates, `ats_seed`
//! auto-population, cross-source dedup, the central location/work-type
//! post-filters, and Track B1 board-health history.

mod ats_partial_failure;
mod ats_seed_autopop;
mod board_batch_cap;
mod board_error_and_truncation;
mod board_health_history;
mod browser_semaphore;
mod core_and_catalog;
mod cross_source_dedup_field_policy;
mod cross_source_dedup_integration;
mod location_policy_notes;
mod location_post_filter_basic;
mod location_post_filter_cap_interaction;
mod location_post_filter_live_stream_agreement;
mod needs_company_skip;
mod panickers;
mod required_board_skip_edge_cases;
mod required_board_skip_reasons;
mod run_boards;
mod run_one;
mod small_guards;
mod support;
mod work_type_post_filter_basic;
mod work_type_post_filter_cap_interaction;
mod work_type_post_filter_combined_notes;

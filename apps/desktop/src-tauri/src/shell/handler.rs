//! The IPC command registry: every `#[tauri::command]` the renderer (and the
//! agent-CLI) can reach, in one list. Split out of `lib.rs` for R8 (issue #1280).
//!
//! The paths are load-bearing, not cosmetic: `extension_bridge::agent_cli::policy`
//! keys its `POLICY` rows on these exact command paths, and a set-equality test
//! (`policy_table_matches_generate_handler_exactly`) reads this file to compare the
//! two, so a command added here without a policy row fails the build.

use crate::{commands, export, updater};

/// The handler `run()` installs with `invoke_handler`.
pub(crate) fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        // cli agents (install status — #22)
        commands::cli_agents::cli_agents_status,
        commands::cli_agents::cli_agents_redetect,
        // system
        commands::system::system_health,
        commands::system::system_get_version,
        commands::system::system_get_locale,
        commands::system::system_set_locale,
        commands::system::system_get_platform,
        commands::system::system_accent_color,
        commands::system::system_open_external,
        commands::system::system_set_performance_mode,
        commands::system::system_get_launch_at_login,
        commands::system::system_set_launch_at_login,
        commands::system::system_set_close_to_tray,
        commands::system::system_get_metrics,
        commands::system::system_check_browser,
        commands::system::system_open_devtools,
        commands::system::system_get_protocol_version,
        commands::system::system_agent_cli_info,
        // native menu (pull buffered intent after close-to-tray restore)
        commands::menu::menu_take_pending,
        // jobs
        commands::jobs::jobs_list,
        commands::jobs::jobs_get,
        commands::jobs::jobs_cancel,
        commands::jobs::jobs_retry,
        // ai
        commands::ai::ai_generate,
        commands::ai::ai_list_models,
        commands::ai::ai_model_capabilities,
        commands::ai::ai_inspect_model,
        commands::ai::ai_research_company,
        commands::ai::ai_research_answer,
        commands::ai::ai_lookup_salary,
        commands::ai::ai_pull_model,
        commands::ai::ai_unload_model,
        commands::ai::ai_embed,
        commands::ai::ai_set_provider_key,
        commands::ai::ai_remove_provider_key,
        commands::ai::ai_has_provider_key,
        commands::ai::ai_test_provider_key,
        commands::ai::ai_list_provider_models,
        commands::ai::ai_embedding_status,
        commands::ai::ai_set_embedding_config,
        commands::ai::ai_reembed_all,
        commands::ai::ai_index_stale_documents,
        commands::ai::ai_spend_summary,
        commands::ai::ai_active_config,
        commands::ai::ai_set_active_provider,
        commands::ai::ai_set_provider_settings,
        commands::ai::ai_seed_active_config,
        // per-stage model overrides (same store, same validation chain)
        commands::ai::ai_stage_overrides,
        commands::ai::ai_set_stage_override,
        commands::ai::ai_clear_stage_override,
        commands::pipeline::generate_pipeline,
        // resume extraction + content-quality checks
        commands::resume::extract_resume,
        commands::resume::resume_validate_content,
        // staged résumé pipeline (quality depth)
        commands::resume_pipeline::run::resume_pipeline_run,
        commands::resume_pipeline::read::resume_pipeline_get,
        commands::resume_pipeline::read::resume_pipeline_list_for_job,
        commands::resume_pipeline::regenerate::resume_pipeline_regenerate_section,
        commands::resume_pipeline::regenerate::resume_pipeline_resolve_fabrication,
        // documents
        commands::documents::documents_list,
        commands::documents::documents_import,
        commands::documents::documents_recommend_template,
        commands::documents::documents_remove,
        commands::documents::documents_set_default,
        commands::documents::documents_get_text,
        // job preferences
        commands::job_preferences::job_preferences_get,
        commands::job_preferences::job_preferences_set,
        commands::job_preferences::job_preferences_set_salary_expectation,
        commands::job_preferences::job_preferences_set_semantic_scoring,
        commands::job_preferences::job_preferences_set_extra_agency_companies,
        // contact profile (header source of truth)
        commands::contact_profile::contact_profile_get,
        commands::contact_profile::contact_profile_set,
        commands::contact_profile::contact_profile_header_line,
        // scrape
        commands::scrape::scrape_boards,
        commands::scrape::scrape_url,
        commands::scrape::scrape_resolve_url,
        commands::scrape::scrape_update_description,
        commands::scrape::scrape_persist_job,
        commands::scrape::scrape_remove_interaction,
        commands::scrape::scrape_list_postings,
        commands::scrape::scrape_clear_postings,
        commands::scrape::scrape_list_interactions,
        // hybrid postings search (lexical FTS5 + dense + fusion + rerank)
        commands::hybrid_search::scrape_hybrid_search,
        // data backup / restore
        commands::data::data_export,
        commands::data::data_import,
        // cross-board dedup (ADR-029 — split a wrongly-merged cluster)
        commands::dedup::dedup_mark_not_duplicate,
        // discovery (ADR-030 — passively-harvested ATS company slugs)
        commands::discovery::discovery_search_companies,
        commands::discovery::discovery_set_starred,
        commands::discovery::discovery_watched,
        // match
        commands::match_resume::match_resume,
        commands::match_resume::match_resume_text,
        commands::match_resume::resume_extract_text,
        commands::match_resume::resume_trim_suggestions,
        // credentials (board-login CRUD removed — sessions auth via boards.*)
        commands::credentials::credentials_available,
        // boards
        commands::boards::boards_login_with_browser,
        commands::boards::boards_import_cookies,
        commands::boards::boards_logout,
        commands::boards::boards_get_status,
        commands::boards::boards_list,
        commands::boards::boards_catalog,
        commands::boards::boards_health,
        // privacy
        commands::privacy::privacy_clear_data,
        commands::privacy::privacy_clear_interactions,
        commands::privacy::privacy_sign_out_all,
        commands::privacy::privacy_reset_app,
        commands::privacy::privacy_get_crash_reporting,
        commands::privacy::privacy_set_crash_reporting,
        // in-app help retrieval (lexical FTS5 + an opt-in dense arm)
        commands::help::help_search,
        // support
        commands::support::support_export_diagnostics,
        commands::support::support_get_system_info,
        // dialog
        commands::dialog::dialog_open_files,
        // geocoding
        commands::geocoding::geocode_suggest,
        // autopilot
        commands::autopilot::autopilot_list,
        commands::autopilot::autopilot_get,
        commands::autopilot::autopilot_create,
        commands::autopilot::autopilot_update,
        commands::autopilot::autopilot_remove,
        commands::autopilot::autopilot_run,
        commands::autopilot::autopilot_pause,
        commands::autopilot::autopilot_resume,
        commands::autopilot::autopilot_take_pending_focus,
        commands::autopilot::autopilot_best_matches,
        // ai generations
        commands::ai_generations::ai_generations_list,
        commands::ai_generations::ai_generations_save,
        commands::ai_generations::ai_generations_update,
        commands::ai_generations::ai_generations_remove,
        commands::ai_generations::ai_generations_remove_bulk,
        // applications (status-bearing aggregate — ADR 0001)
        commands::applications::applications_list,
        commands::applications::applications_get,
        commands::applications::applications_set_status,
        commands::applications::applications_accept_status_event,
        commands::applications::applications_reject_status_event,
        commands::applications::applications_update,
        commands::applications::applications_delete,
        commands::applications::applications_track,
        commands::applications::applications_save_from_posting,
        // notification center (Phase 2 — IPC seam over the persisted store)
        commands::notifications::notifications_list,
        commands::notifications::notifications_mark_read,
        commands::notifications::notifications_mark_all_read,
        commands::notifications::notifications_remove,
        commands::notifications::notifications_clear_all,
        commands::notifications::notifications_clicked,
        // referrals (manual referral helper)
        commands::referrals::referrals_list,
        commands::referrals::referrals_upsert,
        commands::referrals::referrals_remove,
        // profile import
        commands::profile_import::profile_import_from_url,
        // github repos import (resume-builder projects step)
        commands::github::github_import_repos,
        // browser-extension bridge (Feature 2 — loopback WS control)
        commands::extension_bridge::extension_bridge_status,
        commands::extension_bridge::extension_bridge_regenerate_token,
        commands::extension_bridge::extension_bridge_autofill_enabled,
        commands::extension_bridge::extension_bridge_set_autofill_enabled,
        commands::extension_bridge::extension_bridge_ai_assist_enabled,
        commands::extension_bridge::extension_bridge_set_ai_assist_enabled,
        commands::extension_bridge::extension_bridge_auto_track_enabled,
        commands::extension_bridge::extension_bridge_set_auto_track_enabled,
        // email-confirmation watching (Task #23, PR A — connect/status only)
        commands::email_watch::email_watch_status,
        commands::email_watch::email_watch_connect,
        commands::email_watch::email_watch_disconnect,
        commands::email_watch::email_watch_set_enabled,
        commands::email_watch::email_watch_set_auto_write_enabled,
        commands::email_watch::email_watch_check_now,
        // export
        export::commands::documents_export_document,
        export::commands::documents_export_and_save,
        export::commands::documents_render_preview_images,
        // updater
        updater::updater_status,
        updater::updater_check,
        updater::updater_download,
        updater::updater_install,
        updater::updater_changelog,
    ]
}

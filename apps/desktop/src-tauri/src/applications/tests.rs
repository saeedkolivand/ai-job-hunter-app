//! Store tests for [`ApplicationStore`].
//!
//! Split by topic: the CRUD / upsert / status basics ([`crud`], [`generate_save`]); the
//! compare-and-set transitions ([`transitions`]) and the email-derived accept / reject
//! review ([`status_event_review`]); the follow-up reminder sweep, claim and marker
//! ([`follow_up_reminders`], [`reminder_marker`]); the answer-list merges
//! ([`answer_merging`]) and the races around them ([`concurrency`]); the contact
//! unification ([`contact_migration`], [`contact_aliases`]); the additive column
//! migrations ([`description_columns`], [`recipient_salary_columns`]); backup
//! export / import ([`import_export`]); and everything that crosses into the sibling
//! `ai_generations.db` — the legacy backfill and its one-shot marker
//! ([`legacy_backfill`]), the boot-time orphan link ([`generation_links`]) and the
//! delete / no-resurrection sequences ([`delete_resurrection`]). [`support`] holds the
//! fixtures the siblings share.

use std::path::Path;

use rusqlite::{params, Connection};
use tempfile::TempDir;

use super::model::clamp_job_description;
use super::*;
use crate::ai_generations::ApplicationAnswer;
use crate::data_store::DataStore;
use crate::db::{now_ms, ts_to_db};
use crate::error::AppError;

mod answer_merging;
mod concurrency;
mod contact_aliases;
mod contact_migration;
mod crud;
mod delete_resurrection;
mod description_columns;
mod follow_up_reminders;
mod generate_save;
mod generation_links;
mod import_export;
mod legacy_backfill;
mod recipient_salary_columns;
mod reminder_marker;
mod status_event_review;
mod support;
mod transitions;

//! Engine tests: two (or three) simulated devices, each with its own
//! `sync_state.db` and fake in-memory sections, talking to an in-memory
//! server that implements the v1 protocol.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tempfile::TempDir;

use super::engine::{run, SyncReport, Transport};
use super::envelope::{Envelope, Page, PROTOCOL_VERSION};
use super::state::SyncStateStore;
use crate::data_store::{SyncRecord, SyncSection};
use crate::error::{AppError, AppResult};

mod engine_cases;
mod http_transport;
mod legacy_float;
mod rules_cases;
mod settings_cases;
mod stores;

/// An in-memory section: `id -> payload`.
pub(super) struct FakeSection {
    name: &'static str,
    schema: Cell<u32>,
    /// Make every `upsert_record` fail, like a store write error.
    pub(super) fail_writes: Cell<bool>,
    records: Mutex<BTreeMap<String, Value>>,
}

impl FakeSection {
    pub(super) fn new(name: &'static str) -> Self {
        Self {
            name,
            schema: Cell::new(1),
            fail_writes: Cell::new(false),
            records: Mutex::new(BTreeMap::new()),
        }
    }
    pub(super) fn put(&self, id: &str, v: Value) {
        self.records.lock().insert(id.to_string(), v);
    }
    pub(super) fn remove(&self, id: &str) {
        self.records.lock().remove(id);
    }
    pub(super) fn get(&self, id: &str) -> Option<Value> {
        self.records.lock().get(id).cloned()
    }
    pub(super) fn snapshot(&self) -> BTreeMap<String, Value> {
        self.records.lock().clone()
    }
    pub(super) fn set_schema(&self, v: u32) {
        self.schema.set(v);
    }
}

impl SyncSection for FakeSection {
    fn section(&self) -> &'static str {
        self.name
    }
    fn schema_version(&self) -> u32 {
        self.schema.get()
    }
    fn sync_records(&self) -> AppResult<Vec<SyncRecord>> {
        Ok(self
            .records
            .lock()
            .iter()
            .map(|(id, p)| SyncRecord {
                id: id.clone(),
                payload: p.clone(),
            })
            .collect())
    }
    fn upsert_record(&self, id: &str, payload: &Value) -> AppResult<()> {
        if self.fail_writes.get() {
            return Err(AppError::Validation("store write failed".into()));
        }
        self.put(id, payload.clone());
        Ok(())
    }
    fn delete_record(&self, id: &str) -> AppResult<()> {
        self.remove(id);
        Ok(())
    }
}

/// An in-memory v1 server for one namespace. The cursor is the log length.
#[derive(Default)]
pub(super) struct FakeServer {
    log: RefCell<Vec<Envelope>>,
    pub(super) page_size: Cell<usize>,
    pub(super) fail_push: Cell<bool>,
    /// Fail the pull that would start at this log offset.
    pub(super) fail_pull_at: Cell<Option<usize>>,
    pub(super) corrupt_next_page: Cell<bool>,
    /// Store only this many envelopes of the next push.
    pub(super) store_at_most: Cell<Option<usize>>,
    /// Answer every pull with `hasMore` and the same cursor.
    pub(super) stuck_cursor: Cell<bool>,
}

impl FakeServer {
    pub(super) fn new() -> Self {
        let s = Self::default();
        s.page_size.set(1000);
        s
    }
    pub(super) fn len(&self) -> usize {
        self.log.borrow().len()
    }
}

impl Transport for FakeServer {
    fn health(&self) -> AppResult<u32> {
        Ok(PROTOCOL_VERSION)
    }
    fn pull(&self, since: Option<&str>) -> AppResult<Page> {
        let from: usize = since.map_or(0, |c| c.parse().unwrap_or(0));
        if self.stuck_cursor.get() {
            return Ok(Page {
                changes: Vec::new(),
                cursor: since.unwrap_or("0").to_string(),
                has_more: true,
            });
        }
        if self.fail_pull_at.get() == Some(from) {
            return Err(AppError::Validation("network down".into()));
        }
        let log = self.log.borrow();
        let to = (from + self.page_size.get()).min(log.len());
        let mut changes = log[from..to].to_vec();
        if self.corrupt_next_page.replace(false) {
            if let Some(first) = changes.first_mut() {
                first.record_id = String::new();
            }
        }
        Ok(Page {
            changes,
            cursor: to.to_string(),
            has_more: to < log.len(),
        })
    }
    fn push(&self, _device: &str, changes: &[Envelope]) -> AppResult<usize> {
        if self.fail_push.get() {
            return Err(AppError::Validation("push failed".into()));
        }
        let n = self
            .store_at_most
            .take()
            .map_or(changes.len(), |m| m.min(changes.len()));
        self.log.borrow_mut().extend_from_slice(&changes[..n]);
        Ok(n)
    }
}

/// One simulated install: its own state DB and sections.
pub(super) struct Device {
    _dir: TempDir,
    pub(super) state: SyncStateStore,
    pub(super) items: FakeSection,
    pub(super) settings: FakeSection,
}

impl Device {
    pub(super) fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let state = SyncStateStore::open(dir.path()).unwrap();
        Self {
            _dir: dir,
            state,
            items: FakeSection::new("items"),
            settings: FakeSection::new("settings"),
        }
    }
    pub(super) fn sync(&self, server: &FakeServer, rev: u64) -> AppResult<SyncReport> {
        self.sync_scope(server, rev, "scope-a")
    }
    pub(super) fn sync_scope(
        &self,
        server: &FakeServer,
        rev: u64,
        scope: &str,
    ) -> AppResult<SyncReport> {
        let sections: [&dyn SyncSection; 2] = [&self.items, &self.settings];
        run(&sections, &self.state, server, scope, rev)
    }
}

pub(super) fn v(s: &str) -> Value {
    json!({ "v": s })
}

//! One sync run (`docs/knowledge/sync-protocol.md` → "Order of a sync run"):
//! health check, pull and apply every page, then diff the export bundle
//! against the baseline and push. The transport is a trait so the HTTP client
//! (a later PR) and the in-memory test server plug in the same way.

use std::collections::{BTreeMap, BTreeSet};

use super::envelope::{record_hash, Envelope, Page, PROTOCOL_VERSION};
use super::legacy_float::legacy_record_hash;
use super::rules::{decide, Decision, Local};
use super::state::{BaselineEntry, SyncStateStore};
use crate::data_store::SyncSection;
use crate::error::{AppError, AppResult};

/// The server side of the protocol.
pub trait Transport {
    /// `GET /health` → `protocolVersion`.
    fn health(&self) -> AppResult<u32>;
    /// `GET /changes?since=<cursor>`.
    fn pull(&self, since: Option<&str>) -> AppResult<Page>;
    /// `POST /changes`; returns the number of envelopes the server stored.
    fn push(&self, device_id: &str, changes: &[Envelope]) -> AppResult<usize>;
}

/// What one run did, for logs and the settings card.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub pulled: usize,
    pub applied: usize,
    pub held: usize,
    pub pushed: usize,
}

type Key = (String, String);
/// Each record's current hash.
type Hashes = BTreeMap<Key, String>;

/// Run one sync for `scope`. `run_rev` is this run's time in ms.
pub fn run(
    sections: &[&dyn SyncSection],
    state: &SyncStateStore,
    transport: &dyn Transport,
    scope: &str,
    run_rev: u64,
) -> AppResult<SyncReport> {
    let version = transport.health()?;
    if version != PROTOCOL_VERSION {
        return Err(AppError::Validation(format!(
            "sync server speaks protocol {version}, this app speaks {PROTOCOL_VERSION}"
        )));
    }
    let device_id = state.device_id()?;
    let mut report = SyncReport::default();

    let mut baseline = baseline_map(state, scope)?;
    // Payloads are only needed for the hashes and the one-time rebaseline: drop them here.
    let current: Hashes = {
        let records = current_records(sections)?;
        let current = records
            .iter()
            .map(|(k, (_, p))| (k.clone(), record_hash(p)))
            .collect();
        rebaseline_legacy_floats(state, scope, &records, &current, &mut baseline)?;
        current
    };
    let mut ctx = Ctx {
        by_name: sections.iter().map(|s| (s.section(), *s)).collect(),
        state,
        scope,
        run_rev,
        device_id: &device_id,
        current,
        baseline,
        applied: BTreeSet::new(),
    };

    // Envelopes held for a newer schema that this app now understands.
    for env in state.held(scope)? {
        let understood = ctx
            .by_name
            .get(env.store.as_str())
            .is_some_and(|s| env.schema_version <= s.schema_version());
        if understood {
            // Released only once applied: the cursor is already past it, so a
            // failed apply must leave it held for the next run.
            ctx.handle(&env, &mut report)?;
            state.release(scope, &env.store, &env.record_id)?;
        }
    }

    // Pull: apply page by page; the saved cursor is the acknowledgement. A
    // malformed envelope rejects the page before anything in it is applied.
    let mut cursor = state.cursor(scope)?;
    loop {
        let page = transport.pull(cursor.as_deref())?;
        for env in &page.changes {
            env.validate()?;
        }
        for env in &page.changes {
            report.pulled += 1;
            ctx.handle(env, &mut report)?;
        }
        if page.has_more && cursor.as_deref() == Some(page.cursor.as_str()) {
            // A page that promises more but does not move the cursor would
            // loop forever on a buggy or hostile server.
            return Err(AppError::Validation(
                "sync server returned more pages without advancing the cursor".into(),
            ));
        }
        state.set_cursor(scope, &page.cursor)?;
        cursor = Some(page.cursor);
        if !page.has_more {
            break;
        }
    }
    let applied = std::mem::take(&mut ctx.applied);

    // A store may normalize what it is given; take the hash of what it now
    // exports so an applied record is not echoed back as a local change.
    let current = current_records(sections)?;
    for key in &applied {
        if let (Some((_, payload)), Some(mut entry)) =
            (current.get(key), ctx.baseline.get(key).cloned())
        {
            entry.hash = record_hash(payload);
            state.put_baseline(scope, &key.0, &key.1, &entry)?;
        }
    }

    // Push: everything new or changed since the baseline, plus tombstones.
    let outgoing = outgoing(sections, state, scope, &device_id, run_rev, &current)?;
    if !outgoing.is_empty() {
        report.pushed = transport.push(&device_id, &outgoing)?;
        // Record nothing as synced unless the server stored the whole batch;
        // the next run then computes and sends the same changes again.
        if report.pushed != outgoing.len() {
            return Err(AppError::Validation(format!(
                "sync server stored {} of {} changes",
                report.pushed,
                outgoing.len()
            )));
        }
        for env in &outgoing {
            let hash = if env.deleted {
                String::new()
            } else {
                record_hash(&env.payload)
            };
            let entry = entry_of(env, hash);
            state.put_baseline(scope, &env.store, &env.record_id, &entry)?;
        }
    }
    Ok(report)
}

/// Per-run state shared by held and pulled envelopes.
struct Ctx<'a> {
    by_name: BTreeMap<&'static str, &'a dyn SyncSection>,
    state: &'a SyncStateStore,
    scope: &'a str,
    run_rev: u64,
    device_id: &'a str,
    current: Hashes,
    baseline: BTreeMap<Key, BaselineEntry>,
    applied: BTreeSet<Key>,
}

impl Ctx<'_> {
    fn handle(&mut self, env: &Envelope, report: &mut SyncReport) -> AppResult<()> {
        let key = (env.store.clone(), env.record_id.clone());
        let section = self.by_name.get(env.store.as_str()).copied();
        let local = Local {
            baseline: self.baseline.get(&key),
            current_hash: self.current.get(&key).map(String::as_str),
            schema_version: section.map_or(0, |s| s.schema_version()),
            run_rev: self.run_rev,
            device_id: self.device_id,
        };
        match (decide(env, &local, section.is_some()), section) {
            (Decision::Hold, _) => {
                self.state.hold(self.scope, env)?;
                report.held += 1;
            }
            (Decision::Upsert, Some(s)) => {
                s.upsert_record(&env.record_id, &env.payload)?;
                let hash = record_hash(&env.payload);
                self.remember(&key, env, hash.clone())?;
                self.current.insert(key.clone(), hash);
                self.applied.insert(key);
                report.applied += 1;
            }
            (Decision::Delete, Some(s)) => {
                s.delete_record(&env.record_id)?;
                self.remember(&key, env, String::new())?;
                self.current.remove(&key);
                report.applied += 1;
            }
            _ => {}
        }
        Ok(())
    }

    fn remember(&mut self, key: &Key, env: &Envelope, hash: String) -> AppResult<()> {
        let entry = entry_of(env, hash);
        self.state
            .put_baseline(self.scope, &key.0, &key.1, &entry)?;
        self.baseline.insert(key.clone(), entry);
        Ok(())
    }
}

fn outgoing(
    sections: &[&dyn SyncSection],
    state: &SyncStateStore,
    scope: &str,
    device_id: &str,
    run_rev: u64,
    current: &BTreeMap<Key, (u32, serde_json::Value)>,
) -> AppResult<Vec<Envelope>> {
    let baseline = baseline_map(state, scope)?;
    let held: BTreeSet<Key> = state
        .held(scope)?
        .into_iter()
        .map(|e| (e.store, e.record_id))
        .collect();
    let schema: BTreeMap<&str, u32> = sections
        .iter()
        .map(|s| (s.section(), s.schema_version()))
        .collect();
    let mut out = Vec::new();
    for (key, (schema_version, payload)) in current {
        if held.contains(key) {
            continue;
        }
        match baseline.get(key) {
            Some(b) if b.deleted || b.hash == record_hash(payload) => {}
            _ => out.push(envelope(
                key,
                run_rev,
                device_id,
                *schema_version,
                payload.clone(),
            )),
        }
    }
    // An empty baseline (a first sync) has no rows, so it can never produce a
    // tombstone here.
    for (key, b) in &baseline {
        if !b.deleted && !current.contains_key(key) && !held.contains(key) {
            let v = schema
                .get(key.0.as_str())
                .copied()
                .unwrap_or(b.schema_version);
            out.push(Envelope {
                deleted: true,
                ..envelope(key, run_rev, device_id, v, serde_json::Value::Null)
            });
        }
    }
    // Push in section order (the bundle's), so a record that another section
    // refers to — an application, before its status history — goes first.
    let rank: BTreeMap<&str, usize> = sections
        .iter()
        .enumerate()
        .map(|(i, s)| (s.section(), i))
        .collect();
    out.sort_by_key(|e| rank.get(e.store.as_str()).copied().unwrap_or(usize::MAX));
    Ok(out)
}

fn envelope(key: &Key, rev: u64, device: &str, v: u32, payload: serde_json::Value) -> Envelope {
    Envelope {
        store: key.0.clone(),
        record_id: key.1.clone(),
        rev,
        device_id: device.to_string(),
        deleted: false,
        schema_version: v,
        payload,
    }
}

fn entry_of(env: &Envelope, hash: String) -> BaselineEntry {
    BaselineEntry {
        hash,
        rev: env.rev,
        device_id: env.device_id.clone(),
        schema_version: env.schema_version,
        deleted: env.deleted,
    }
}

fn baseline_map(state: &SyncStateStore, scope: &str) -> AppResult<BTreeMap<Key, BaselineEntry>> {
    Ok(state
        .baseline(scope)?
        .into_iter()
        .map(|(store, id, e)| ((store, id), e))
        .collect())
}

fn current_records(
    sections: &[&dyn SyncSection],
) -> AppResult<BTreeMap<Key, (u32, serde_json::Value)>> {
    let mut out = BTreeMap::new();
    for s in sections {
        for rec in s.sync_records()? {
            out.insert(
                (s.section().to_string(), rec.id),
                (s.schema_version(), rec.payload),
            );
        }
    }
    Ok(out)
}

/// A baseline hash written before `float_roundtrip` (#1435) can differ from
/// the same unchanged record's hash now. Adopt the new hash silently so such a
/// record is not mistaken for a local edit that beats a pending remote update.
fn rebaseline_legacy_floats(
    state: &SyncStateStore,
    scope: &str,
    records: &BTreeMap<Key, (u32, serde_json::Value)>,
    current: &Hashes,
    baseline: &mut BTreeMap<Key, BaselineEntry>,
) -> AppResult<()> {
    for (key, (_, payload)) in records {
        let Some(entry) = baseline.get_mut(key).filter(|e| !e.deleted) else {
            continue;
        };
        let Some(now) = current.get(key) else {
            continue;
        };
        if entry.hash != *now && entry.hash == legacy_record_hash(payload) {
            entry.hash.clone_from(now);
            state.put_baseline(scope, &key.0, &key.1, entry)?;
        }
    }
    Ok(())
}

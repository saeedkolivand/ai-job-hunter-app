//! Two simulated devices with every real store of the export bundle, synced
//! through the in-memory server: the per-store `SyncSection` impls end to end.

use super::*;
use crate::ai_config::AiConfigStore;
use crate::ai_generations::AiGenerationStore;
use crate::applications::{ApplicationStatusEvents, ApplicationStore};
use crate::autopilot::AutopilotStore;
use crate::contact_profile::ContactProfileStore;
use crate::data_store::DataStore;
use crate::dedup::DedupStore;
use crate::discovered::DiscoveredCompanyStore;
use crate::documents::DocumentStore;
use crate::job_preferences::JobPreferencesStore;
use crate::pipeline::runs::PipelineRunStore;
use crate::postings::{InteractionRecord, InteractionStore, InteractionsSection};
use crate::referrals::ReferralStore;
use crate::spend::SpendStore;

/// One install: a data dir with every bundle store opened in it.
pub(super) struct RealDevice {
    _dir: TempDir,
    state: SyncStateStore,
    documents: DocumentStore,
    generations: AiGenerationStore,
    applications: ApplicationStore,
    prefs: JobPreferencesStore,
    contact: ContactProfileStore,
    ai_config: AiConfigStore,
    pub(super) referrals: ReferralStore,
    autopilots: AutopilotStore,
    interactions: Mutex<InteractionStore>,
    spend: SpendStore,
    dedup: DedupStore,
    discovered: DiscoveredCompanyStore,
    runs: PipelineRunStore,
}

impl RealDevice {
    pub(super) fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let p = dir.path().to_path_buf();
        Self {
            state: SyncStateStore::open(&p).unwrap(),
            documents: DocumentStore::open(&p).unwrap(),
            generations: AiGenerationStore::open(&p).unwrap(),
            applications: ApplicationStore::open(&p).unwrap(),
            prefs: JobPreferencesStore::open(&p).unwrap(),
            contact: ContactProfileStore::open(&p).unwrap(),
            ai_config: AiConfigStore::open(&p).unwrap(),
            referrals: ReferralStore::open(&p).unwrap(),
            autopilots: AutopilotStore::new(&p),
            interactions: Mutex::new(InteractionStore::new(&p)),
            spend: SpendStore::open(&p).unwrap(),
            dedup: DedupStore::open(&p).unwrap(),
            discovered: DiscoveredCompanyStore::open(&p).unwrap(),
            runs: PipelineRunStore::open(&p).unwrap(),
            _dir: dir,
        }
    }

    /// Run `f` over every section, in bundle order.
    fn with_sections<R>(&self, f: impl FnOnce(&[&dyn SyncSection]) -> R) -> R {
        let events = ApplicationStatusEvents(&self.applications);
        let interactions = InteractionsSection(&self.interactions);
        let sections: [&dyn SyncSection; 14] = [
            &self.documents,
            &self.generations,
            &self.applications,
            &events,
            &self.prefs,
            &self.contact,
            &self.ai_config,
            &self.referrals,
            &self.autopilots,
            &interactions,
            &self.spend,
            &self.dedup,
            &self.discovered,
            &self.runs,
        ];
        f(&sections)
    }

    /// One run against any transport (the in-memory server or real HTTP).
    pub(super) fn sync_via(&self, transport: &dyn Transport, rev: u64) -> AppResult<SyncReport> {
        self.with_sections(|s| run(s, &self.state, transport, "scope-a", rev))
    }

    fn sync(&self, server: &FakeServer, rev: u64) -> SyncReport {
        self.with_sections(|s| run(s, &self.state, server, "scope-a", rev))
            .unwrap()
    }

    /// Every section's records, for comparing two devices.
    pub(super) fn records(&self) -> BTreeMap<(String, String), Value> {
        self.with_sections(|sections| {
            let mut out = BTreeMap::new();
            for s in sections {
                for r in s.sync_records().unwrap() {
                    out.insert((s.section().to_string(), r.id), r.payload);
                }
            }
            out
        })
    }

    pub(super) fn count(&self, section: &str) -> usize {
        self.records().keys().filter(|(s, _)| s == section).count()
    }

    /// One record in every array section, written through each store's own
    /// backup import (the bundle shape is the sync payload shape).
    pub(super) fn seed(&self) {
        let import = |s: &dyn DataStore, v: Value| {
            s.import(&v).unwrap();
        };
        import(
            &self.documents,
            json!([{"_id": "doc-1", "title": "CV", "name": "cv.pdf",
            "text": "Rust engineer", "createdAt": 1, "indexed": false, "isDefault": true}]),
        );
        import(
            &self.generations,
            json!([{"id": "gen-1", "createdAt": 1, "candidateName": "N",
            "jobTitle": "Dev", "companyName": "Acme", "resumeLanguage": "en",
            "jobAdLanguage": "en", "targetLanguage": "en", "mismatch": false,
            "topRequirements": ["rust"], "mode": "full", "resumeText": "r",
            "coverLetterText": "c", "jobAd": "ad"}]),
        );
        import(
            &self.applications,
            json!([{"id": "app-1", "status": "applied",
            "appliedAt": 2, "createdAt": 1, "updatedAt": 2, "jobUrl": "https://acme.test/1",
            "board": "manual", "company": "Acme", "title": "Dev", "candidate": "N",
            "answers": [], "brief": "", "nextActionAt": null}]),
        );
        let events = ApplicationStatusEvents(&self.applications);
        import(
            &events,
            json!([{"applicationId": "app-1", "fromStatus": "", "toStatus":
            "applied", "at": 2, "note": "", "source": "user", "confirmed": true}]),
        );
        import(
            &self.referrals,
            json!([{"id": "ref-1", "jobUrl": "u", "companyName": "Acme",
            "personName": "P", "channel": "email", "status": "draft", "createdAt": 1,
            "updatedAt": 1}]),
        );
        self.autopilots.create(json!({"name": "Rust in Milan"}));
        self.interactions.lock().upsert(InteractionRecord {
            job_id: "job-1".into(),
            interaction_type: "bookmarked".into(),
            timestamp: 1,
            title: "Dev".into(),
            company: "Acme".into(),
            url: "u".into(),
            source: "s".into(),
            location: "Milan".into(),
        });
        import(
            &self.spend,
            json!([{"id": "sp-1", "createdAt": 1, "provider": "ollama",
            "model": "m", "inputTokens": 1, "outputTokens": 2, "thinkingTokens": 0,
            "estCostUsd": 0.00009500900000000001}]),
        );
        import(
            &self.dedup,
            json!([{"keyA": "a", "keyB": "b", "createdAt": 1}]),
        );
        import(
            &self.discovered,
            json!([{"atsKind": "greenhouse", "slug": "acme",
            "firstSeenAt": 1, "lastSeenAt": 2, "seenCount": 3, "source": "scan",
            "starred": true}]),
        );
        import(
            &self.runs,
            json!({"runs": [{"id": "run-1", "jobUrl": "https://acme.test/1",
            "kind": "resume", "depth": "full", "status": "done", "startedAt": 1}],
            "events": [{"runId": "run-1", "seq": 0, "ts": 1, "stage": "parse",
            "phase": "start", "artifactJson": "{}"}]}),
        );
    }
}

#[test]
fn every_section_reaches_a_fresh_device_unchanged() {
    let server = FakeServer::new();
    let (a, b) = (RealDevice::new(), RealDevice::new());
    a.seed();
    a.sync(&server, 10);
    b.sync(&server, 20);
    assert_eq!(a.records(), b.records());
    for section in [
        "documents",
        "aiGenerations",
        "applications",
        "applicationStatusEvents",
        "referrals",
        "autopilots",
        "interactions",
        "spend",
        "dedupTombstones",
        "discoveredCompanies",
    ] {
        assert_eq!(b.count(section), 1, "{section}");
    }
    assert_eq!(b.count("pipelineRuns"), 2, "a run and its event");
    // Nothing echoes back: a second round pushes nothing.
    let before = server.len();
    a.sync(&server, 30);
    b.sync(&server, 40);
    assert_eq!(server.len(), before);
}

#[test]
fn edits_and_deletes_converge_across_every_store() {
    let server = FakeServer::new();
    let (a, b) = (RealDevice::new(), RealDevice::new());
    a.seed();
    a.sync(&server, 10);
    b.sync(&server, 20);

    b.referrals.delete_record("ref-1").unwrap();
    b.applications
        .set_status(
            "app-1",
            crate::applications::ApplicationStatus::Interviewing,
            "call",
        )
        .unwrap();
    b.dedup
        .delete_record(&crate::data_store::composite_id(&["a", "b"]))
        .unwrap();
    b.sync(&server, 30);
    a.sync(&server, 40);

    assert_eq!(a.records(), b.records());
    assert_eq!(a.count("referrals"), 0);
    assert_eq!(a.count("dedupTombstones"), 0);
    assert_eq!(a.count("applicationStatusEvents"), 2, "history unions");
    assert_eq!(
        a.applications.get("app-1").unwrap().status,
        crate::applications::ApplicationStatus::Interviewing
    );
}

#[test]
fn a_document_travels_without_its_vector_or_index_flag() {
    let server = FakeServer::new();
    let (a, b) = (RealDevice::new(), RealDevice::new());
    a.seed();
    a.sync(&server, 10);
    b.sync(&server, 20);
    let payload = &b.records()[&("documents".to_string(), "doc-1".to_string())];
    assert!(payload.get("vector").is_none() && payload.get("indexed").is_none());
    assert!(b.documents.get("doc-1").unwrap().is_default);
}

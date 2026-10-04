//! The résumé snapshot vector and scores die with the autopilot that produced them.

use super::super::drop_orphaned_resume_cache;
use super::support::snapshot_score_key;
use crate::autopilot::Autopilot;

// ── the résumé snapshot vector dies with its autopilot ────────────────────

/// `autopilot-resume:<sha>` is résumé-derived user content in a cache bounded
/// only by a TTL and a row cap. Deleting the autopilot must delete it too,
/// rather than leaving it readable for up to the TTL (7 days by default).
#[test]
fn deleting_an_autopilot_drops_its_resume_snapshot_vector() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let docs = crate::documents::DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let resume = "rust engineer, kubernetes, distributed systems";
    let id = crate::commands::match_resume::autopilot_resume_id(resume);
    docs.upsert_posting_vector(
        &id,
        &crate::documents::sha256_hex(resume),
        &snapshot_vector(&docs),
    )
    .unwrap();
    assert!(docs.get_posting_vector(&id).is_some(), "seeded");

    drop_orphaned_resume_cache(&docs, Some(resume), &[]);

    assert!(
        docs.get_posting_vector(&id).is_none(),
        "the résumé-derived row must not outlive the record it was derived from"
    );
}

/// …but the id is the CONTENT, so a second autopilot with the same résumé is
/// still a live producer of that row: deleting it would just re-embed.
#[test]
fn a_resume_shared_with_another_autopilot_keeps_its_vector() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let docs = crate::documents::DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let resume = "rust engineer, kubernetes, distributed systems";
    let id = crate::commands::match_resume::autopilot_resume_id(resume);
    docs.upsert_posting_vector(
        &id,
        &crate::documents::sha256_hex(resume),
        &snapshot_vector(&docs),
    )
    .unwrap();

    let survivor = autopilot_with_resume(Some(resume));
    drop_orphaned_resume_cache(&docs, Some(resume), std::slice::from_ref(&survivor));
    assert!(docs.get_posting_vector(&id).is_some());

    // A remaining autopilot with a DIFFERENT résumé is not a producer of it.
    let other = autopilot_with_resume(Some("python data engineer"));
    drop_orphaned_resume_cache(&docs, Some(resume), std::slice::from_ref(&other));
    assert!(docs.get_posting_vector(&id).is_none());
}

/// The vector is only half the résumé's cache footprint: every `match_scores`
/// row keyed on `autopilot-resume:<sha>` holds résumé-DERIVED content too — the
/// gaps, the recommendations and the explanation all describe that résumé — and
/// nothing else can ever reach those rows once the record is gone.
#[test]
fn dropping_a_resume_snapshot_also_drops_the_scores_it_produced() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let docs = crate::documents::DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let resume = "rust engineer, kubernetes, distributed systems";
    let id = crate::commands::match_resume::autopilot_resume_id(resume);
    docs.upsert_posting_vector(
        &id,
        &crate::documents::sha256_hex(resume),
        &snapshot_vector(&docs),
    )
    .unwrap();
    // Two scored jobs for this résumé, plus one for an unrelated one.
    let job_hash = crate::documents::sha256_hex("We need a Rust engineer");
    let other_resume = crate::commands::match_resume::autopilot_resume_id("python data engineer");
    for resume_id in [&id, &id, &other_resume] {
        docs.upsert_match_score(
            &snapshot_score_key(resume_id, "autopilot:job-1", &job_hash),
            "{\"combined\":91,\"gaps\":[\"kubernetes\"]}",
        )
        .unwrap();
    }
    assert!(
        docs.get_match_score(&snapshot_score_key(&id, "autopilot:job-1", &job_hash))
            .is_some(),
        "seeded"
    );

    drop_orphaned_resume_cache(&docs, Some(resume), &[]);

    assert!(
        docs.get_match_score(&snapshot_score_key(&id, "autopilot:job-1", &job_hash))
            .is_none(),
        "the résumé's cached scores must die with it, not linger for the TTL"
    );
    assert!(
        docs.get_match_score(&snapshot_score_key(
            &other_resume,
            "autopilot:job-1",
            &job_hash
        ))
        .is_some(),
        "…and the delete must be scoped to THAT résumé — another autopilot's \
         scores are untouched"
    );
}

/// The UPDATE shape, which shipped with exactly the defect the DELETE path had
/// just closed: the cache id is `sha256(resume_text)`, so replacing the text
/// orphans the old rows just as thoroughly as deleting the record.
///
/// No before/after diff is needed to see it — the post-mutation snapshot still
/// contains this record carrying its NEW text, so the old text has no producer
/// left; an update that did NOT touch the résumé leaves the record as its own
/// producer and keeps the rows.
#[test]
fn editing_an_autopilots_resume_orphans_the_previous_snapshot() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let docs = crate::documents::DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let before = "rust engineer, kubernetes, distributed systems";
    let id = crate::commands::match_resume::autopilot_resume_id(before);
    let job_hash = crate::documents::sha256_hex("We need a Rust engineer");
    let seed = || {
        docs.upsert_posting_vector(
            &id,
            &crate::documents::sha256_hex(before),
            &snapshot_vector(&docs),
        )
        .unwrap();
        docs.upsert_match_score(
            &snapshot_score_key(&id, "autopilot:job-1", &job_hash),
            "{\"combined\":91}",
        )
        .unwrap();
    };

    // An update that did NOT change the résumé: the record still carries it.
    seed();
    let unchanged = autopilot_with_resume(Some(before));
    drop_orphaned_resume_cache(&docs, Some(before), std::slice::from_ref(&unchanged));
    assert!(
        docs.get_posting_vector(&id).is_some(),
        "an update that leaves resume_text alone must not evict its own cache"
    );

    // An update that REPLACED the résumé: nothing produces the old rows now.
    let edited = autopilot_with_resume(Some("staff platform engineer, terraform"));
    drop_orphaned_resume_cache(&docs, Some(before), std::slice::from_ref(&edited));
    assert!(
        docs.get_posting_vector(&id).is_none(),
        "the replaced résumé's snapshot vector is unreachable — it must not survive the TTL"
    );
    assert!(
        docs.get_match_score(&snapshot_score_key(&id, "autopilot:job-1", &job_hash))
            .is_none(),
        "…and neither may the scores it produced"
    );
}

/// A vector in the active embedding space, so `get_posting_vector` reads it back.
fn snapshot_vector(
    docs: &crate::documents::DocumentStore,
) -> crate::commands::ai_provider::EmbeddingVector {
    let active = docs.embedding_config();
    crate::commands::ai_provider::EmbeddingVector {
        values: vec![0.1, 0.2, 0.3],
        space: crate::commands::ai_provider::EmbeddingSpace {
            provider: active.provider,
            model: active.model,
            dim: 3,
            version: crate::commands::ai_provider::EMBEDDING_VECTOR_VERSION,
        },
    }
}

fn autopilot_with_resume(resume_text: Option<&str>) -> Autopilot {
    let mut ap: Autopilot = serde_json::from_value(serde_json::json!({
        "_id": "ap-1",
        "name": "n",
        "status": "active",
        "target": { "boards": [], "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 0.0 },
        "schedule": "manual",
        "totalFound": 0,
        "totalApplied": 0,
        "createdAt": 0,
        "updatedAt": 0,
    }))
    .expect("autopilot fixture");
    ap.resume_text = resume_text.map(String::from);
    ap
}

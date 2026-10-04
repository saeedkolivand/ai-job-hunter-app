use super::{support::*, *};

#[test]
fn attach_interactions_joins_records_by_job_id() {
    // `scrape_list_postings` joins InteractionStore records onto each posting so
    // the jobs list can render viewed/applied/saved badges. The join keys on the
    // posting's string `"id"` == record `job_id`, and serializes the records in
    // the renderer's camelCase `JobInteraction` shape.
    let items = vec![
        serde_json::json!({"id": "1", "title": "Has two"}),
        serde_json::json!({"id": "2", "title": "Has one"}),
        serde_json::json!({"id": "3", "title": "Has none"}),
        serde_json::json!({"title": "No id"}),
    ];
    let interactions = vec![
        interaction("1", "viewed"),
        interaction("1", "applied"),
        interaction("2", "bookmarked"),
    ];

    let joined = attach_interactions(&items, &interactions);
    assert_eq!(joined.len(), 4, "every input item is returned, in order");

    // Item "1" collects both of its interactions, exposed under camelCase keys.
    let first = joined[0]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("item 1 must carry an interactions array");
    assert_eq!(first.len(), 2, "item 1 has two interactions");
    let types: Vec<&str> = first
        .iter()
        .filter_map(|i| i.get("interactionType").and_then(serde_json::Value::as_str))
        .collect();
    assert!(
        types.contains(&"viewed") && types.contains(&"applied"),
        "item 1 must carry viewed + applied under the camelCase interactionType key, got {types:?}"
    );

    // The projected object must carry EXACTLY the `JobInteraction` contract keys —
    // no extra storage-only fields can leak if `InteractionRecord` grows later.
    let obj = first[0]
        .as_object()
        .expect("each interaction must be a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "company",
            "interactionType",
            "jobId",
            "location",
            "source",
            "timestamp",
            "title",
            "url",
        ],
        "interaction object must contain exactly the JobInteraction contract keys"
    );

    // Item "2" gets exactly its one interaction.
    let second = joined[1]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("item 2 must carry an interactions array");
    assert_eq!(second.len(), 1, "item 2 has one interaction");

    // Item "3" has an id but no recorded interactions → empty array (stable shape).
    let third = joined[2]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("item 3 must carry an interactions array even with no matches");
    assert!(
        third.is_empty(),
        "an id with no interactions gets an empty array"
    );

    // The id-less item must not panic and gets an empty array too.
    let fourth = joined[3]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("an id-less item must still get an interactions array");
    assert!(fourth.is_empty(), "an id-less item gets an empty array");
}

#[test]
fn attach_interactions_clamps_unknown_interaction_type_to_viewed() {
    // The persisted `interaction_type` is a free String on disk, but the shared
    // `JobInteraction` contract is a strict union. A corrupt/out-of-union value
    // must be coerced to "viewed" so it never breaks the cross-layer contract;
    // valid types pass through unchanged.
    let items = vec![serde_json::json!({"id": "1"})];
    let interactions = vec![
        interaction("1", "garbage"),
        interaction("1", "applied"),
        interaction("1", "opened"),
        interaction("1", "dismissed"),
    ];

    let joined = attach_interactions(&items, &interactions);
    let arr = joined[0]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("item 1 must carry an interactions array");

    let types: Vec<&str> = arr
        .iter()
        .filter_map(|i| i.get("interactionType").and_then(serde_json::Value::as_str))
        .collect();
    assert_eq!(
        types,
        ["viewed", "applied", "opened", "dismissed"],
        "an unknown type is coerced to \"viewed\"; valid types (including \"dismissed\",
        load-bearing for Best Matches' dismiss filter) pass through unchanged"
    );
}

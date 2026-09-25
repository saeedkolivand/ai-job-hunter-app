use super::*;
// ── `resources/*` + `prompts/*` (issue #1146 P4/P5) ──────────────────────

/// A dispatch stub that answers with the VERB it was given, never a fixed stub — so two call
/// sites that silently built different `Verb`s (e.g. a resource path that dropped a filter the
/// tool path sets) produce visibly different text instead of the identical stub answer masking
/// it.
fn dispatch_echo(verb: &Verb) -> Result<Value, &'static str> {
    Ok(json!({ "ok": true, "resource": verb.resource_name(), "verb": format!("{verb:?}") }))
}

#[test]
fn initialize_advertises_resources_and_prompts_capabilities() {
    let result = initialize_result(&json!({}), INSTRUCTIONS);
    assert_eq!(result["capabilities"]["resources"], json!({}));
    assert_eq!(result["capabilities"]["prompts"], json!({}));
}

#[test]
fn resources_list_advertises_profile_and_best_matches() {
    let list = resources::resources_list();
    let uris: Vec<&str> = list.iter().map(|r| r["uri"].as_str().unwrap()).collect();
    assert_eq!(
        uris,
        vec![resources::URI_PROFILE, resources::URI_BEST_MATCHES]
    );
    for r in &list {
        assert_eq!(r["mimeType"], "application/json", "{r}");
    }
}

#[test]
fn resource_templates_list_advertises_the_job_template() {
    let templates = resources::resource_templates();
    assert_eq!(templates.len(), 1, "{templates:?}");
    assert_eq!(templates[0]["uriTemplate"], "ajh://job/{url}");
    assert_eq!(templates[0]["name"], TOOL_JOB);
}

#[test]
fn resources_read_missing_uri_is_invalid_params() {
    assert!(matches!(
        resources::classify_resource_read(&json!({})),
        resources::ResourceCall::Local(Err((-32602, "Invalid params")))
    ));
}

#[test]
fn resources_read_unknown_uri_is_resource_not_found() {
    assert!(matches!(
        resources::classify_resource_read(&json!({ "uri": "ajh://nope" })),
        resources::ResourceCall::Local(Err((-32002, "Resource not found")))
    ));
}

/// T7 (PR #1184 CodeRabbit review): `ajh://job/` with an empty (or whitespace-only, once
/// percent-decoded) tail must be refused locally, before any bridge call — the `job` tool itself
/// would never accept an empty `url`, and a resource read reaching the bridge with one paid for a
/// round trip no successful outcome could ever come back from.
#[test]
fn resources_read_empty_job_url_is_resource_not_found() {
    for uri in ["ajh://job/", "ajh://job/%20", "ajh://job/   "] {
        assert!(
            matches!(
                resources::classify_resource_read(&json!({ "uri": uri })),
                resources::ResourceCall::Local(Err((-32002, "Resource not found")))
            ),
            "{uri} must be refused locally as resource-not-found"
        );
    }
}

#[test]
fn resources_read_malformed_job_percent_encoding_is_resource_not_found() {
    // `%FF` decodes to a lone byte that is not valid UTF-8 on its own — the one shape
    // `urlencoding::decode` actually errors on (an unrecognized escape like `%zz` passes through
    // literally instead, so it is not the case this test needs).
    assert!(matches!(
        resources::classify_resource_read(&json!({ "uri": "ajh://job/%FF" })),
        resources::ResourceCall::Local(Err((-32002, "Resource not found")))
    ));
}

#[test]
fn resources_read_profile_and_best_matches_build_the_same_verb_the_tools_do() {
    let cases = [
        (resources::URI_PROFILE, Verb::Profile),
        (
            resources::URI_BEST_MATCHES,
            Verb::BestMatches {
                limit: None,
                cursor: None,
                query: None,
            },
        ),
    ];
    for (uri, expected) in cases {
        match resources::classify_resource_read(&json!({ "uri": uri })) {
            resources::ResourceCall::Bridge(got_uri, verb) => {
                assert_eq!(got_uri, uri);
                assert_eq!(verb, expected);
            }
            resources::ResourceCall::Local(_) => panic!("{uri} must be a bridge call"),
        }
    }
}

#[test]
fn resources_read_job_percent_decodes_the_url_into_the_same_verb_the_tool_builds() {
    let url = "https://example.com/x?y=1 2&z=ä";
    let uri = format!("ajh://job/{}", urlencoding::encode(url));
    match resources::classify_resource_read(&json!({ "uri": uri })) {
        resources::ResourceCall::Bridge(got_uri, Verb::Job { url: got_url }) => {
            assert_eq!(got_uri, uri);
            assert_eq!(got_url, url);
        }
        _ => panic!("a job uri must be a bridge call carrying the decoded url"),
    }
}

/// The literal ask behind issue #1146 P4: a resource and its identically-named tool must return
/// BYTE-IDENTICAL text for the same input, because both share the same [`Verb`],
/// [`results::dispatch_payload`] and [`results::capped_result_text`]. Run through the REAL
/// [`serve`] loop (not the pure classify fns alone) with [`dispatch_echo`], so a regression that
/// built a different `Verb` on one of the two paths would answer with visibly different text
/// instead of an identical fixed stub masking it.
#[test]
fn profile_tool_and_resource_return_byte_identical_text() {
    let input = format!(
        "{}{}",
        line(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": TOOL_PROFILE, "arguments": {} },
        })),
        line(json!({
            "jsonrpc": "2.0", "id": 2, "method": "resources/read",
            "params": { "uri": resources::URI_PROFILE },
        })),
    );
    let frames = parsed_frames(&run_serve(&input, dispatch_echo));
    let tool_text = frame_with_id(&frames, 1)["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let resource_text = frame_with_id(&frames, 2)["result"]["contents"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(tool_text, resource_text);
}

#[test]
fn best_matches_tool_and_resource_return_byte_identical_text() {
    let input = format!(
        "{}{}",
        line(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": TOOL_BEST_MATCHES, "arguments": {} },
        })),
        line(json!({
            "jsonrpc": "2.0", "id": 2, "method": "resources/read",
            "params": { "uri": resources::URI_BEST_MATCHES },
        })),
    );
    let frames = parsed_frames(&run_serve(&input, dispatch_echo));
    let tool_text = frame_with_id(&frames, 1)["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let resource_text = frame_with_id(&frames, 2)["result"]["contents"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(tool_text, resource_text);
}

#[test]
fn job_tool_and_resource_return_byte_identical_text_for_a_percent_encoded_url() {
    let url = "https://example.com/jobs?id=42&ref=abc def";
    let input = format!(
        "{}{}",
        line(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": TOOL_JOB, "arguments": { "url": url } },
        })),
        line(json!({
            "jsonrpc": "2.0", "id": 2, "method": "resources/read",
            "params": { "uri": format!("ajh://job/{}", urlencoding::encode(url)) },
        })),
    );
    let frames = parsed_frames(&run_serve(&input, dispatch_echo));
    let tool_text = frame_with_id(&frames, 1)["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let resource_text = frame_with_id(&frames, 2)["result"]["contents"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(tool_text, resource_text);
}

#[test]
fn resources_list_and_templates_list_answer_without_a_bridge_call() {
    let input = format!(
        "{}{}",
        line(json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" })),
        line(json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/templates/list" })),
    );
    let dispatched = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&dispatched);
    let frames = parsed_frames(&run_serve(&input, move |_: &Verb| {
        flag.store(true, Ordering::SeqCst);
        Ok(json!({ "ok": true }))
    }));
    assert!(
        !dispatched.load(Ordering::SeqCst),
        "resources/list and resources/templates/list must never touch the bridge"
    );
    assert_eq!(
        frame_with_id(&frames, 1)["result"]["resources"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        frame_with_id(&frames, 2)["result"]["resourceTemplates"][0]["uriTemplate"],
        "ajh://job/{url}"
    );
}

//! CWE-639 regression: a different connection's registry can never see (let
//! alone cancel) another connection's stream.

use super::super::AssistStreamRegistry;

#[test]
fn take_on_a_different_connections_registry_never_sees_another_connections_stream() {
    // Two independent registries — one per connection, exactly as
    // `handle_connection` creates a fresh one per socket.
    let connection_a = AssistStreamRegistry::default();
    let connection_b = AssistStreamRegistry::default();

    let gen_a = connection_a.begin("req-1").expect("a fresh reqId");
    connection_a.register("req-1", gen_a, "job-1");

    // Connection B never registered "req-1" — it must be a no-op, NEVER
    // able to observe (let alone cancel) connection A's stream.
    assert_eq!(
        connection_b.take("req-1"),
        None,
        "a different connection's registry must never see this reqId"
    );

    // Connection A can still cancel its own stream — the isolation is
    // per-connection, not "nobody can ever cancel it".
    assert_eq!(connection_a.take("req-1"), Some("job-1".to_string()));
}

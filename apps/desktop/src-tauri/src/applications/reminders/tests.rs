use super::*;

#[test]
fn the_sql_terminal_list_is_derived_from_is_terminal() {
    // Drift guard: the SQL predicate must name EXACTLY the statuses
    // `is_terminal` reports, so adding a terminal stage cannot leave the
    // claim's WHERE behind. `ghosted` is soft-terminal and must stay out.
    let list = terminal_status_sql_list();
    for status in ApplicationStatus::ALL {
        let quoted = format!("'{}'", status.as_id());
        assert_eq!(
            list.contains(&quoted),
            status.is_terminal(),
            "{status:?} membership in the SQL list must follow is_terminal()"
        );
    }
    assert_eq!(list, "'accepted','rejected','withdrawn'");
}

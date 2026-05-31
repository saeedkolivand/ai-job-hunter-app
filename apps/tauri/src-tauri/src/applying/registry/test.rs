use super::*;

#[test]
fn test_catalog_not_empty() {
    let catalog = ApplierRegistry::catalog();
    assert!(!catalog.is_empty());
}

#[test]
fn test_catalog_contains_known_boards() {
    let catalog = ApplierRegistry::catalog();
    let board_ids: Vec<&str> = catalog.iter().map(|(id, _)| *id).collect();
    assert!(board_ids.contains(&"linkedin"));
    assert!(board_ids.contains(&"indeed"));
    assert!(board_ids.contains(&"greenhouse"));
}

#[test]
fn test_catalog_display_names() {
    let catalog = ApplierRegistry::catalog();
    let linkedin = catalog.iter().find(|(id, _)| *id == "linkedin");
    assert_eq!(linkedin, Some(&("linkedin", "LinkedIn")));
}

#[test]
fn test_get_known_board() {
    let applier = ApplierRegistry::get("linkedin");
    assert!(applier.is_some());
}

#[test]
fn test_get_unknown_board() {
    let applier = ApplierRegistry::get("unknown_board");
    assert!(applier.is_none());
}

#[test]
fn test_get_all_known_boards() {
    let known_boards = [
        "linkedin",
        "indeed",
        "greenhouse",
        "workday",
        "xing",
        "glassdoor",
    ];
    for board_id in known_boards {
        assert!(
            ApplierRegistry::get(board_id).is_some(),
            "Failed for {}",
            board_id
        );
    }
}

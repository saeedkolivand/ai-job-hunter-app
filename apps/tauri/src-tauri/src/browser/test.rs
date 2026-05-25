use super::*;

#[test]
fn test_browser_controller_options_default() {
    let opts = BrowserControllerOptions::default();
    assert!(opts.headless);
    assert_eq!(opts.locale, "en-US");
    assert_eq!(opts.viewport_width, 1366);
    assert_eq!(opts.viewport_height, 900);
    assert!(!opts.user_agent.is_empty());
}

#[test]
fn test_browser_controller_new() {
    let opts = BrowserControllerOptions::default();
    let controller = BrowserController::new(opts);
    assert!(!controller.is_open());
}

#[test]
fn test_browser_controller_is_open() {
    let opts = BrowserControllerOptions::default();
    let controller = BrowserController::new(opts);
    assert_eq!(controller.is_open(), false);
}

#[test]
fn test_browser_controller_options_clone() {
    let opts = BrowserControllerOptions::default();
    let opts2 = opts.clone();
    assert_eq!(opts.headless, opts2.headless);
    assert_eq!(opts.locale, opts2.locale);
}

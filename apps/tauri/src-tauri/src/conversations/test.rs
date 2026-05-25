use super::*;

#[test]
fn test_now_ms() {
    let now = now_ms();
    assert!(now > 0);
}

#[test]
fn test_now_ms_increases() {
    let now1 = now_ms();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let now2 = now_ms();
    assert!(now2 > now1);
}

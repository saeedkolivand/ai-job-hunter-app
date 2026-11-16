use std::cell::Cell;

use super::*;

#[test]
fn a_locked_gate_refuses_and_never_runs() {
    let (refused, ran) = (Cell::new(false), Cell::new(false));
    let handled = gate(
        true,
        (),
        |()| refused.set(true),
        |()| {
            ran.set(true);
            false
        },
    );
    assert!(handled, "a refused command still counts as handled");
    assert!(refused.get());
    assert!(!ran.get(), "no command may run while locked");
}

#[test]
fn an_open_gate_runs_and_returns_the_handler_answer() {
    let refused = Cell::new(false);
    assert!(gate(false, (), |()| refused.set(true), |()| true));
    assert!(!gate(false, (), |()| refused.set(true), |()| false));
    assert!(!refused.get());
}

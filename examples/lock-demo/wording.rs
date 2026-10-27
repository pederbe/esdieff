//! Shared wording for expectation mismatches. It depends on no other demonstration module,
//! so the oracle and the expectation matching can use it without seeing the simulator.
use std::fmt::Display;

/// Describe a difference as "what expected X, observed Y", or nothing when both agree.
pub fn mismatch<T: Display + PartialEq>(what: &str, expected: T, observed: T) -> Option<String> {
    (expected != observed).then(|| format!("{what} expected {expected}, observed {observed}"))
}

/// Items in order, or "none" when there are none.
pub fn listed<T>(items: impl IntoIterator<Item = T>, show: impl Fn(T) -> String) -> String {
    let shown: Vec<String> = items.into_iter().map(show).collect();
    if shown.is_empty() {
        "none".to_owned()
    } else {
        shown.join(", ")
    }
}

pub fn invocation(invoked: bool) -> &'static str {
    if invoked { "invoked" } else { "not invoked" }
}

pub fn result(accepted: bool, state: &str) -> String {
    format!("accepted {accepted} with {state}")
}

pub fn event(state: &str) -> String {
    format!("stateChanged {state}")
}

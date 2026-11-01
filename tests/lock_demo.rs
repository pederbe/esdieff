//! Exercise the documented example through its real SDF operations and fixtures.
#[path = "../examples/lock-demo/checks.rs"]
mod checks;

#[test]
fn example_checks_model_and_matches_both_admitted_and_rejected_values() {
    let mut output = Vec::new();
    checks::run(&mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.starts_with("Door-lock model: passed\n"));
    assert_eq!(output.matches(": admitted (expected admitted)").count(), 4);
    assert_eq!(output.matches(": rejected (expected rejected)").count(), 4);
    assert!(output.ends_with("8 sample expectations matched\n"));
}

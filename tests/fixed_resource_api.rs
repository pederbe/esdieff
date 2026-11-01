//! Ordinary value inspection and comparison need no caller-owned work budget.
use esdieff::{Catalogue, SourceInput};
use std::cmp::Ordering;

#[test]
fn exact_comparison_and_locations_use_fixed_internal_safeguards() {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("values", br#"[10000,1e4,9999]"#).sample()])
            .unwrap();
    let root = catalogue
        .sources()
        .next()
        .unwrap()
        .json()
        .require()
        .unwrap();
    let first = root.at(0).require().unwrap();
    let same = root.at(1).require().unwrap();
    let lower = root.at(2).require().unwrap();
    assert_eq!(first.pointer().require().unwrap(), "/0");
    assert_eq!(
        first
            .number()
            .require()
            .unwrap()
            .compare(same.number().require().unwrap()),
        Ordering::Equal
    );
    assert_eq!(
        first
            .number()
            .require()
            .unwrap()
            .compare(lower.number().require().unwrap()),
        Ordering::Greater
    );
}

//! Revisions and difference lookups shared by the revision comparison tests.
#![allow(dead_code)]
use esdieff::{
    Admission, Catalogue, Difference, Direction, Revision, RevisionComparison, SourceInput,
};
use serde_json::Value;

/// A witness as its value, direction and admission in the earlier and later revision.
pub type WitnessRow = (String, Direction, Admission, Admission);

/// One revision whose documents are supplied under these keys.
pub fn catalogue(documents: &[(&str, &str)]) -> Catalogue {
    let inputs: Vec<_> = documents
        .iter()
        .map(|(key, text)| SourceInput::supplied(key, text.as_bytes()))
        .collect();
    Catalogue::from_inputs(&inputs).unwrap()
}

/// Revise a document through its JSON structure.
pub fn revised(text: &str, revise: impl FnOnce(&mut Value)) -> String {
    let mut document: Value = serde_json::from_str(text).unwrap();
    revise(&mut document);
    document.to_string()
}

/// The pointer of a difference in one revision, which must have it.
pub fn pointer(difference: &Difference<'_>, revision: Revision) -> String {
    difference
        .side(revision)
        .expect("revision has the difference")
        .pointer(8192)
        .require()
        .unwrap()
}

/// The one difference whose later or earlier location ends with `suffix`.
pub fn at<'r>(comparison: &'r RevisionComparison<'r>, suffix: &str) -> Difference<'r> {
    let mut found = comparison.differences().filter(|difference| {
        [Revision::Later, Revision::Earlier].iter().any(|revision| {
            difference.side(*revision).is_some_and(|side| {
                side.pointer(8192)
                    .present()
                    .is_some_and(|pointer| pointer.ends_with(suffix))
            })
        })
    });
    let difference = found
        .next()
        .unwrap_or_else(|| panic!("difference at {suffix}"));
    assert!(found.next().is_none(), "one difference at {suffix}");
    difference
}

/// The difference's witness.
pub fn witness(difference: &Difference<'_>) -> WitnessRow {
    let witness = difference.witness().expect("a witness");
    (
        witness.value().to_owned(),
        witness.direction(),
        witness.admission(Revision::Earlier),
        witness.admission(Revision::Later),
    )
}

/// A Client-supplied witness, admitted by the earlier revision and rejected by the later.
pub fn client(value: &str) -> WitnessRow {
    (
        value.to_owned(),
        Direction::ClientSupplied,
        Admission::Admitted,
        Admission::Rejected,
    )
}

/// A Thing-supplied witness, rejected by the earlier revision and admitted by the later.
pub fn thing(value: &str) -> WitnessRow {
    (
        value.to_owned(),
        Direction::ThingSupplied,
        Admission::Rejected,
        Admission::Admitted,
    )
}

//! Shared Property sample selection under ADR0016: Selected samples with explicit admission and coverage.
use esdieff::{Catalogue, Report, SampleKind, SampleRequest};

pub fn selected(catalogue: &Catalogue) -> Report<'_> {
    catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap()
}

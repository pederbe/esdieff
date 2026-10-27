//! Content-format declaration state and recorded positive registry evidence.
//! Follows ADR 0029: Content-format declarations and recorded registry guidance.
use crate::registry::Facts;
mod compare;
mod grammar;
mod lookup;
mod meaning;
mod scan;
mod table;
pub(crate) use compare::{Unsettled, same_meaning};
pub(crate) use scan::temporary_storage_supported;
pub(crate) use scan::{Rule, scan};
pub(crate) const AREAS: [&str; 2] = ["content-format-syntax/1", "content-format-registry/1"];

#[derive(Default)]
#[repr(C)]
pub(crate) struct State {
    pub(crate) statuses: [u8; 2],
    pub(crate) used: u8,
    pub(crate) needed: u8,
    pub(crate) withheld: u8,
    pub(crate) reasons: u8,
    pub(crate) padding: [u8; 2],
    pub(crate) counts: [u32; 4],
}
impl State {
    fn assessed(&mut self, area: usize) {
        if self.statuses[area] == 0 {
            self.statuses[area] = 1;
        }
    }
    fn defer(&mut self) {
        self.statuses = [2, 2];
    }
    fn increment(&mut self, index: usize) {
        self.counts[index] = self.counts[index]
            .checked_add(1)
            .expect("one precharged observation below work ceiling");
    }
    pub(crate) fn status(&self, area: usize) -> crate::AssessmentStatus {
        match self.statuses[area] {
            0 => crate::AssessmentStatus::NotApplicable,
            1 => crate::AssessmentStatus::Assessed,
            _ => crate::AssessmentStatus::Undetermined,
        }
    }
}

pub(crate) static FACTS: [Facts; 3] = [
    Facts {
        registry: "coap-content-formats",
        snapshot: "iana.coap-content-formats/2026-10-05",
        scope: "relevant_dated_positive_records",
        extraction_policy: "iana-content-format-positives/2",
        digest: "4dfa7aca5dc7d7f98e6d7ec354db0325373d55c12f2add1af096db92bec72a4a",
        raw_digest: "48a9f69ecbebcd8a091c3ec4680d36281da74da8734a133ea6a95d6203f137ea",
        source_updated: "2026-08-11",
        retrieved_at: "2026-10-05T16:55:20Z",
        entries_count: 119,
    },
    Facts {
        registry: "media-types",
        snapshot: "iana.media-types/2026-10-05",
        scope: "relevant_dated_positive_records",
        extraction_policy: "iana-content-format-positives/2",
        digest: "dd7486a8665423f632da7f6d1a5f0c3321e39e94c43c0b983cd88131664f7e08",
        raw_digest: "0cbb8819dfe01c1badc71f8ba836291ba43718a5e797287cd1ac00d49f168ffb",
        source_updated: "2026-09-24",
        retrieved_at: "2026-10-05T16:55:20Z",
        entries_count: 2361,
    },
    Facts {
        registry: "http-content-codings",
        snapshot: "iana.http-content-codings/2026-10-05",
        scope: "relevant_dated_positive_records",
        extraction_policy: "iana-content-format-positives/2",
        digest: "f853d2b95b1e723a3848787bdc48b68946e3fda93b6797338accc6dcc86ee307",
        raw_digest: "e4581c5fd5bd902d9b8de2a8e7cc855e5616e7a9b9d7fc00b070e94092ba2c67",
        source_updated: "2025-10-02",
        retrieved_at: "2026-10-05T16:55:20Z",
        entries_count: 12,
    },
];

#[cfg(test)]
mod tests {
    #[test]
    fn every_generated_key_and_flag_matches_the_frozen_canonical_records() {
        use super::table;
        use serde_json::Value;
        let coap: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/content-format/coap-positive.json"
        ))
        .unwrap();
        let rows = coap.as_array().unwrap();
        assert_eq!(rows.len(), table::NUMERIC.len());
        assert_eq!(rows.len(), super::FACTS[0].entries_count as usize);
        for (row, projected) in rows.iter().zip(table::NUMERIC) {
            assert_eq!(row["id"].as_u64().unwrap(), u64::from(projected.id));
            assert_eq!(row["flags"]["temporary"], projected.temporary);
            assert_eq!(row["content_type"], projected.content_type);
            assert_eq!(row["coding"].as_str(), projected.coding);
            assert!(projected.content_type.len() + projected.coding.map_or(0, str::len) <= 128);
            assert_eq!(row["flags"]["obsolete"], false);
            assert_eq!(row["flags"]["deprecated"], false);
            assert_eq!(row["temporary_expires"].is_string(), projected.temporary);
        }
        assert!(
            table::NUMERIC
                .windows(2)
                .all(|pair| pair[0].id < pair[1].id)
        );
        assert_eq!(table::NUMERIC.iter().filter(|row| row.temporary).count(), 3);
        for (bytes, table, count, max) in [
            (
                include_str!("../tests/fixtures/content-format/media-positive.json"),
                table::MEDIA,
                2361,
                255,
            ),
            (
                include_str!("../tests/fixtures/content-format/http-positive.json"),
                table::CODINGS,
                12,
                12,
            ),
        ] {
            let json: Value = serde_json::from_str(bytes).unwrap();
            let rows = json.as_array().unwrap();
            assert_eq!(rows.len(), count);
            assert_eq!(rows.len(), table.len());
            assert!(table.windows(2).all(|pair| pair[0].0 < pair[1].0));
            for (row, &(key, flags)) in rows.iter().zip(table) {
                assert_eq!(row["key"], key);
                assert_eq!(row["canonical"].as_str().unwrap().to_ascii_lowercase(), key);
                assert!(key.len() <= max);
                let expected = u8::from(row["flags"]["obsolete"] == true)
                    | u8::from(row["flags"]["deprecated"] == true) << 1
                    | u8::from(row["flags"]["temporary"] == true) << 2;
                assert_eq!(flags, expected, "{key}");
            }
        }
        assert_eq!(table::MEDIA.iter().filter(|row| row.1 & 1 != 0).count(), 24);
        assert_eq!(table::MEDIA.iter().filter(|row| row.1 & 2 != 0).count(), 8);
        assert_eq!(
            table::CODINGS.iter().filter(|row| row.1 & 2 != 0).count(),
            2
        );
        let media: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/content-format/media-positive.json"
        ))
        .unwrap();
        for (key, template) in [("image/x-emf", "image/emf"), ("image/x-wmf", "image/wmf")] {
            let row = media
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["key"] == key)
                .unwrap();
            assert_eq!(row["canonical"], key);
            assert_eq!(row["template"], template);
            assert_eq!(row["flags"]["deprecated"], true);
        }
        assert_eq!(table::RESERVED_CODINGS, &["identity"]);
        assert!(!table::CODINGS.iter().any(|row| row.0 == "identity"));
        assert_eq!(table::EXTRACTION_POLICY, super::FACTS[0].extraction_policy);
    }
    #[test]
    fn content_owner_report_and_real_facts_fit_before_scanning() {
        use std::mem::size_of;
        crate::registry::qualify().unwrap();
        let owner = size_of::<Option<super::State>>();
        let view = size_of::<crate::Field<crate::RegistryKnowledgeView>>();
        let report = size_of::<crate::Report<'_>>();
        println!(
            "content_owner={owner} borrowed_view={view} combined={} Report={report} Finding={}",
            owner + view,
            size_of::<crate::structure::Finding<'_>>()
        );
        assert!(owner + view <= 64);
        assert!(size_of::<crate::structure::Finding<'_>>() <= 160);
        assert!(super::scan::temporary_storage_supported());
        let padded = owner + std::mem::align_of::<crate::Report<'_>>() - 1 + view;
        assert!(padded <= 64, "include the inline owner's alignment padding");
        assert!(crate::source::common_report_supported());
    }

    #[test]
    fn actual_six_record_owner_envelope_is_independently_qualified() {
        use crate::{Field, RecordedRegistryPolicy};
        use std::mem::{align_of, size_of};
        crate::registry::qualify().unwrap();
        assert!(crate::sample::mandatory_storage_supported());
        let state = crate::registry::State {
            consulted: 63,
            interpretations: 7,
            ..Default::default()
        };
        let records = state.wire(true);
        assert_eq!(records.as_array().unwrap().len(), crate::registry::RECORDS);
        assert_eq!(records.as_array().unwrap().capacity(), 8);
        let fallback = records.clone();
        assert!(fallback.as_array().unwrap().capacity() <= 8);
        assert!(serde_json::to_string(&records).unwrap().len() < 3176);
        let withheld = crate::registry::State {
            withheld: 63,
            policy: RecordedRegistryPolicy::WithholdRecorded,
            ..Default::default()
        };
        for index in 0..crate::registry::RECORDS {
            let facts = crate::registry::facts(index).unwrap();
            assert_eq!(
                state.view(facts.registry).require().unwrap().snapshot(),
                Some(facts.snapshot)
            );
            assert_eq!(
                withheld.view(facts.registry).require().unwrap().snapshot(),
                None
            );
            assert!(matches!(
                crate::registry::State::default().view(facts.registry),
                Field::Absent
            ));
            assert!(matches!(
                crate::registry::State {
                    owner_failed: true,
                    requested: 63,
                    ..Default::default()
                }
                .view(facts.registry),
                Field::Unavailable(_)
            ));
        }
        let mut human = crate::human::HumanText::new(crate::HumanStyle::Plain);
        state.write_human(&mut human);
        assert!(human.len() < 6 * 600 + 3 * 64);
        let full_d = 23344 + 512 + 19480 + 14848 + 4608;
        assert_eq!(full_d, 62792);
        assert_eq!(2 * full_d + 208, 4 * crate::registry::FULL_RESERVE);
        println!(
            "Value={} Map={} String={} Vec={} alignment={}/{} real_record_array={} real_human={} content_State={} Option={} padded_owner_plus_view={} sample_mandatory_supported={} two_D_plus_owners={}",
            size_of::<serde_json::Value>(),
            size_of::<serde_json::Map<String, serde_json::Value>>(),
            size_of::<String>(),
            size_of::<Vec<serde_json::Value>>(),
            align_of::<serde_json::Value>(),
            align_of::<serde_json::Map<String, serde_json::Value>>(),
            serde_json::to_string(&records).unwrap().len(),
            human.len(),
            size_of::<super::State>(),
            size_of::<Option<super::State>>(),
            size_of::<Option<super::State>>() + align_of::<crate::Report<'_>>() - 1
                + size_of::<Field<crate::RegistryKnowledgeView>>(),
            crate::sample::mandatory_storage_supported(),
            2 * full_d + 208
        );
    }
}

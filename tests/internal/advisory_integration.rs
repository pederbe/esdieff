//! Model advice keeps sample admission and declaration inspection independently scoped.
//! Follows ADR 0003: Check findings and completeness,
//! ADR 0025: Selected numeric samples, and
//! ADR 0026: Required declaration relationships and source evidence.
use super::coverage_support;
use esdieff::{
    Admission, AssessmentStatus, BasisKind, Catalogue, CheckRequest, Class, Field, InspectRequest,
    Limits, Lookup, RequiredReferenceStatus, SampleKind, SampleRequest, SourceInput, View,
};
use serde_json::Value;

fn status(report: &esdieff::Report<'_>) -> AssessmentStatus {
    report
        .assessment_area_status("advisory-four/1")
        .require()
        .unwrap()
}

fn proofs(report: &esdieff::Report<'_>) -> Vec<Value> {
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["code"].as_str().unwrap().starts_with("proof."))
        .cloned()
        .collect()
}

#[test]
fn model_advice_excludes_sample_roots_and_selected_sample_context() {
    let model = br#"{"info":{},"sdfData":{"empty":{"type":"number","nullable":false,"minimum":2,"maximum":1}},"sdfProperty":{"selected":{"type":"number","nullable":false,"minimum":0,"maximum":4}}}"#;
    let sample_model = br#"{"info":{},"sdfData":{"fake":{"type":"number","nullable":false,"minimum":9,"maximum":8}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model),
        SourceInput::supplied("sample_model", sample_model).sample(),
        SourceInput::supplied("sample", b"3").sample(),
    ])
    .unwrap();
    let model_id = catalogue.source_id("model").unwrap();
    let sample_id = catalogue.source_id("sample").unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.requested_source_ids(), &[model_id]);
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().warnings(), 1);
    assert_eq!(status(&report), AssessmentStatus::Assessed);
    assert_eq!(report.requested_coverage().unassessed(), 0);
    assert_eq!(proofs(&report)[0]["primary"]["root"], model_id.get());
    assert_eq!(
        catalogue.check(CheckRequest::new().root(sample_id)).err(),
        Some("unknown or duplicate selected root")
    );
    let selected = catalogue
        .check_sample(
            SampleRequest::new(model_id, SampleKind::Property, "/sdfProperty/selected")
                .sample(sample_id),
        )
        .unwrap();
    assert_eq!(
        selected.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert!(selected.outcome().complete());
    assert_eq!(selected.outcome().warnings(), 0);
    assert!(proofs(&selected).is_empty());
    assert!(matches!(
        selected.assessment_area_status("advisory-four/1"),
        Field::Absent
    ));
    let wire: Value = serde_json::from_str(&selected.to_json()).unwrap();
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
    let rows = wire["coverage"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 9);
    let numeric = rows
        .iter()
        .find(|row| row["id"] == "sample-numeric/1")
        .unwrap();
    assert_eq!(numeric["basis"]["kind"], "rfc");
    assert_eq!(numeric["basis"]["citation"], "RFC 9880 Appendix C.1");
    assert!(rows.iter().all(|row| row["count_unit"] == "sample_area"));
    let rejected = catalogue
        .check_sample(
            SampleRequest::new(model_id, SampleKind::Data, "/sdfData/empty").sample(sample_id),
        )
        .unwrap();
    assert_eq!(
        rejected.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(rejected.outcome().complete());
    assert_eq!(rejected.outcome().warnings(), 0);
    assert!(proofs(&rejected).is_empty());
    let raw = catalogue
        .inspect(InspectRequest::new(sample_id, ""))
        .unwrap();
    assert!(raw.outcome().complete());
    assert!(proofs(&raw).is_empty());
    assert_eq!(
        catalogue
            .inspect(InspectRequest::new(sample_id, "/missing").required_declarations(true))
            .err(),
        Some("sample source cannot be a model root")
    );
}

#[test]
fn model_classes_and_pairs_share_acquisition_work_from_sample_inputs() {
    let interval = br#"{"info":{},"sdfData":{"first":{"type":"number","nullable":false,"minimum":2,"maximum":1}}}"#;
    let counts = br#"{"info":{},"sdfData":{"second":{"type":"string","nullable":false,"minLength":3,"maxLength":2},"independent":{"type":"boolean","const":""}}}"#;
    for sample in [false, true] {
        let mut inputs = vec![
            SourceInput::supplied("interval", interval),
            SourceInput::supplied("counts", counts),
        ];
        if sample {
            inputs.push(SourceInput::supplied("sample", b"1234").sample());
        }
        let catalogue =
            Catalogue::with_limits(&inputs, Limits::default().numeric_work(10)).unwrap();
        let owned =
            Catalogue::take_inputs_with_limits(inputs, Limits::default().numeric_work(10)).unwrap();
        for catalogue in [&catalogue, &owned] {
            let report = catalogue.check(CheckRequest::new()).unwrap();
            assert_eq!(report.outcome().warnings(), if sample { 2 } else { 3 });
            assert_eq!(
                status(&report),
                if sample {
                    AssessmentStatus::Undetermined
                } else {
                    AssessmentStatus::Assessed
                }
            );
            assert_eq!(report.requested_source_ids().len(), 2);
            assert_eq!(
                report.to_json(),
                catalogue.check(CheckRequest::new()).unwrap().to_json()
            );
            let retained = proofs(&report);
            assert_eq!(
                retained
                    .iter()
                    .filter(|row| row["code"] == "proof.empty_numeric_interval")
                    .count(),
                1
            );
            assert_eq!(
                retained
                    .iter()
                    .filter(|row| row["code"] == "proof.primitive_const_type")
                    .count(),
                1
            );
            assert_eq!(
                retained
                    .iter()
                    .filter(|row| row["code"] == "proof.inverted_text_lengths")
                    .count(),
                usize::from(!sample)
            );
        }
    }
    for (work, warnings, expected) in [
        (12, 3, AssessmentStatus::Undetermined),
        (13, 4, AssessmentStatus::Assessed),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied(
                    "pairs",
                    include_bytes!("../fixtures/advisory/shared-pairs.json"),
                ),
                SourceInput::supplied("sample", b"1").sample(),
            ],
            Limits::default().numeric_work(work),
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().warnings(), warnings);
        assert_eq!(status(&report), expected);
    }
}

#[test]
fn advice_and_required_inspection_keep_their_own_evidence_and_omissions() {
    let model = br##"{"info":{},"sdfObject":{"Base":{"sdfProperty":{"reading":{"type":"number","nullable":false,"minimum":2,"maximum":1}},"sdfRequired":["reading","missing"]},"Copy":{"sdfRef":"#/sdfObject/Base"}}}"##;
    for pointer_limit in [8192, 1] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("model", model),
                SourceInput::supplied("sample", b"0").sample(),
            ],
            Limits::default().pointer_bytes(pointer_limit),
        )
        .unwrap();
        let model_id = catalogue.source_id("model").unwrap();
        let check = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(check.outcome().warnings(), 2);
        assert_eq!(status(&check), AssessmentStatus::Assessed);
        coverage_support::assert_requested_counts(&check);
        let inspection = catalogue
            .inspect(
                InspectRequest::new(model_id, "/sdfObject/Copy")
                    .view(View::Effective)
                    .required_declarations(true),
            )
            .unwrap();
        assert!(proofs(&inspection).is_empty());
        let relationships = inspection.required_declarations().require().unwrap();
        let owner = relationships
            .owners()
            .find(|owner| owner.class().present() == Some(Class::Object))
            .unwrap();
        let reference = owner.references().require().unwrap().next().unwrap();
        assert_eq!(reference.status(), RequiredReferenceStatus::Resolved);
        let Lookup::Found(mut targets) = reference.targets() else {
            panic!("reading declaration")
        };
        let target = targets.next().unwrap();
        assert_eq!(
            target
                .source_definition()
                .require()
                .unwrap()
                .json()
                .pointer(8192)
                .require()
                .unwrap(),
            "/sdfObject/Base/sdfProperty/reading"
        );
        assert_eq!(
            target
                .effective_definition()
                .require()
                .unwrap()
                .json()
                .location()
                .pointer(8192)
                .require()
                .unwrap(),
            "/sdfObject/Copy/sdfProperty/reading"
        );
        assert_eq!(
            reference
                .effective_json()
                .require()
                .unwrap()
                .explanation()
                .consumer()
                .unwrap()
                .pointer(8192)
                .require()
                .unwrap(),
            "/sdfObject/Copy/sdfRef"
        );
        let diagnostics = inspection.diagnostics();
        let missing = diagnostics
            .conditions()
            .find(|row| row.code() == "required.target_missing")
            .unwrap();
        assert_eq!(missing.basis().kind(), BasisKind::Advice);
        assert_eq!(missing.basis().id(), Some("required.target_missing"));
        if pointer_limit == 1 {
            let wire: Value = serde_json::from_str(&inspection.to_json()).unwrap();
            let owner = &wire["result"]["required_declarations"]["owners"][0];
            for location in [&owner["source_owner"], &owner["effective_owner"]] {
                assert!(location["pointer"].is_null());
                assert_eq!(location["omission"]["reason"], "location_limit");
                assert_eq!(location["omission"]["omitted_count"], 1);
            }
            assert!(owner["source_owner"]["span"].is_array());
            assert_eq!(wire["diagnostics"]["truncated"], true);
        }
    }
}

#[test]
fn full_alternative_proofs_keep_coordinates_while_unrelated_sample_failure_is_excluded() {
    let model = include_bytes!("../fixtures/advisory/composition.json");
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model),
        SourceInput::supplied("sample", b"not JSON").sample(),
    ])
    .unwrap();
    let ordinary = Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let baseline = ordinary.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), baseline.outcome().errors());
    assert_eq!(report.outcome().warnings(), 7);
    assert_eq!(status(&report), AssessmentStatus::Undetermined);
    assert_eq!(proofs(&report), proofs(&baseline));
    let retained = proofs(&report);
    assert!(
        retained
            .iter()
            .any(|row| row["arguments"]["alternative"]["name"] == "bad")
    );
    assert!(
        retained
            .iter()
            .all(|row| row["primary"]["pointer"] != "/sdfData/Rows/items")
    );
    assert!(
        retained
            .iter()
            .any(|row| row["primary"]["pointer"] == "/sdfProperty/Inherited")
    );
}

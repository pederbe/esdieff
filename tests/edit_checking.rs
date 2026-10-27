mod location_support;
use esdieff::{
    Assessment, Catalogue, Disposition, Edit, EditRequest, Fidelity, Field, Generation,
    JsonBuilder, Limits, ReportLocation, SourceInput,
};

const SOURCE: &[u8] = br#"{"sdfObject":{"Sensor":{"description":"old","sdfProperty":{"reading":{"type":"number","multipleOf":0.10,"maxLength":18446744073709551616}}}}}"#;

fn request(source: &[u8]) -> EditRequest {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", source)]).unwrap();
    let original = catalogue.source_id("base").unwrap();
    EditRequest::new(catalogue, "correction", original, "revised")
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfObject/Sensor",
                "description",
                JsonBuilder::new().text("corrected").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
}

#[test]
fn requested_initial_and_full_checks_keep_candidate_and_editing_coverage_independent() {
    for (assessment, expected) in [
        (None, Disposition::Passed),
        (Some(Assessment::Initial), Disposition::Passed),
        (Some(Assessment::FullBaseline), Disposition::Failed),
    ] {
        let request = request(SOURCE);
        let request = match assessment {
            Some(assessment) => request.checking(assessment).unwrap(),
            None => request,
        };
        let result = request.edit().unwrap();
        assert_eq!(result.outcome().disposition(), expected);
        assert_eq!(result.generation(), Generation::Complete);
        assert_eq!(result.fidelity(), Fidelity::Established);
        assert!(result.generated_document().is_some());
        assert_eq!(
            result.document_deliverable(),
            expected == Disposition::Passed
        );
        let checking = result.checking();
        assert_eq!(checking.assessment(), assessment);
        assert_eq!(checking.outcome().is_some(), assessment.is_some());
        assert_eq!(result.requested_coverage().total(), Some(1));
        let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
        assert_eq!(wire["request"]["assessment"], serde_json::Value::Null);
        assert_eq!(wire["request"]["inventory"], serde_json::Value::Null);
        assert_eq!(wire["coverage"]["requested"]["count_unit"], "editing");
        assert_eq!(wire["coverage"]["requested"]["total"], 1);
        if let Some(assessment) = assessment {
            let check = &wire["result"]["checking"];
            assert_eq!(check["state"], "requested");
            assert_eq!(check["scope"]["roots"], serde_json::json!([2]));
            assert_eq!(check["scope"], wire["request"]["scope"]);
            assert_eq!(
                check["coverage"]["requested"]["count_unit"],
                "assessment_area"
            );
            if assessment == Assessment::FullBaseline {
                assert_eq!(check["assessment"], "rfc9880/base-1");
                assert_eq!(check["coverage"]["requested"]["total"], 13);
                assert_eq!(check["coverage"]["requirements"]["total"], 54);
                assert!(result.to_json().contains("formal.uint"));
            }
        } else {
            assert_eq!(wire["result"]["checking"]["state"], "not_requested");
            assert!(checking.requested_coverage().is_none());
        }
    }
}

#[test]
fn checking_findings_relate_revised_annotation_to_original_and_typed_intent() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", SOURCE)]).unwrap();
    let original = catalogue.source_id("base").unwrap();
    let result = EditRequest::new(catalogue, "correction", original, "revised")
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfObject/Sensor",
                "description",
                JsonBuilder::new().number("17").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .checking(Assessment::Initial)
        .unwrap()
        .edit()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Failed);
    assert!(result.outcome().complete());
    assert_eq!(result.fidelity(), Fidelity::Established);
    assert!(result.generated_document().is_some());
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    let finding = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "metadata.string")
        .unwrap();
    assert_eq!(finding["primary"]["kind"], "source");
    assert_eq!(
        finding["primary"]["source_id"],
        result.generated_source().get()
    );
    let related = finding["related"].as_array().unwrap();
    let original = related
        .iter()
        .find(|location| location["kind"] == "source" && location["source_id"] == original.get())
        .expect("original bytes");
    assert_eq!(original["pointer"], "/sdfObject/Sensor/description");
    let span = original["span"].as_array().unwrap();
    assert_eq!(
        &SOURCE[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
        br#""old""#
    );
    let intent = related
        .iter()
        .find(|location| location["kind"] == "edit_request")
        .expect("edit intent");
    assert_eq!(intent["pointer"], "/edits/0/value");
    assert_eq!(intent["span"], serde_json::Value::Null);
    assert!(result.to_human().contains("/edits/0/value"));
    let diagnostics = result.diagnostics();
    location_support::assert_all_agree(&diagnostics, &result.to_json());
    let typed = diagnostics
        .conditions()
        .find(|condition| condition.code() == "metadata.string")
        .unwrap()
        .related()
        .find_map(|location| match location {
            ReportLocation::EditRequest(location) => Some(location),
            _ => None,
        })
        .expect("typed edit intent");
    assert_eq!(typed.key(), "correction");
    assert!(matches!(typed.pointer(), Field::Present("/edits/0/value")));
    assert_eq!(typed.span(), None);
}

#[test]
fn strict_initial_warning_failure_retains_complete_faithful_candidate() {
    let result = request(SOURCE)
        .checking(Assessment::Initial)
        .unwrap()
        .strict_warnings(true)
        .edit()
        .unwrap();
    assert_eq!(result.outcome().errors(), 0);
    assert!(result.outcome().warnings() > 0);
    assert_eq!(result.outcome().disposition(), Disposition::Failed);
    assert_eq!(result.fidelity(), Fidelity::Established);
    assert_eq!(result.generation(), Generation::Complete);
    assert!(result.generated_document().is_some());
    assert!(!result.document_deliverable());
    assert!(
        result
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "metadata.info_absent")
    );
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(
        wire["result"]["checking"]["outcome"]["strict_warnings"],
        true
    );
}

const REFERENCING: &[u8] = br##"{"namespace":{"n":"https://example.org/dependency"},"sdfObject":{"Sensor":{"description":"old","sdfProperty":{"reading":{"sdfRef":"n:#/sdfData/reading"}}}}}"##;
const DEPENDENCY: &[u8] = br#"{"namespace":{"n":"https://example.org/dependency"},"defaultNamespace":"n","sdfData":{"reading":{"type":"number"}}}"#;

#[test]
fn checking_scope_uses_needed_dependencies_and_retains_unavailable_candidate() {
    for unavailable in [false, true] {
        let dependency = if unavailable {
            SourceInput::failed("dependency")
        } else {
            SourceInput::supplied("dependency", DEPENDENCY)
        };
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("base", REFERENCING),
            dependency,
            SourceInput::supplied("unrelated", br#"{"sdfThing":{"bad":123}}"#),
        ])
        .unwrap();
        let original = catalogue.source_id("base").unwrap();
        let result = EditRequest::new(catalogue, "correction", original, "revised")
            .unwrap()
            .with_edit(
                Edit::replace(
                    "/sdfObject/Sensor",
                    "description",
                    JsonBuilder::new().text("corrected").unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
            .checking(Assessment::Initial)
            .unwrap()
            .edit()
            .unwrap();
        assert_eq!(result.fidelity(), Fidelity::Established);
        assert_eq!(result.generation(), Generation::Complete);
        assert!(result.generated_document().is_some());
        let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
        let scope = &wire["request"]["scope"];
        assert_eq!(scope["roots"], serde_json::json!([4]));
        assert!(
            !scope["assessed"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(3))
        );
        assert!(
            !scope["assessed"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(1))
        );
        assert_eq!(
            wire["result"]["active_sources"],
            serde_json::json!([2, 3, 4])
        );
        assert_eq!(wire["sources"].as_array().unwrap().len(), 4);
        if unavailable {
            assert_eq!(result.outcome().disposition(), Disposition::Undetermined);
            assert!(!result.document_deliverable());
            assert_eq!(scope["unavailable_required"], serde_json::json!([2]));
            assert_eq!(wire["coverage"]["requested"]["assessed"], 0);
            assert_eq!(wire["coverage"]["requested"]["undetermined"], 1);
            assert_eq!(
                wire["result"]["checking"]["coverage"]["requested"]["count_unit"],
                "assessment_area"
            );
            assert!(
                result
                    .to_human()
                    .contains("Unavailable required sources: [2]")
            );
        } else {
            assert_eq!(result.outcome().disposition(), Disposition::Passed);
            assert!(result.document_deliverable());
            assert_eq!(scope["dependencies"], serde_json::json!([2]));
            assert_eq!(scope["unavailable_required"], serde_json::json!([]));
        }
    }
}

#[test]
fn checking_selects_substituted_contributor_and_preserves_real_alias_ambiguity() {
    let source = br##"{"namespace":{"n":"https://example.org/dependency"},"defaultNamespace":"n","sdfObject":{"Sensor":{"description":"old","sdfProperty":{"reading":{"sdfRef":"n:#/sdfData/reading"}}}},"sdfData":{"reading":{"type":"number"}}}"##;
    for alias in [false, true] {
        let mut inputs = vec![SourceInput::supplied("base", source)];
        if alias {
            inputs.push(SourceInput::supplied("explicit-alias", source));
        }
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let original = catalogue.source_id("base").unwrap();
        let result = EditRequest::new(catalogue, "correction", original, "revised")
            .unwrap()
            .with_edit(
                Edit::replace(
                    "/sdfObject/Sensor",
                    "description",
                    JsonBuilder::new().text("corrected").unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
            .checking(Assessment::Initial)
            .unwrap()
            .edit()
            .unwrap();
        assert_eq!(result.fidelity(), Fidelity::Established);
        assert_eq!(
            result.outcome().disposition(),
            if alias {
                Disposition::Undetermined
            } else {
                Disposition::Passed
            }
        );
        assert_eq!(
            result
                .diagnostics()
                .conditions()
                .any(|condition| condition.code().contains("ambiguous")),
            alias
        );
        assert!(result.catalogue().source(original).is_none());
        assert_eq!(result.original_source().raw_bytes(), source);
    }
}

#[test]
fn checking_request_rejects_repeated_or_unsupported_assessment_and_small_reserve() {
    assert!(
        request(SOURCE)
            .checking(Assessment::Initial)
            .unwrap()
            .checking(Assessment::FullBaseline)
            .is_err()
    );
    assert!(
        request(SOURCE)
            .checking(Assessment::ExpandedModified)
            .is_err()
    );
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("base", SOURCE)],
        Limits::default().report_bytes(70_000),
    )
    .unwrap();
    let original = catalogue.source_id("base").unwrap();
    let request = EditRequest::new(catalogue, "correction", original, "revised").unwrap();
    assert!(request.checking(Assessment::FullBaseline).is_err());
}

fn budgeted_annotation(
    source: &[u8],
    limits: Limits,
    assessment: Assessment,
) -> esdieff::EditResult {
    let catalogue =
        Catalogue::with_limits(&[SourceInput::supplied("base", source)], limits).unwrap();
    let original = catalogue.source_id("base").unwrap();
    EditRequest::new(catalogue, "correction", original, "revised")
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfObject/Sensor",
                "description",
                JsonBuilder::new().number("17").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .checking(assessment)
        .unwrap()
        .edit()
        .unwrap()
}

#[test]
fn mandatory_only_and_location_limited_reports_keep_checking_conclusions() {
    let reference = budgeted_annotation(SOURCE, Limits::default(), Assessment::FullBaseline);
    let reference_wire: serde_json::Value = serde_json::from_str(&reference.to_json()).unwrap();
    let total = reference_wire["diagnostics"]["retained"].as_u64().unwrap()
        + reference_wire["diagnostics"]["omitted"].as_u64().unwrap();
    for (bytes, count, pointers) in [
        (1, 1, 1),
        (1500, 1, 128),
        (5000, 3, 1),
        (20_000, 100, 100_000),
    ] {
        let result = budgeted_annotation(
            SOURCE,
            Limits::default()
                .diagnostic_bytes(bytes)
                .conditions(count)
                .pointer_output_bytes(pointers)
                .report_bytes(100_000),
            Assessment::FullBaseline,
        );
        let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
        assert_eq!(wire["outcome"], reference_wire["outcome"]);
        assert_eq!(
            wire["result"]["fidelity"],
            reference_wire["result"]["fidelity"]
        );
        assert_eq!(
            wire["result"]["checking"]["outcome"],
            reference_wire["result"]["checking"]["outcome"]
        );
        assert_eq!(
            wire["result"]["checking"]["coverage"]["requested"],
            reference_wire["result"]["checking"]["coverage"]["requested"]
        );
        assert_eq!(
            wire["result"]["checking"]["coverage"]["requirements"],
            reference_wire["result"]["checking"]["coverage"]["requirements"]
        );
        assert_eq!(wire["result"]["active_sources"], serde_json::json!([2]));
        assert_eq!(
            wire["result"]["substitution"],
            serde_json::json!({"original_source":1,"generated_source":2})
        );
        assert_eq!(wire["request"]["editing"]["edit_count"], 1);
        assert_eq!(wire["sources"].as_array().unwrap().len(), 2);
        assert!(result.to_json().len() <= 100_000);
        assert_eq!(
            wire["diagnostics"]["retained"].as_u64().unwrap()
                + wire["diagnostics"]["omitted"].as_u64().unwrap(),
            total
        );
        let items = wire["diagnostics"]["items"].as_array().unwrap();
        let detail_cost = items
            .iter()
            .map(|item| serde_json::to_vec(item).unwrap().len() + 1)
            .sum::<usize>()
            + wire["result"]["checking"]["coverage"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| serde_json::to_vec(row).unwrap().len() + 1)
                .sum::<usize>()
            + wire["result"]["checking"]["coverage"]["requirement_rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| serde_json::to_vec(row).unwrap().len() + 1)
                .sum::<usize>();
        assert!(detail_cost <= bytes, "{detail_cost} > {bytes}");
        assert!(items.len() <= count);
        let pointer_cost = items
            .iter()
            .flat_map(|item| {
                std::iter::once(&item["primary"]).chain(item["related"].as_array().unwrap())
            })
            .filter_map(|location| location["pointer"].as_str())
            .map(str::len)
            .sum::<usize>();
        assert!(pointer_cost <= pointers, "{pointer_cost} > {pointers}");
        assert!(result.to_human().contains("failed"));
        assert!(
            result
                .to_human()
                .contains(&format!("Errors: {}", result.outcome().errors()))
        );
        assert!(result.to_human().contains("established"));
    }
}

#[test]
fn late_numeric_exhaustion_keeps_definite_findings_and_shared_remainders() {
    let source = br#"{"info":{},"sdfObject":{"Sensor":{"description":"old","sdfProperty":{"reading":{"type":"string","nullable":false,"minLength":18446744073709551616,"maxLength":18446744073709551617}}}}}"#;
    let run = |limit| {
        budgeted_annotation(
            source,
            Limits::default().numeric_work(limit),
            Assessment::FullBaseline,
        )
    };
    let cap = (1..512)
        .find(|limit| run(*limit).outcome().complete())
        .expect("bounded full numeric completion");
    let partial = run(cap - 1);
    println!(
        "numeric boundary {cap}: {:?}",
        partial
            .diagnostics()
            .conditions()
            .map(|condition| condition.code().to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(partial.fidelity(), Fidelity::Established);
    assert_eq!(partial.generation(), Generation::Complete);
    assert!(partial.generated_document().is_some());
    assert!(!partial.document_deliverable());
    assert!(!partial.outcome().complete());
    assert!(partial.outcome().errors() > 0);
    assert_eq!(partial.outcome().disposition(), Disposition::Failed);
    assert!(
        partial
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "metadata.string")
    );
    assert!(
        partial
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "formal.unavailable")
    );
    let completed = run(cap);
    assert!(completed.outcome().complete());
    assert_eq!(
        completed
            .diagnostics()
            .conditions()
            .filter(|condition| condition.code() == "formal.uint")
            .count(),
        2
    );
    assert_eq!(
        partial
            .diagnostics()
            .conditions()
            .filter(|condition| condition.code() == "formal.uint")
            .count(),
        1
    );
    let fresh = Catalogue::with_limits(
        &[SourceInput::supplied(
            "candidate",
            partial.generated_document().unwrap(),
        )],
        Limits::default().numeric_work(cap - 1),
    )
    .unwrap();
    let independently_checked = fresh
        .check(esdieff::CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(
        independently_checked.outcome().complete(),
        "the same numeric limit completes after acquiring only candidate bytes, so editing did not reset original parser expenditure"
    );
    println!(
        "edit full numeric boundary {cap}, one-less retains definite errors and complete candidate"
    );
}

#[test]
fn late_checking_work_exhaustion_keeps_candidate_and_earlier_errors() {
    let run = |work| {
        budgeted_annotation(
            SOURCE,
            Limits::default().decoding_work(work),
            Assessment::FullBaseline,
        )
    };
    let mut upper = 4096;
    while !run(upper).outcome().complete() {
        upper *= 2;
        assert!(upper <= 131_072);
    }
    let mut lower = 1;
    while lower < upper {
        let mid = lower + (upper - lower) / 2;
        if run(mid).outcome().complete() {
            upper = mid;
        } else {
            lower = mid + 1;
        }
    }
    let cap = upper;
    let partial = run(cap - 1);
    assert_eq!(partial.fidelity(), Fidelity::Established);
    assert_eq!(partial.generation(), Generation::Complete);
    assert!(partial.generated_document().is_some());
    assert!(!partial.outcome().complete());
    assert!(!partial.document_deliverable());
    assert!(
        partial
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "metadata.string")
    );
    assert!(partial.outcome().errors() > 0);
    let fresh = Catalogue::with_limits(
        &[SourceInput::supplied(
            "candidate",
            partial.generated_document().unwrap(),
        )],
        Limits::default().decoding_work(cap - 1),
    )
    .unwrap();
    let independently_checked = fresh
        .check(esdieff::CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(
        independently_checked.outcome().complete(),
        "acquiring only candidate bytes avoids editing's spent transformation allowance"
    );
    println!(
        "edit full work boundary {cap}, one-less retains definite errors and complete candidate"
    );
}

#[test]
fn textual_data_findings_keep_exact_plan_original_and_generated_spans() {
    let source = br#"{"sdfProperty":{"reading":{"description":"Reading","type":"number","maximum":5,"multipleOf":1,"maxLength":1}}}"#;
    for (quality, value, assessment, code, original_value) in [
        ("maximum", "null", Assessment::Initial, "data.number", "5"),
        (
            "maximum",
            "\"too hot\"",
            Assessment::Initial,
            "data.number",
            "5",
        ),
        (
            "maxLength",
            "18446744073709551616",
            Assessment::FullBaseline,
            "formal.uint",
            "1",
        ),
    ] {
        let plan = format!(
            r#"{{"key":"range-correction","source":"base","generated_key":"revised","edits":[{{"owner":"/sdfProperty/reading","quality":"{quality}","operation":"replace","value":{value}}}]}}"#
        );
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", source)]).unwrap();
        let result = EditRequest::from_plan(catalogue, plan.as_bytes())
            .unwrap()
            .checking(assessment)
            .unwrap()
            .edit()
            .unwrap();
        assert_eq!(result.outcome().disposition(), Disposition::Failed);
        assert_eq!(result.fidelity(), Fidelity::Established);
        assert_eq!(result.generation(), Generation::Complete);
        assert!(!result.document_deliverable());
        let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|finding| finding["code"] == code)
            .unwrap();
        let generated = std::iter::once(&finding["primary"])
            .chain(finding["related"].as_array().unwrap())
            .find(|location| {
                location["kind"] == "source"
                    && location["source_id"] == result.generated_source().get()
            })
            .unwrap();
        assert_eq!(
            generated["pointer"],
            format!("/sdfProperty/reading/{quality}")
        );
        let span = generated["span"].as_array().unwrap();
        assert_eq!(
            &result.generated_document().unwrap()
                [span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
            value.as_bytes()
        );
        let original = finding["related"]
            .as_array()
            .unwrap()
            .iter()
            .find(|location| location["kind"] == "source" && location["source_id"] == 1)
            .unwrap();
        let span = original["span"].as_array().unwrap();
        assert_eq!(
            &source[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
            original_value.as_bytes()
        );
        let intent = finding["related"]
            .as_array()
            .unwrap()
            .iter()
            .find(|location| location["kind"] == "edit_request")
            .unwrap();
        assert_eq!(intent["key"], "range-correction");
        assert_eq!(intent["pointer"], "/edits/0/value");
        let span = intent["span"].as_array().unwrap();
        assert_eq!(
            &plan[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
            value
        );
        assert!(result.to_human().contains("/edits/0/value"));
    }
}

#[test]
fn checked_removal_exposes_absence_and_original_intent_without_a_generated_span() {
    let source = br#"{"sdfObject":{"Sensor":{"description":17}}}"#;
    let plan = br#"{"key":"remove-invalid","source":"base","generated_key":"revised","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"remove"}]}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", source)]).unwrap();
    let result = EditRequest::from_plan(catalogue, plan)
        .unwrap()
        .checking(Assessment::Initial)
        .unwrap()
        .edit()
        .unwrap();
    assert!(result.document_deliverable());
    let target = result.target(0).require().unwrap();
    assert!(matches!(target.generated_member(), esdieff::Field::Absent));
    let original = target.original_member();
    let span = original.span();
    assert_eq!(&source[span.start..span.end], b"17");
    let intent = target.request_location();
    assert_eq!(intent.pointer().require().unwrap(), "/edits/0");
    let span = intent.span().unwrap();
    assert_eq!(
        &plan[span.start as usize..span.end as usize],
        br#"{"owner":"/sdfObject/Sensor","quality":"description","operation":"remove"}"#
    );
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    for finding in wire["diagnostics"]["items"].as_array().unwrap() {
        for location in
            std::iter::once(&finding["primary"]).chain(finding["related"].as_array().unwrap())
        {
            assert_ne!(location["pointer"], "/sdfObject/Sensor/description");
        }
    }
}

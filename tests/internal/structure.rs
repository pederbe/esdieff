//! Check established structural rules through the public supplied-catalogue workflow.
//! Follows ADR0001: Base SDF as the semantic foundation, and
//! ADR0003: Check findings and completeness.
use super::coverage_support;
use super::location_support;
use esdieff::{BasisKind, Catalogue, CheckRequest, Class, InspectRequest, Lookup, SourceInput};
use serde_json::Value;

#[test]
fn given_names_are_case_sensitive_and_colon_findings_have_escaped_source_locations() {
    let text = br#"{"sdfData":{"Room /~ Count":{},"count":{},"Count":{},"bad:/~ name":{}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("names", text)]).unwrap();
    let source = catalogue.sources().next().unwrap();
    assert!(matches!(
        source.definition(Class::Data, "count"),
        Lookup::Found(_)
    ));
    assert!(matches!(
        source.definition(Class::Data, "Count"),
        Lookup::Found(_)
    ));
    assert!(matches!(
        source.definition(Class::Data, "COUNT"),
        Lookup::NotFound
    ));
    let view = catalogue
        .inspect(InspectRequest::new(source.id(), "/sdfData/Room ~1~0 Count"))
        .unwrap();
    assert!(view.outcome().complete());
    assert_eq!(
        view.inspection_json()
            .unwrap()
            .pointer(8192)
            .require()
            .unwrap(),
        "/sdfData/Room ~1~0 Count"
    );
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let diagnostics = report.diagnostics();
    let condition = diagnostics
        .conditions()
        .find(|c| c.code() == "name.colon")
        .unwrap();
    assert_eq!(condition.basis().kind(), BasisKind::Rfc);
    let location = location_support::primary_source(condition);
    assert_eq!(location.source_id(), source.id());
    assert_eq!(
        location.pointer().require().unwrap(),
        "/sdfData/bad:~1~0 name"
    );
    let span = location.span().unwrap();
    assert_eq!(&text[span.start as usize..span.end as usize], b"{}");
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    assert!(report.to_human().contains("Errors: 1"));
}

#[test]
fn names_follow_typed_base_containment_and_leave_opaque_data_untouched() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "device",
        br#"{
        "sdfThing":{"Room / device":{
            "sdfObject":{"Sensor":{
                "sdfProperty":{"temperature": {"type":"number"}},
                "sdfAction":{"reset":{
                    "sdfInputData":{"type":"object","properties":{"bad:input":{}}},
                    "sdfOutputData":{"type":"number"},
                    "sdfData":{"Action Data":{}}
                }},
                "sdfEvent":{"changed":{"sdfOutputData":{"sdfChoice":{"bad:choice":{}}}}},
                "sdfData":{"Object Data":{}},
                "vendor:opaque":{"sdfData":{"ignored:name":{}}}
            }}
        }},
        "sdfData":{"Values":{"type":"object","const":{"literal:key":1}}}
    }"#,
    )])
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    let thing = source
        .definitions(Class::Thing)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    let object = thing
        .children(Class::Object)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert_eq!(object.class(), Class::Object);
    let property = object
        .children(Class::Property)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert!(property.data_description().require().is_ok());
    let action = object
        .children(Class::Action)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert!(action.input().require().is_ok());
    assert!(action.output().require().is_ok());
    assert_eq!(
        action
            .children(Class::Data)
            .require()
            .unwrap()
            .next()
            .unwrap()
            .0,
        "Action Data"
    );
    let event = object
        .children(Class::Event)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert!(event.output().require().is_ok());
    assert!(matches!(event.input(), esdieff::Field::Absent));
    assert_eq!(
        object
            .children(Class::Data)
            .require()
            .unwrap()
            .next()
            .unwrap()
            .0,
        "Object Data"
    );
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 2);
    let diagnostics = report.diagnostics();
    let pointers: Vec<_> = diagnostics
        .conditions()
        .filter(|c| c.code() == "name.colon")
        .map(|c| {
            location_support::primary_source(c)
                .pointer()
                .require()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert!(
        pointers
            .iter()
            .any(|p| p.ends_with("/sdfInputData/properties/bad:input"))
    );
    assert!(
        pointers
            .iter()
            .any(|p| p.ends_with("/sdfOutputData/sdfChoice/bad:choice"))
    );
    assert!(
        !pointers
            .iter()
            .any(|p| p.contains("ignored:name") || p.contains("literal:key"))
    );
}

#[test]
fn document_blocks_and_default_namespace_report_independent_established_failures() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("blocks", br#"{"namespace":[]}"#),
        SourceInput::supplied(
            "unmapped",
            br#"{"namespace":{"home":"https://example.invalid/model"},"defaultNamespace":"Home"}"#,
        ),
        SourceInput::supplied("absent", br#"{"defaultNamespace":"home"}"#),
        SourceInput::supplied(
            "shape",
            br#"{"namespace":{"home":42},"defaultNamespace":null}"#,
        ),
    ])
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 5);
    let diagnostics = report.diagnostics();
    for (source_key, code, pointer) in [
        ("blocks", "structure.block_shape", "/namespace"),
        ("unmapped", "namespace.default", "/defaultNamespace"),
        ("absent", "namespace.default", "/defaultNamespace"),
        ("shape", "namespace.uri", "/namespace/home"),
        ("shape", "namespace.default", "/defaultNamespace"),
    ] {
        assert!(
            diagnostics
                .conditions()
                .any(|condition| condition.code() == code && {
                    let location = location_support::primary_source(condition);
                    location.source_id() == catalogue.source_id(source_key).unwrap()
                        && location.pointer().require().unwrap() == pointer
                }),
            "{source_key} {code} {pointer}"
        );
    }
}

#[test]
fn base_definition_group_shapes_fail_without_hiding_source_json() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "shapes",
        br#"{
        "sdfProperty":[], "sdfData":{"Value":1}, "sdfThing":{}, "vendor:top":42
    }"#,
    )])
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    assert!(matches!(
        source.definitions(Class::Property),
        esdieff::Field::Unavailable(_)
    ));
    assert!(matches!(
        source.definitions(Class::Data),
        esdieff::Field::Unavailable(_)
    ));
    assert_eq!(
        source
            .json()
            .require()
            .unwrap()
            .get("vendor:top")
            .require()
            .unwrap()
            .raw_json(),
        "42"
    );
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 2);
    let diagnostics = report.diagnostics();
    for code in ["structure.group_shape", "structure.definition_shape"] {
        assert!(
            diagnostics
                .conditions()
                .any(|condition| condition.code() == code)
        );
    }
    let empty = Catalogue::from_inputs(&[SourceInput::supplied("empty", b"{}")]).unwrap();
    assert_eq!(
        empty.check(CheckRequest::new()).unwrap().outcome().errors(),
        0
    );
}

#[test]
fn diagnostic_and_location_omissions_preserve_structural_errors_and_fixed_inventory() {
    use esdieff::{Disposition, Field, Limits};
    let text = br#"{"sdfData":{"long/name:bad":{},"other:bad":{}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("names", text)],
        Limits::default().pointer_bytes(1),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 2);
    assert_eq!(report.outcome().disposition(), Disposition::Failed);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["diagnostics"]["truncated"], true);
    assert!(
        wire["diagnostics"]["reasons"]
            .as_array()
            .unwrap()
            .contains(&Value::from("location_limit"))
    );
    let diagnostics = report.diagnostics();
    for condition in diagnostics
        .conditions()
        .filter(|condition| condition.code() == "name.colon")
    {
        let location = location_support::primary_source(condition);
        assert!(matches!(location.pointer(), Field::Unavailable(_)));
        assert!(location.span().is_some());
    }
    let omitted = Catalogue::with_limits(
        &[SourceInput::supplied("names", text)],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let report = omitted.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 2);
    assert_eq!(report.diagnostics().conditions().len(), 0);
    assert!(report.diagnostics().omitted() >= 2);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    coverage_support::assert_wire_counts(&wire);
}

#[test]
fn unavailable_member_identity_preserves_its_location_omission_cause() {
    use esdieff::Field;
    let text = br#"{"info":{"\uD800":1},"sdfData":{"bad:Name":{}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("identity", text)]).unwrap();
    let source = catalogue.sources().next().unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    coverage_support::assert_requested_counts(&report);
    assert!(!report.outcome().complete());
    assert_eq!(source.raw_bytes(), text);
    let unknown = source
        .json()
        .require()
        .unwrap()
        .get("info")
        .require()
        .unwrap()
        .children()
        .next()
        .unwrap();
    assert_eq!(unknown.raw_json(), "1");
    assert!(matches!(unknown.pointer(8192), Field::Unavailable(_)));
    let diagnostics = report.diagnostics();
    let condition = diagnostics
        .conditions()
        .find(|c| c.code() == "structure.feature_context_unavailable")
        .unwrap();
    let location = location_support::primary_source(condition);
    assert_eq!(location.source_id(), source.id());
    let span = unknown.span();
    assert_eq!(location.span().unwrap(), span.start as u32..span.end as u32);
    assert!(matches!(location.pointer(), Field::Unavailable(_)));
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let condition = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "structure.feature_context_unavailable")
        .unwrap();
    assert_eq!(condition["primary"]["omission"]["reason"], "not_available");
    assert_eq!(
        wire["diagnostics"]["reasons"],
        serde_json::json!(["not_available"])
    );
    assert_eq!(
        wire["diagnostics"]["omissions"][0]["reason"],
        "not_available"
    );
    assert!(report.to_human().contains("not_available"));
    assert!(!report.to_human().contains("location_limit"));
}

#[test]
fn rejected_long_locations_spend_shared_work_before_later_short_locations() {
    use esdieff::{Field, Limits};
    let short_only = Catalogue::with_limits(
        &[SourceInput::supplied(
            "short",
            br#"{"info":{},"sdfData":{"x:y":{}}}"#,
        )],
        Limits::default().pointer_bytes(30).pointer_output_bytes(64),
    )
    .unwrap();
    let short_report = short_only.check(CheckRequest::new()).unwrap();
    let diagnostics = short_report.diagnostics();
    let short_condition = diagnostics
        .conditions()
        .find(|c| c.code() == "name.colon")
        .unwrap();
    assert_eq!(
        location_support::primary_source(short_condition)
            .pointer()
            .require()
            .unwrap(),
        "/sdfData/x:y"
    );
    let long_name = "Ancestor".repeat(20);
    let entries: Vec<_> = (0..12)
        .map(|n| format!(r#""{long_name}{n}":{{"minItems":-1}}"#))
        .collect();
    let text = format!(
        r#"{{"info":{{}},"sdfThing":{{{}}},"sdfData":{{"x:y":{{}}}}}}"#,
        entries.join(",")
    );
    for (allowance, short_available) in [(64, false), (8192, true)] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("locations", text.as_bytes())],
            Limits::default()
                .pointer_bytes(30)
                .pointer_output_bytes(allowance),
        )
        .unwrap();
        let source = catalogue.sources().next().unwrap();
        let inspection = catalogue
            .inspect(InspectRequest::new(source.id(), ""))
            .unwrap();
        assert!(inspection.outcome().complete());
        assert_eq!(inspection.inspection_json().unwrap().raw_json(), text);
        let short = source
            .json()
            .require()
            .unwrap()
            .get("sdfData")
            .require()
            .unwrap()
            .get("x:y")
            .require()
            .unwrap();
        assert_eq!(short.pointer(30).require().unwrap(), "/sdfData/x:y");
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 13);
        coverage_support::assert_requested_counts(&report);
        let diagnostics = report.diagnostics();
        assert_eq!(diagnostics.conditions().len(), 13);
        assert_eq!(diagnostics.omitted(), 0);
        for count in diagnostics
            .conditions()
            .filter(|c| c.code() == "grouping.count")
        {
            let location = location_support::primary_source(count);
            assert!(matches!(location.pointer(), Field::Unavailable(_)));
            let span = location.span().unwrap();
            assert_eq!(
                &text.as_bytes()[span.start as usize..span.end as usize],
                b"-1"
            );
        }
        let name = diagnostics
            .conditions()
            .find(|c| c.code() == "name.colon")
            .unwrap();
        let location = location_support::primary_source(name);
        assert_eq!(location.source_id(), source.id());
        let span = short.span();
        assert_eq!(location.span().unwrap(), span.start as u32..span.end as u32);
        assert_eq!(
            location.pointer().present(),
            short_available.then_some("/sdfData/x:y")
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["diagnostics"]["reasons"],
            serde_json::json!(["location_limit"])
        );
        assert!(report.to_human().contains("location_limit"));
    }
}

#[test]
fn namespaces_use_rfc3986_syntax_and_identity_without_retrieval_or_scheme_policy() {
    for uri in [
        "https://example.invalid/model",
        "urn:example:device",
        "file:///model",
        "x:",
        "x://",
        "x://user:password@host:/model",
        "HTTPS://user:pass@[2001:db8::1]:443/model?rev=1#part",
        "x://[v1.example:future]/model",
        "x://[Vf.a:b]/model",
        "x://999.999.999.999/model",
        "x://a!b$%2f_/model",
        "https://example.invalid/model?other=/path?query#fragment?/path",
        "urn:example:%ff",
        "https://example.invalid/%E2%82%AC",
    ] {
        let text = format!(r#"{{"namespace":{{"home":"{uri}"}},"defaultNamespace":"home"}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("uri", text.as_bytes())]).unwrap();
        assert_eq!(
            catalogue
                .check(CheckRequest::new())
                .unwrap()
                .outcome()
                .errors(),
            0,
            "{uri}"
        );
    }
    for uri in [
        "relative/path",
        "//example.invalid/path",
        "1x:model",
        "https://bad host/",
        "https://example.invalid/%",
        "https://example.invalid/%0G",
        "https://example.invalid/{wrong}",
        "https://[2001:db8:::1]/",
        "https://[v1.]/",
        "https://[v.abc]/",
        "https://[::1]tail/",
        "https://example.invalid:port/",
        "https://a@b@host/",
        "https://[fe80::1%25zone]/",
        "https://example.invalid/model#one#two",
        "https://example.invalid/€",
        "https://[::ffff:192.168.001.1]/",
        "x://[v1.%20]/",
    ] {
        let text = format!(r#"{{"namespace":{{"home":"{uri}"}},"defaultNamespace":"home"}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("uri", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 1, "{uri}");
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "namespace.uri"),
            "{uri}"
        );
    }
    let shared = Catalogue::from_inputs(&[
        SourceInput::supplied("first", br#"{"namespace":{"a":"https://unavailable.invalid/shared"},"defaultNamespace":"a","sdfData":{"First":{}}}"#),
        SourceInput::supplied("second", br#"{"namespace":{"b":"https://unavailable.invalid/shared"},"defaultNamespace":"b","sdfData":{"Second":{}}}"#),
    ]).unwrap();
    assert_eq!(shared.sources().len(), 2);
    assert_eq!(
        shared
            .check(CheckRequest::new())
            .unwrap()
            .outcome()
            .errors(),
        0
    );
}

#[test]
fn grouping_repetition_is_typed_separately_from_array_data_and_extensions() {
    use esdieff::Field;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "roles",
        br#"{
        "sdfThing":{"Repeated":{"minItems":1.00e3,"maxItems":1e1000000}},
        "sdfObject":{"Extension":{"minItems":"1","sdfObject":{"opaque:name":{}}}},
        "sdfData":{"Array":{"type":"array","minItems":4}}
    }"#,
    )])
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    let thing = source
        .definitions(Class::Thing)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert_eq!(
        thing.repetition_minimum().require().unwrap().lexeme(),
        "1.00e3"
    );
    assert_eq!(
        thing.repetition_maximum().require().unwrap().lexeme(),
        "1e1000000"
    );
    assert!(matches!(thing.data_description(), Field::Absent));
    let object = source
        .definitions(Class::Object)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert!(matches!(object.repetition_minimum(), Field::Unavailable(_)));
    assert!(matches!(object.repetition_maximum(), Field::Absent));
    assert!(matches!(
        object.children(Class::Object),
        Field::Unavailable(_)
    ));
    assert!(matches!(object.children(Class::Thing), Field::Absent));
    assert_eq!(
        object.json().get("sdfObject").require().unwrap().raw_json(),
        r#"{"opaque:name":{}}"#
    );
    let data = source
        .definitions(Class::Data)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert!(matches!(data.repetition_minimum(), Field::Absent));
    assert_eq!(
        data.data_description()
            .require()
            .unwrap()
            .number("minItems")
            .require()
            .unwrap()
            .lexeme(),
        "4"
    );
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let diagnostics = report.diagnostics();
    assert!(
        diagnostics
            .conditions()
            .any(|c| c.code() == "grouping.count")
    );
    assert!(
        diagnostics
            .conditions()
            .any(|c| c.code() == "structure.role_unavailable")
    );
    assert!(!diagnostics.conditions().any(|c| c.code() == "name.colon"));
}

#[test]
fn patch_deletions_are_distinct_from_surviving_names_and_definite_overrides() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "patch",
            br##"{
            "sdfThing":{"Consumer":{
                "sdfRef":"#/sdfThing/Source", "minItems":null,
                "sdfObject":null, "sdfProperty":{"deleted:Name":null},
                "sdfAction":{"reset":{"sdfInputData":null,"sdfOutputData":null}},
                "sdfEvent":{"changed":{"sdfOutputData":null}},
                "sdfData":{"Data":{"properties":{"deleted:entry":null}}}
            }}
        }"##,
        ),
        SourceInput::supplied(
            "overrides",
            br##"{
            "sdfThing":{"bad:Consumer":{
                "sdfRef":"#/sdfThing/Source", "minItems":-1,
                "sdfProperty":{"surviving:Name":{}}, "sdfObject":[]
            }}
        }"##,
        ),
        SourceInput::supplied(
            "target",
            br#"{"sdfThing":{"Source":{"sdfProperty":{"bad:Target":{}}}}}"#,
        ),
    ])
    .unwrap();
    let patch = catalogue
        .check(CheckRequest::new().root(catalogue.source_id("patch").unwrap()))
        .unwrap();
    assert_eq!(patch.outcome().errors(), 0);
    assert!(!patch.outcome().complete());
    assert_eq!(
        patch.requested_source_ids(),
        &[catalogue.source_id("patch").unwrap()]
    );
    let overrides = catalogue
        .check(CheckRequest::new().root(catalogue.source_id("overrides").unwrap()))
        .unwrap();
    assert_eq!(overrides.outcome().errors(), 4);
    let diagnostics = overrides.diagnostics();
    assert_eq!(
        diagnostics
            .conditions()
            .filter(|c| c.code() == "name.colon")
            .count(),
        2
    );
    assert!(
        diagnostics
            .conditions()
            .any(|c| c.code() == "grouping.count")
    );
    assert!(
        diagnostics
            .conditions()
            .any(|c| c.code() == "structure.group_shape")
    );
    assert_eq!(
        catalogue
            .check(CheckRequest::new())
            .unwrap()
            .outcome()
            .errors(),
        5
    );
}

#[test]
fn grouping_count_shapes_preserve_exact_capability_and_do_not_prove_empty_groupings() {
    use esdieff::{Field, Limits};
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "counts",
        br#"{
        "info":{},
        "sdfThing":{
            "Fraction":{"minItems":0.5}, "Negative":{"maxItems":-1},
            "Text":{"minItems":"1"}, "Null":{"maxItems":null},
            "Inverted":{"minItems":10,"maxItems":1}, "Zero":{"minItems":-0.0}
        },
        "sdfData":{"Array":{"type":"array","minItems":-1}}
    }"#,
    )])
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 5);
    assert_eq!(report.outcome().warnings(), 0);
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|c| c.code() == "grouping.count")
            .count(),
        4
    );
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|c| c.code() == "data.count")
            .count(),
        1
    );
    let limited = Catalogue::with_limits(
        &[SourceInput::supplied(
            "exact",
            br#"{"sdfObject":{"Many":{"minItems":123}}}"#,
        )],
        Limits::default().coefficient_digits(1),
    )
    .unwrap();
    let definition = limited
        .sources()
        .next()
        .unwrap()
        .definitions(Class::Object)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert!(matches!(
        definition.repetition_minimum(),
        Field::Unavailable(_)
    ));
    assert_eq!(
        definition
            .json()
            .get("minItems")
            .require()
            .unwrap()
            .raw_json(),
        "123"
    );
    let report = limited.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert!(!report.outcome().complete());
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "grouping.number_unavailable")
    );
}

#[test]
fn bounded_structure_work_retains_independent_findings_and_complete_source_inspection() {
    use esdieff::Limits;
    let uri = format!("https://example.invalid/{}", "a".repeat(1000));
    let text = format!(r#"{{"sdfData":{{"bad:Name":{{}}}},"namespace":{{"home":"{uri}"}}}}"#);
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("bounded", text.as_bytes())],
        Limits::default().decoding_work(5000),
    )
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    let inspection = catalogue
        .inspect(InspectRequest::new(source.id(), ""))
        .unwrap();
    assert!(inspection.outcome().complete());
    assert_eq!(inspection.inspection_json().unwrap().raw_json(), text);
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(!report.outcome().complete());
    coverage_support::assert_requested_counts(&report);
    let diagnostics = report.diagnostics();
    assert!(diagnostics.conditions().any(|c| c.code() == "name.colon"));
    assert!(
        diagnostics
            .conditions()
            .any(|c| c.code() == "operation.limit" && {
                location_support::primary_source(c)
                    .pointer()
                    .require()
                    .unwrap()
                    == "/namespace/home"
            })
    );
    assert!(
        !diagnostics
            .conditions()
            .any(|c| c.code() == "namespace.uri")
    );
    for report in [
        report,
        catalogue
            .check(CheckRequest::new().assessment(esdieff::Assessment::ExpandedModified))
            .unwrap(),
    ] {
        let diagnostics = report.diagnostics();
        let limit = diagnostics
            .conditions()
            .find(|condition| condition.code() == "operation.limit")
            .unwrap();
        assert_eq!(limit.assessment_unit(), None);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let limit = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|condition| condition["code"] == "operation.limit")
            .unwrap();
        assert!(limit.get("assessment_unit").is_none());
    }
}

#[test]
fn critical_feature_context_precedes_semantic_findings_and_preserves_companion_work() {
    for info in [
        r#"{"features":["future"]}"#,
        r#"{"features":[{"future":true}]}"#,
        r#"{"features":null}"#,
        r#"{"features":true}"#,
        r#"{"features":"future"}"#,
        r#"{"features":{}}"#,
        r#"{"features":[],"\uD800":1}"#,
        "[]",
        r#""\uD800""#,
    ] {
        for info_first in [true, false] {
            let body = r#""sdfThing":{"bad:Name":{"minItems":-1}},"namespace":{"home":"relative/path"},"defaultNamespace":"missing""#;
            let text = if info_first {
                format!("{{\"info\":{info},{body}}}")
            } else {
                format!("{{{body},\"info\":{info}}}")
            };
            let catalogue = Catalogue::from_inputs(&[
                SourceInput::supplied("extension", text.as_bytes()),
                SourceInput::supplied("ordinary", br#"{"sdfData":{"bad:Companion":{}}}"#),
            ])
            .unwrap();
            let selected = catalogue
                .check(CheckRequest::new().root(catalogue.source_id("extension").unwrap()))
                .unwrap();
            assert_eq!(selected.outcome().errors(), 0, "{info}");
            assert!(!selected.outcome().complete());
            assert!(
                selected
                    .diagnostics()
                    .conditions()
                    .any(|c| c.code() == "structure.feature_context_unavailable"),
                "{info}"
            );
            assert_eq!(
                catalogue
                    .check(CheckRequest::new())
                    .unwrap()
                    .outcome()
                    .errors(),
                1,
                "{info}"
            );
            assert_eq!(
                catalogue
                    .source(catalogue.source_id("extension").unwrap())
                    .unwrap()
                    .raw_bytes(),
                text.as_bytes()
            );
        }
    }
    for text in [
        br#"{"sdfData":{"bad:Name":{}},"\uD800":1}"#.as_slice(),
        br#"{"info":{},"sdfData":{"bad:Name":{}},"\uD800":1}"#.as_slice(),
    ] {
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("identity", text)]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "structure.feature_context_unavailable")
        );
    }
    for info in ["", r#""info":{},"#, r#""info":{"features":[]},"#] {
        let text = format!(r#"{{{info}"sdfData":{{"bad:Name":{{}}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("ordinary", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "structure.feature_context_unavailable")
        );
    }
    let independent = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "duplicate",
            br#"{"info":{"features":["future"]},"x":1,"x":2}"#,
        ),
        SourceInput::supplied("envelope", b"[]"),
    ])
    .unwrap();
    let report = independent.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 2);
    let diagnostics = report.diagnostics();
    assert!(
        diagnostics
            .conditions()
            .any(|c| c.code() == "input.duplicate_name")
    );
    assert!(
        diagnostics
            .conditions()
            .any(|c| c.code() == "document.envelope")
    );
}

#[test]
fn feature_preflight_work_limit_cannot_leak_later_dependent_findings() {
    use esdieff::Limits;
    let uri = format!("https://example.invalid/{}", "a".repeat(1000));
    let first = format!(r#"{{"namespace":{{"a":"{uri}","b":"{uri}","c":"{uri}","d":"{uri}"}}}}"#);
    let last = format!(
        r#"{{"sdfData":{{"bad:Name":{{}}}},"info":{{"{}":0}}}}"#,
        "a".repeat(3000)
    );
    let inputs = [
        SourceInput::supplied("first", first.as_bytes()),
        SourceInput::supplied("last", last.as_bytes()),
    ];
    let catalogue =
        Catalogue::with_limits(&inputs, Limits::default().decoding_work(34000)).unwrap();
    for source in catalogue.sources() {
        let inspection = catalogue
            .inspect(InspectRequest::new(source.id(), ""))
            .unwrap();
        assert!(inspection.outcome().complete(), "{}", inspection.to_human());
    }
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "operation.limit"
                && location_support::primary_source(c).source_id()
                    == catalogue.source_id("last").unwrap())
    );
    let ordinary = Catalogue::from_inputs(&inputs).unwrap();
    assert_eq!(
        ordinary
            .check(CheckRequest::new())
            .unwrap()
            .outcome()
            .errors(),
        1
    );
}

//! Local numeric advice follows ADR 0030: Bounded numeric advice.
use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Field, Limits, SourceInput};
use serde_json::Value;

const AREA: &str = "advisory-numeric-three/1";

fn numeric(bytes: &[u8]) -> Value {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    let wire = serde_json::from_str(&report.to_json()).unwrap();
    assert_numeric_inventory(&report, &wire);
    wire
}

fn conditions<'a>(wire: &'a Value, code: &str) -> Vec<&'a Value> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["code"] == code)
        .collect()
}

fn numeric_area_row(wire: &Value) -> &Value {
    wire["coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == AREA)
        .unwrap()
}

#[test]
fn const_hidden_by_outer_uncertainty_is_not_proven_absent() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"const":0.5,"minimum":1}}}"#;
    let inputs = [SourceInput::supplied("model", bytes)];
    for limits in [
        Limits::default().document_nodes(2),
        Limits::default().parsed_nodes(2),
        Limits::default().parsed_allocation_bytes(1),
        Limits::default().decoding_work(1),
        Limits::default().effective_nodes(1),
        Limits::default().effective_allocation_bytes(1),
    ] {
        let catalogue = Catalogue::with_limits(&inputs, limits).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
            .unwrap();
        assert_eq!(
            report.assessment_area_status(AREA).present(),
            Some(AssessmentStatus::Undetermined)
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(conditions(&wire, "proof.numeric_const_bound").is_empty());
    }
    for bytes in [
        &br##"{"info":{},"sdfData":{"value":{"sdfRef":"#/sdfData/missing","const":0.5,"minimum":1}}}"##[..],
        &br#"{"info":{},"sdfData":{"value":{"sdfRef":"missing:#/sdfData/value","const":0.5,"minimum":1}}}"#[..],
    ] {
        let wire = numeric(bytes);
        assert!(conditions(&wire, "proof.numeric_const_bound").is_empty());
        assert_eq!(numeric_area_row(&wire)["status"], "undetermined");
    }
    let ordinary = numeric(br#"{"info":{},"sdfData":{"value":{"type":"number","unit":"m"}}}"#);
    assert_eq!(numeric_area_row(&ordinary)["status"], "not_applicable");
    assert!(conditions(&ordinary, "proof.numeric_const_bound").is_empty());
    let first = SourceInput::supplied("first", bytes);
    let later = SourceInput::supplied("later", br##"{"info":{},"sdfData":{"value":{"sdfRef":"#/sdfData/missing","const":0.5,"minimum":1}}}"##);
    let catalogue = Catalogue::from_inputs(&[first, later]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        report.assessment_area_status(AREA).present(),
        Some(AssessmentStatus::Undetermined)
    );
    assert_eq!(conditions(&wire, "proof.numeric_const_bound").len(), 1);
}

#[test]
fn absent_type_const_bound_retains_absence_and_original_operands() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"const":0.5,"minimum":1}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let leaf = catalogue
        .sources()
        .next()
        .unwrap()
        .json()
        .require()
        .unwrap()
        .get("sdfData")
        .require()
        .unwrap()
        .get("value")
        .require()
        .unwrap();
    assert!(matches!(leaf.get("type"), Field::Absent));
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    assert_eq!(
        report.assessment_area_status(AREA).present(),
        Some(AssessmentStatus::Assessed)
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let rows = conditions(&wire, "proof.numeric_const_bound");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["arguments"]["const"], "0.5");
    assert_eq!(rows[0]["arguments"]["minimum"], "1");
    assert!(rows[0]["arguments"].get("type").is_none());
    assert_eq!(rows[0]["assessment_unit"], AREA);
    assert_eq!(rows[0]["basis"]["kind"], "advice");
    assert_eq!(rows[0]["severity"], "warning");
    for location in rows[0]["related"]
        .as_array()
        .unwrap()
        .iter()
        .chain(std::iter::once(&rows[0]["primary"]))
    {
        assert!(
            !location["pointer"]
                .as_str()
                .is_some_and(|p| p.ends_with("/type"))
        );
    }
    assert_eq!(catalogue.sources().next().unwrap().raw_bytes(), bytes);
}

#[test]
fn integer_interval_gap_is_explicit_advice_with_signed_endpoint_rounding() {
    for (low, high, gap) in [
        ("0.1", "0.9", true),
        ("-0.9", "-0.1", true),
        ("-1.1", "-0.9", false),
        ("0", "1", false),
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"integer","nullable":false,"minimum":{low},"maximum":{high}}}}}}}"#
        );
        let wire = numeric(bytes.as_bytes());
        assert_eq!(
            conditions(&wire, "proof.integer_interval_gap").len(),
            usize::from(gap),
            "{low}..{high}"
        );
    }
}

#[test]
fn const_checks_all_four_bounds_and_retains_equal_endpoint_exclusivity() {
    let wire = numeric(br#"{"info":{},"sdfData":{"value":{"const":0.5,"minimum":1,"exclusiveMinimum":1,"maximum":0,"exclusiveMaximum":0}}}"#);
    let rows = conditions(&wire, "proof.numeric_const_bound");
    assert_eq!(rows.len(), 4);
    for bound in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        assert_eq!(
            rows.iter()
                .filter(|row| row["arguments"].get(bound).is_some())
                .count(),
            1
        );
    }
    for (key, expected) in [
        ("minimum", 0),
        ("maximum", 0),
        ("exclusiveMinimum", 1),
        ("exclusiveMaximum", 1),
    ] {
        let bytes = format!(r#"{{"info":{{}},"sdfData":{{"value":{{"const":1.0,"{key}":1e0}}}}}}"#);
        assert_eq!(
            conditions(&numeric(bytes.as_bytes()), "proof.numeric_const_bound").len(),
            expected,
            "{key}"
        );
    }
}

#[test]
fn const_meaning_and_role_guards_are_distinct_from_payload_capability() {
    for format in ["uri", "uri-reference", "uuid", "date", "time", "date-time"] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"const":0.5,"minimum":1,"format":"{format}"}}}}}}"#
        );
        assert_eq!(
            conditions(&numeric(bytes.as_bytes()), "proof.numeric_const_bound").len(),
            1
        );
    }
    for extra in [
        r#""format":"extended""#,
        r#""sdfType":"unix-time""#,
        r#""format":false"#,
        r#""items":{}"#,
        r#""properties":{}"#,
        r#""readable":true"#,
        r#""sdfRequired":[]"#,
        r#""title":"t""#,
        r#""sdfRequiredInputData":[]"#,
        r#""type":false"#,
    ] {
        let bytes =
            format!(r#"{{"info":{{}},"sdfData":{{"value":{{"const":0.5,"minimum":1,{extra}}}}}}}"#);
        let wire = numeric(bytes.as_bytes());
        assert!(
            conditions(&wire, "proof.numeric_const_bound").is_empty(),
            "{extra}"
        );
        let area = numeric_area_row(&wire);
        assert_eq!(area["status"], "undetermined", "{extra}");
    }
    // Undeclared extension vocabulary leaves the const meaning intact, following
    // ADR 0037: Undeclared extension vocabulary.
    for extra in [r#""unknown":true"#, r#""ext:meaning":true"#] {
        let bytes =
            format!(r#"{{"info":{{}},"sdfData":{{"value":{{"const":0.5,"minimum":1,{extra}}}}}}}"#);
        let wire = numeric(bytes.as_bytes());
        assert_eq!(
            conditions(&wire, "proof.numeric_const_bound").len(),
            1,
            "{extra}"
        );
        assert_eq!(numeric_area_row(&wire)["status"], "assessed", "{extra}");
    }
    for annotation in [
        "description",
        "label",
        "$comment",
        "default",
        "unit",
        "contentFormat",
        "required",
        "minItems",
        "maxItems",
        "uniqueItems",
        "minLength",
        "maxLength",
        "pattern",
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"const":0.5,"minimum":1,"{annotation}":{{"items":"literal"}}}}}}}}"#
        );
        assert_eq!(
            conditions(&numeric(bytes.as_bytes()), "proof.numeric_const_bound").len(),
            1,
            "{annotation}"
        );
    }
}

#[test]
fn numeric_inventory_and_exact_e0_omissions_preserve_the_same_outcome() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"const":0.5,"minimum":1}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    let reference: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_numeric_inventory(&report, &reference);
    let cap = 6 * "model".len() + 4096 + 16384;
    assert!(
        Catalogue::with_limits(
            &[SourceInput::supplied("model", bytes)],
            Limits::default().report_bytes(cap - 1)
        )
        .is_err()
    );
    let limited = Catalogue::with_limits(
        &[SourceInput::supplied("model", bytes)],
        Limits::default().report_bytes(cap),
    )
    .unwrap();
    let report = limited
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    for identity in [
        &wire["request"]["assessment"],
        &wire["request"]["inventory"],
        &wire["coverage"]["requested"]["inventory"],
    ] {
        assert_eq!(identity, "esdieff.expanded.numeric/1");
    }
    assert_eq!(wire["outcome"], reference["outcome"]);
    assert_eq!(report.requested_coverage().total(), Some(9));
    assert_eq!(wire["coverage"]["rows_omitted"], 9);
    assert!(wire["coverage"]["rows"].as_array().unwrap().is_empty());
    assert_eq!(wire["diagnostics"]["retained"], 0);
    assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
    assert_eq!(
        wire["diagnostics"]["omitted"],
        reference["diagnostics"]["omitted"].as_u64().unwrap()
            + reference["diagnostics"]["retained"].as_u64().unwrap()
    );
    assert_eq!(wire["diagnostics"]["truncated"], true);
    let omissions = wire["diagnostics"]["omissions"].as_array().unwrap();
    let row_omission = omissions
        .iter()
        .find(|o| o["field"] == "coverage.rows")
        .unwrap();
    assert_eq!(row_omission["omitted_count"], 9);
    assert_eq!(row_omission["reason"], "byte_limit");
    assert_eq!(wire["coverage"]["omissions"][0], *row_omission);
    assert!(report.to_human().contains("warnings: 1"));
}

fn assert_numeric_inventory(report: &esdieff::Report<'_>, wire: &Value) {
    let expected: std::collections::BTreeSet<_> = [
        "document-envelope/1",
        "base-roles-names/1",
        "metadata-namespaces/1",
        "established-composition/1",
        "basic-parameters/1",
        "alternatives-requiredness/1",
        "extensions-policies/1",
        "advisory-four/1",
        AREA,
    ]
    .into_iter()
    .collect();
    for identity in [
        &wire["request"]["assessment"],
        &wire["request"]["inventory"],
        &wire["coverage"]["requested"]["inventory"],
    ] {
        assert_eq!(identity, "esdieff.expanded.numeric/1");
    }
    assert_eq!(
        report.requested_coverage().inventory(),
        Some("esdieff.expanded.numeric/1")
    );
    let rows = wire["coverage"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 9);
    assert_eq!(
        rows.iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    let mut counts = [0u32; 4];
    for row in rows {
        let status = report
            .assessment_area_status(row["id"].as_str().unwrap())
            .present()
            .unwrap();
        assert_eq!(row["status"], status.as_str());
        assert_eq!(row["count_unit"], "assessment_area");
        counts[match status {
            AssessmentStatus::Assessed => 0,
            AssessmentStatus::NotApplicable => 1,
            AssessmentStatus::Unassessed => 2,
            AssessmentStatus::Undetermined => 3,
            _ => panic!("unexpected assessment status"),
        }] += 1;
        if row["id"] == AREA {
            assert_eq!(
                row["basis"]["kind"],
                if status == AssessmentStatus::Undetermined {
                    "operational"
                } else {
                    "advice"
                }
            );
            assert_eq!(
                row["basis"]["id"],
                if status == AssessmentStatus::Undetermined {
                    "expanded-numeric-establishment"
                } else {
                    AREA
                }
            );
        }
        if row["basis"]["kind"] == "operational" {
            assert!(row["basis"].get("citation").is_none());
        }
    }
    let coverage = report.requested_coverage();
    assert_eq!(
        [
            coverage.assessed(),
            coverage.not_applicable(),
            coverage.unassessed(),
            coverage.undetermined()
        ],
        counts
    );
    assert_eq!(counts.into_iter().sum::<u32>(), 9);
}

#[test]
fn interval_uses_all_bounds_exclusivity_null_and_independent_const_kind() {
    for (minimum, maximum, gap) in [
        ("minimum", "maximum", false),
        ("exclusiveMinimum", "maximum", false),
        ("minimum", "exclusiveMaximum", false),
        ("exclusiveMinimum", "exclusiveMaximum", true),
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"integer","nullable":false,"{minimum}":0,"{maximum}":1}}}}}}"#
        );
        assert_eq!(
            conditions(&numeric(bytes.as_bytes()), "proof.integer_interval_gap").len(),
            usize::from(gap)
        );
    }
    for bounds in [
        r#""minimum":0,"exclusiveMinimum":0.1,"maximum":1,"exclusiveMaximum":0.9"#,
        r#""minimum":0.1,"exclusiveMinimum":0,"maximum":0.9,"exclusiveMaximum":1"#,
        r#""minimum":1,"exclusiveMinimum":1,"maximum":1"#,
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"integer","nullable":false,{bounds}}}}}}}"#
        );
        assert_eq!(
            conditions(&numeric(bytes.as_bytes()), "proof.integer_interval_gap").len(),
            1
        );
    }
    for (extra, warning) in [
        ("", false),
        (r#", "nullable":true"#, false),
        (r#", "default":0.5"#, false),
        (r#", "const":null"#, false),
        (r#", "const":false"#, true),
        (r#", "const":"literal""#, true),
        (r#", "const":{"format":"literal","items":null}"#, true),
        (r#", "const":[], "nullable":false"#, true),
        (r#", "const":0.5,"nullable":"invalid""#, true),
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"integer","minimum":0.1,"maximum":0.9{extra}}}}}}}"#
        );
        assert_eq!(
            conditions(&numeric(bytes.as_bytes()), "proof.integer_interval_gap").len(),
            usize::from(warning),
            "{extra}"
        );
    }
    let bytes = br#"{"info":{},"sdfData":{"value":{"type":"integer","nullable":true,"const":12,"minimum":0.1,"maximum":0.9}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", bytes)],
        Limits::default().coefficient_digits(1),
    )
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(conditions(&wire, "proof.integer_interval_gap").len(), 1);
    assert!(conditions(&wire, "proof.numeric_const_bound").is_empty());
    assert_eq!(
        report.assessment_area_status(AREA).present(),
        Some(AssessmentStatus::Undetermined)
    );
    assert_numeric_inventory(&report, &wire);
}

#[test]
fn unsupported_interval_meaning_keeps_independent_const_and_sibling_warnings() {
    for format in ["uri", "uri-reference", "uuid", "date", "time", "date-time"] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"integer","const":0,"minimum":0.1,"maximum":0.9,"format":"{format}"}}}}}}"#
        );
        let wire = numeric(bytes.as_bytes());
        assert_eq!(conditions(&wire, "proof.numeric_const_bound").len(), 1);
        assert!(conditions(&wire, "proof.integer_interval_gap").is_empty());
        assert_eq!(
            conditions(&wire, "proof.numeric_capability_unavailable").len(),
            1
        );
    }
    let bytes = br#"{"info":{},"sdfData":{"known":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9},"unknown":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9,"readable":true}}}"#;
    let wire = numeric(bytes);
    assert_eq!(conditions(&wire, "proof.integer_interval_gap").len(), 1);
    assert_eq!(numeric_area_row(&wire)["status"], "undetermined");
    for condition in conditions(&wire, "proof.numeric_capability_unavailable") {
        assert_eq!(condition["basis"]["kind"], "operational");
        assert_eq!(condition["basis"]["id"], "expanded-numeric-capability");
        assert!(condition["basis"].get("citation").is_none());
    }
    // A sibling carrying undeclared extension vocabulary has its own gap, following
    // ADR 0037: Undeclared extension vocabulary.
    let bytes = br#"{"info":{},"sdfData":{"known":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9},"extended":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9,"extension:denotation":true}}}"#;
    let wire = numeric(bytes);
    assert_eq!(conditions(&wire, "proof.integer_interval_gap").len(), 2);
    assert_eq!(numeric_area_row(&wire)["status"], "assessed");
}

#[test]
fn finite_carry_and_virtual_width_are_separate_from_source_number_limits() {
    let nines = "9".repeat(512);
    for (upper, gap) in [("maximum", false), ("exclusiveMaximum", true)] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"integer","nullable":false,"exclusiveMinimum":{nines},"{upper}":1e512}}}}}}"#
        );
        let wire = numeric(bytes.as_bytes());
        assert_eq!(
            conditions(&wire, "proof.integer_interval_gap").len(),
            usize::from(gap)
        );
    }
    let bytes = format!(
        r#"{{"info":{{}},"sdfData":{{"value":{{"type":"integer","nullable":false,"minimum":-1e512,"exclusiveMaximum":-{nines}}}}}}}"#
    );
    assert!(conditions(&numeric(bytes.as_bytes()), "proof.integer_interval_gap").is_empty());
    for (power, supported) in [(1023, true), (1024, false)] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"integer","nullable":false,"minimum":1e{power},"maximum":1e{power}}}}}}}"#
        );
        let wire = numeric(bytes.as_bytes());
        assert!(conditions(&wire, "proof.integer_interval_gap").is_empty());
        assert_eq!(
            conditions(&wire, "proof.numeric_capability_unavailable").len(),
            usize::from(!supported)
        );
        assert_eq!(
            numeric_area_row(&wire)["status"],
            if supported {
                "assessed"
            } else {
                "undetermined"
            }
        );
    }
    for key in ["minimum", "exclusiveMinimum"] {
        let bytes = format!(r#"{{"info":{{}},"sdfData":{{"value":{{"const":-0,"{key}":0}}}}}}"#);
        let wire = numeric(bytes.as_bytes());
        let warnings = conditions(&wire, "proof.numeric_const_bound");
        assert_eq!(warnings.len(), usize::from(key == "exclusiveMinimum"));
        if let Some(warning) = warnings.first() {
            assert_eq!(warning["arguments"]["const"], "-0");
        }
    }
}

#[test]
fn positive_step_lattice_and_integer_intersection_are_exact() {
    for (kind, step, gap) in [
        ("integer", "1.5", true),
        ("number", "1.5", false),
        ("integer", "0.6", true),
        ("integer", "0.2", false),
        ("integer", "0.125", false),
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"{kind}","nullable":false,"minimum":1,"maximum":2,"multipleOf":{step}}}}}}}"#
        );
        assert_eq!(
            conditions(&numeric(bytes.as_bytes()), "proof.step_interval_gap").len(),
            usize::from(gap),
            "{kind}/{step}"
        );
    }
    for (low_key, low, high_key, high, gap) in [
        ("minimum", "0.1", "maximum", "0.2", true),
        ("minimum", "0.25", "maximum", "0.25", false),
        ("exclusiveMinimum", "0.25", "exclusiveMaximum", "0.5", true),
        ("minimum", "0.25", "exclusiveMaximum", "0.5", false),
        ("exclusiveMinimum", "0.25", "maximum", "0.5", false),
        ("minimum", "-0.2", "maximum", "-0.1", true),
        ("minimum", "-0.5", "maximum", "-0.25", false),
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"number","nullable":false,"{low_key}":{low},"{high_key}":{high},"multipleOf":0.25}}}}}}"#
        );
        assert_eq!(
            conditions(&numeric(bytes.as_bytes()), "proof.step_interval_gap").len(),
            usize::from(gap),
            "{low_key}{low}..{high_key}{high}"
        );
    }
    for (low, high, step, gap, supported) in [
        ("1e1000000", "2e1000000", "3e1000000", true, true),
        ("1e-1000000", "2e-1000000", "1", true, true),
        ("1e1000000", "2e1000000", "1", false, false),
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"number","nullable":false,"minimum":{low},"maximum":{high},"multipleOf":{step}}}}}}}"#
        );
        let wire = numeric(bytes.as_bytes());
        assert_eq!(
            conditions(&wire, "proof.step_interval_gap").len(),
            usize::from(gap)
        );
        assert_eq!(
            conditions(&wire, "proof.numeric_capability_unavailable").len(),
            usize::from(!supported)
        );
    }
    for step in ["0", "-1", "false", "\"bad\""] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"number","nullable":false,"minimum":0.1,"maximum":0.2,"multipleOf":{step}}}}}}}"#
        );
        let wire = numeric(bytes.as_bytes());
        assert!(conditions(&wire, "proof.step_interval_gap").is_empty());
        assert_eq!(wire["outcome"]["errors"], 1);
        assert_eq!(numeric_area_row(&wire)["status"], "not_applicable");
    }
}

#[test]
fn four_operands_preserve_nearest_donor_and_consumer_origins() {
    let provider = br#"{"info":{},"namespace":{"p":"urn:example:numeric"},"defaultNamespace":"p","sdfData":{"Range":{"type":"number","nullable":false,"minimum":1.1,"maximum":1.2,"multipleOf":0.5}}}"#;
    let consumer = br#"{"info":{},"namespace":{"shared":"urn:example:numeric"},"sdfProperty":{"Rea/d~ing":{"sdfRef":"shared:#/sdfData/Range","maximum":1.4}}}"#;
    for reverse in [false, true] {
        let provider_input = SourceInput::supplied("provider", provider);
        let consumer_input = SourceInput::supplied("consumer", consumer);
        let inputs = if reverse {
            [consumer_input, provider_input]
        } else {
            [provider_input, consumer_input]
        };
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let root = catalogue.source_id("consumer").unwrap();
        let donor = catalogue.source_id("provider").unwrap();
        let report = catalogue
            .check(
                CheckRequest::new()
                    .root(root)
                    .assessment(Assessment::ExpandedNumeric),
            )
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let warnings = conditions(&wire, "proof.step_interval_gap");
        assert_eq!(warnings.len(), 1);
        let warning = warnings[0];
        assert_eq!(warning["primary"]["kind"], "effective");
        assert_eq!(warning["primary"]["root"], root.get());
        assert_eq!(warning["primary"]["pointer"], "/sdfProperty/Rea~1d~0ing");
        let related = warning["related"].as_array().unwrap();
        assert!(related.len() <= 9);
        for (name, source, pointer, raw) in [
            ("minimum", donor, "/sdfData/Range/minimum", "1.1"),
            ("maximum", root, "/sdfProperty/Rea~1d~0ing/maximum", "1.4"),
            ("multipleOf", donor, "/sdfData/Range/multipleOf", "0.5"),
            ("nullable", donor, "/sdfData/Range/nullable", "false"),
        ] {
            if name != "nullable" {
                assert_eq!(warning["arguments"][name], raw);
            } else {
                assert!(warning["arguments"].get(name).is_none());
            }
            let location = related
                .iter()
                .find(|location| {
                    location["kind"] == "source"
                        && location["source_id"] == source.get()
                        && location["pointer"] == pointer
                })
                .unwrap();
            let span = location["span"].as_array().unwrap();
            let start = span[0].as_u64().unwrap() as usize;
            let end = span[1].as_u64().unwrap() as usize;
            assert_eq!(
                &catalogue.source(source).unwrap().raw_bytes()[start..end],
                raw.as_bytes()
            );
            assert!(
                std::iter::once(&warning["primary"]).chain(related).any(
                    |location| location["kind"] == "effective"
                        && location["root"] == root.get()
                        && location["pointer"] == format!("/sdfProperty/Rea~1d~0ing/{name}")
                ),
                "{name}"
            );
        }
        assert!(related.iter().any(|location| location["kind"] == "source"
            && location["source_id"] == root.get()
            && location["pointer"] == "/sdfProperty/Rea~1d~0ing/sdfRef"));
        assert_eq!(catalogue.source(donor).unwrap().raw_bytes(), provider);
        assert_eq!(catalogue.source(root).unwrap().raw_bytes(), consumer);
    }
}

#[test]
fn full_roles_alternatives_and_critical_context_are_established_before_proofs() {
    let leaf = serde_json::json!({"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9});
    let bytes = serde_json::json!({"info":{"features":[],"title":false},
        "namespace":{"p":"urn:example:roles"},"defaultNamespace":"p",
        "sdfThing":{"thing":{"minItems":2,"maxItems":1,"sdfThing":{"nested":{}},"sdfObject":{"object":{
            "sdfProperty":{"property":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9,
                "readable":true,"writable":false,"observable":true,"sdfRequired":[true]}},
            "sdfData":{"data":leaf},
            "sdfAction":{"action":{"sdfInputData":leaf,"sdfOutputData":leaf,"sdfData":{"local":leaf}}},
            "sdfEvent":{"event":{"sdfOutputData":leaf,"sdfData":{"local":leaf}}}
        }}}},
        "sdfData":{"map":{"type":"object","properties":{"entry":leaf}},
            "rows":{"type":"array","items":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9}},
            "choice":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9,
                "sdfChoice":{"g/ap~":{},"allowed":{"maximum":1},"unknown":{"type":"extended"}}},
            "enum":{"type":"string","enum":["1","1"]}}}).to_string();
    let wire = numeric(bytes.as_bytes());
    let warnings = conditions(&wire, "proof.integer_interval_gap");
    for path in [
        "/sdfThing/thing/sdfObject/object/sdfProperty/property",
        "/sdfThing/thing/sdfObject/object/sdfData/data",
        "/sdfThing/thing/sdfObject/object/sdfAction/action/sdfInputData",
        "/sdfThing/thing/sdfObject/object/sdfAction/action/sdfOutputData",
        "/sdfThing/thing/sdfObject/object/sdfAction/action/sdfData/local",
        "/sdfThing/thing/sdfObject/object/sdfEvent/event/sdfOutputData",
        "/sdfThing/thing/sdfObject/object/sdfEvent/event/sdfData/local",
        "/sdfData/map/properties/entry",
    ] {
        assert!(
            warnings.iter().any(|row| row["primary"]["pointer"] == path),
            "{path}"
        );
    }
    assert!(
        warnings
            .iter()
            .any(|row| row["arguments"]["alternative"]["name"] == "g/ap~")
    );
    assert!(
        !warnings
            .iter()
            .any(|row| row["primary"]["pointer"] == "/sdfData/rows/items"
                || row["primary"]["pointer"] == "/sdfData/choice"
                || row["primary"]["pointer"] == "/sdfThing/thing")
    );
    assert!(conditions(&wire, "proof.numeric_const_bound").is_empty());
    assert_eq!(numeric_area_row(&wire)["status"], "undetermined");
    for extra in [
        serde_json::json!({"features":["urn:example:critical"]}),
        serde_json::json!({"features":false}),
    ] {
        let bytes = serde_json::json!({"info":extra,"sdfData":{"leaf":leaf}}).to_string();
        let wire = numeric(bytes.as_bytes());
        assert!(conditions(&wire, "proof.integer_interval_gap").is_empty());
        assert_eq!(numeric_area_row(&wire)["status"], "undetermined");
    }
}

#[test]
fn shared_remaining_work_has_an_exact_pass_boundary_and_keeps_earlier_findings() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9}}}"#;
    let check = |inputs: &[SourceInput<'_>], limit| {
        let catalogue =
            Catalogue::with_limits(inputs, Limits::default().numeric_work(limit)).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        (report.assessment_area_status(AREA).present().unwrap(), wire)
    };
    let one = [SourceInput::supplied("one", bytes)];
    let cap = (1..512)
        .find(|limit| check(&one, *limit).0 == AssessmentStatus::Assessed)
        .unwrap();
    let at = check(&one, cap);
    let before = check(&one, cap - 1);
    assert_eq!(conditions(&at.1, "proof.integer_interval_gap").len(), 1);
    assert_eq!(before.0, AssessmentStatus::Undetermined);
    assert!(conditions(&before.1, "proof.integer_interval_gap").is_empty());
    let work = conditions(&before.1, "proof.numeric_capability_unavailable");
    assert!(
        work.iter()
            .any(|row| row["arguments"]["reason"] == "exact numeric work limit")
    );
    for row in work {
        assert!(row["basis"].get("citation").is_none());
    }
    let catalogue = Catalogue::with_limits(&one, Limits::default().numeric_work(cap - 1)).unwrap();
    let leaf = catalogue
        .sources()
        .next()
        .unwrap()
        .json()
        .require()
        .unwrap()
        .get("sdfData")
        .require()
        .unwrap()
        .get("value")
        .require()
        .unwrap();
    assert!(matches!(
        leaf.get("minimum").require().unwrap().number(),
        Field::Present(_)
    ));
    assert!(matches!(
        leaf.get("maximum").require().unwrap().number(),
        Field::Present(_)
    ));
    let two = [
        SourceInput::supplied("one", bytes),
        SourceInput::supplied("two", bytes),
    ];
    let two_cap = (cap..1024)
        .find(|limit| check(&two, *limit).0 == AssessmentStatus::Assessed)
        .unwrap();
    assert!(two_cap > cap);
    let partial = check(&two, two_cap - 1);
    assert_eq!(partial.0, AssessmentStatus::Undetermined);
    assert_eq!(
        conditions(&partial.1, "proof.integer_interval_gap").len(),
        1
    );
    assert_eq!(
        conditions(&check(&two, two_cap).1, "proof.integer_interval_gap").len(),
        2
    );
    let with_sample = [
        SourceInput::supplied("one", bytes),
        SourceInput::supplied("sample", b"123456").sample(),
    ];
    assert_eq!(check(&with_sample, cap).0, AssessmentStatus::Undetermined);
    let catalogue =
        Catalogue::with_limits(&with_sample, Limits::default().numeric_work(cap)).unwrap();
    let first = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    let second = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    assert_eq!(first.to_json(), second.to_json());
    println!(
        "actual shared numeric-work boundary one={cap} two={two_cap}; one-less partial and acquisition sample debit preserved"
    );
}

#[test]
fn presentation_saturation_cannot_hide_a_later_effective_parameter_error() {
    let first = br#"{"info":{},"sdfData":{"value":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9}}}"#;
    let later = br##"{"info":{},"sdfData":{"Base":{"required":false}},"sdfProperty":{"Receiver":{"sdfRef":"#/sdfData/Base","type":"object"}}}"##;
    let inputs = [
        SourceInput::supplied("first", first),
        SourceInput::supplied("later", later),
    ];
    let reference = Catalogue::from_inputs(&inputs).unwrap();
    let full = reference
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    let limited =
        Catalogue::with_limits(&inputs, Limits::default().conditions(1).pointer_bytes(1)).unwrap();
    let report = limited
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    assert_eq!(report.outcome().warnings(), full.outcome().warnings());
    assert_eq!(report.outcome().errors(), full.outcome().errors());
    assert!(report.outcome().errors() > 0);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["diagnostics"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        wire["diagnostics"]["items"][0]["code"],
        "proof.integer_interval_gap"
    );
    assert!(wire["diagnostics"]["items"][0]["primary"]["pointer"].is_null());
    assert_eq!(wire["diagnostics"]["truncated"], true);
    assert!(wire["diagnostics"]["omitted"].as_u64().unwrap() > 0);
}

#[test]
fn unavailable_step_keeps_independent_integer_warning_and_full_factor_width() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9,"multipleOf":12}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", bytes)],
        Limits::default().coefficient_digits(1),
    )
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedNumeric))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(conditions(&wire, "proof.integer_interval_gap").len(), 1);
    assert!(conditions(&wire, "proof.step_interval_gap").is_empty());
    assert_eq!(
        report.assessment_area_status(AREA).present(),
        Some(AssessmentStatus::Undetermined)
    );
    assert_numeric_inventory(&report, &wire);
    let unfinished = conditions(&wire, "proof.numeric_capability_unavailable");
    assert!(
        unfinished
            .iter()
            .any(|row| row["arguments"]["proof_class"] == "proof.step_interval_gap")
    );
    for row in unfinished {
        assert!(row["basis"].get("citation").is_none());
    }

    // This coefficient is coprime to ten, so its integer lattice has a
    // 512-digit positive numerator. Neither one nor two belongs to it.
    let fractional = format!("0.{}", "9".repeat(512));
    for (kind, gap) in [("integer", true), ("number", false)] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"{kind}","nullable":false,"minimum":1,"maximum":2,"multipleOf":{fractional}}}}}}}"#
        );
        let wire = numeric(bytes.as_bytes());
        assert_eq!(
            conditions(&wire, "proof.step_interval_gap").len(),
            usize::from(gap)
        );
        assert!(conditions(&wire, "proof.numeric_capability_unavailable").is_empty());
    }
}

#[test]
fn step_guards_do_not_use_default_and_other_profiles_keep_the_initial_inventory() {
    for (extra, gap) in [
        ("", false),
        (r#", "nullable":true"#, false),
        (r#", "default":0.15"#, false),
        (r#", "const":null"#, false),
        (r#", "const":false"#, true),
        (r#", "nullable":false"#, true),
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"value":{{"type":"number","minimum":0.1,"maximum":0.2,"multipleOf":0.25{extra}}}}}}}"#
        );
        assert_eq!(
            conditions(&numeric(bytes.as_bytes()), "proof.step_interval_gap").len(),
            usize::from(gap),
            "{extra}"
        );
    }
    let bytes = br#"{"info":{},"sdfData":{"value":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    for assessment in [
        Assessment::Initial,
        Assessment::ExpandedModified,
        Assessment::ExpandedUnits,
        Assessment::ExpandedContentFormat,
        Assessment::ExpandedPatternSyntax,
        Assessment::FullBaseline,
    ] {
        let report = catalogue
            .check(CheckRequest::new().assessment(assessment))
            .unwrap();
        assert!(matches!(report.assessment_area_status(AREA), Field::Absent));
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(conditions(&wire, "proof.integer_interval_gap").is_empty());
        assert!(conditions(&wire, "proof.step_interval_gap").is_empty());
        if assessment == Assessment::Initial {
            assert_eq!(wire["coverage"]["requested"]["total"], 8);
            assert_eq!(wire["request"]["inventory"], "esdieff.initial/1");
        }
    }
}

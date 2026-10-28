//! Inline Context definitions follow draft-ietf-asdf-sdf-nonaffordance-06 and ADR 0046.
use esdieff::{
    Assessment, BasisKind, Catalogue, CheckRequest, Class, EffectiveRequest, Field, InspectRequest,
    Lookup, SourceInput, View,
};
use serde_json::Value;

const DRAFT: &str = "draft-ietf-asdf-sdf-nonaffordance-06";
const DEVICE: &[u8] = br#"{
  "info":{"title":"Context example","version":"2026-10-27"},
  "sdfObject":{"Device":{
    "sdfProperty":{"serialNumber":{"type":"integer","nullable":false}},
    "sdfContext":{
      "serialNumber":{"type":"string","description":"Factory identifier"},
      "installationLocation":{"type":"object","properties":{
        "floor":{"type":"integer"},"room":{"type":"string"}},"required":["room"]},
      "calibrationOffset":{"type":"number","unit":"Cel"},
      "mountType":{"enum":["wall","ceiling"]},
      "labels":{"type":"array","items":{"sdfChoice":{"text":{
        "type":"string","nullable":false,"pattern":"^[a-z]+$"}}}}
    }}},
  "sdfThing":{"Fleet":{"sdfContext":{}}}
}"#;

fn catalogue(bytes: &[u8]) -> Catalogue {
    Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap()
}

fn full(catalogue: &Catalogue) -> esdieff::Report<'_> {
    catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap()
}

fn codes(report: &esdieff::Report<'_>) -> Vec<String> {
    report
        .diagnostics()
        .conditions()
        .map(|condition| condition.code().to_owned())
        .collect()
}

#[test]
fn source_and_effective_views_keep_context_separate_from_same_name_properties() {
    let catalogue = catalogue(DEVICE);
    let id = catalogue.source_id("model").unwrap();
    let source = catalogue.source(id).unwrap();
    let Lookup::Found(object) = source.definition(Class::Object, "Device") else {
        panic!("Object is inspectable");
    };
    let rows = object.children(Class::Context).require().unwrap();
    assert_eq!(rows.len(), 5);
    let (_, context) = rows.into_iter().next().unwrap();
    assert_eq!(context.class(), Class::Context);
    assert!(matches!(context.readable(), Field::Absent));
    assert!(matches!(
        context.data_description().require().unwrap().nullable(),
        Field::Absent
    ));
    assert_eq!(
        context.json().pointer(8192).require().unwrap(),
        "/sdfObject/Device/sdfContext/serialNumber"
    );
    let effective = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(object) = effective.definition(Class::Object, "Device") else {
        panic!("effective Object is inspectable");
    };
    let (_, context) = object
        .children(Class::Context)
        .require()
        .unwrap()
        .next()
        .unwrap();
    let context = context.require().unwrap();
    assert_eq!(context.class(), Class::Context);
    assert!(matches!(context.readable_characteristic(), Field::Absent));
    assert!(matches!(context.writable(), Field::Absent));
    assert!(matches!(context.observable(), Field::Absent));
    assert!(matches!(
        context.data_description().require().unwrap().nullable(),
        Field::Unavailable(_)
    ));
    assert!(matches!(
        effective.default_explanation("/sdfObject/Device/sdfContext/serialNumber/nullable"),
        Field::Unavailable(_)
    ));
    let (_, property) = object
        .children(Class::Property)
        .require()
        .unwrap()
        .next()
        .unwrap();
    assert!(property.require().unwrap().readable().require().unwrap());
    let report = full(&catalogue);
    assert_eq!(report.outcome().errors(), 0, "{}", report.to_json());
    assert!(report.outcome().complete(), "{}", report.to_json());
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["baseline"]["extensions"][0]["id"], DRAFT);
    assert!(report.to_human().contains(DRAFT));
}

#[test]
fn context_placement_and_shapes_have_draft_findings() {
    for (model, code, pointer) in [
        (r#"{"sdfContext":{}}"#, "context.placement", "/sdfContext"),
        (
            r#"{"sdfProperty":{"p":{"sdfContext":{}}}}"#,
            "context.placement",
            "/sdfProperty/p/sdfContext",
        ),
        (
            r#"{"sdfObject":{"O":{"sdfContext":[]}}}"#,
            "context.group_shape",
            "/sdfObject/O/sdfContext",
        ),
        (
            r#"{"sdfThing":{"T":{"sdfContext":{"bad":17}}}}"#,
            "context.definition_shape",
            "/sdfThing/T/sdfContext/bad",
        ),
    ] {
        let catalogue = catalogue(model.as_bytes());
        let report = full(&catalogue);
        let diagnostics = report.diagnostics();
        let condition = diagnostics.conditions().find(|c| c.code() == code).unwrap();
        assert_eq!(condition.basis().kind(), BasisKind::Interpretation);
        assert_eq!(condition.basis().id(), Some(DRAFT));
        assert_eq!(condition.severity(), esdieff::Severity::Error);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let condition = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["code"] == code)
            .unwrap();
        assert_eq!(condition["primary"]["pointer"], pointer);
        assert!(condition["primary"]["span"].is_array());
    }
    let catalogue = catalogue(br#"{"sdfObject":{"O":{"sdfContext":{"bad:name":{}}}}}"#);
    assert!(codes(&full(&catalogue)).contains(&"name.colon".to_owned()));
}

#[test]
fn supported_context_data_checks_retain_independent_affordance_findings() {
    let catalogue = catalogue(
        br#"{"sdfObject":{"O":{
      "sdfProperty":{"p":{"type":"number","multipleOf":0}},
      "sdfContext":{
        "numeric":{"type":"number","minimum":"bad"},
        "map":{"type":"object","required":[],"properties":{"x":{"type":false}}},
        "choice":{"sdfChoice":{"x":{"pattern":"["}}},
        "constant":{"type":"integer","nullable":false,"const":"bad"},
        "format":{"type":"string","format":false},
        "unit":{"type":"number","unit":17}
      }}}}"#,
    );
    let report = full(&catalogue);
    let codes = codes(&report);
    for code in [
        "parameter.positive_step",
        "data.number",
        "data.required",
        "data.string",
        "pattern.syntax",
        "proof.primitive_const_type",
    ] {
        assert!(
            codes.iter().any(|found| found == code),
            "{code}: {}",
            report.to_json()
        );
    }
    assert!(!codes.iter().any(|code| code == "context.placement"));
    let consistency = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert!(
        consistency
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "consistency.const_excluded")
    );
}

#[test]
fn nullability_uncertainty_is_scoped_to_conclusions_needing_the_default() {
    for (nullable, incomplete) in [
        ("", true),
        (",\"nullable\":true", false),
        (",\"nullable\":false", false),
    ] {
        let model = format!(
            r#"{{"sdfObject":{{"O":{{"sdfContext":{{"c":{{"type":"string","const":null{nullable}}},"n":{{"type":"number","minimum":0,"const":1}}}},"sdfProperty":{{"p":{{"type":"string","const":null}}}}}}}}}}"#
        );
        let catalogue = catalogue(model.as_bytes());
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        assert_eq!(
            !report.outcome().complete(),
            incomplete,
            "{}",
            report.to_json()
        );
        let codes = codes(&report);
        assert_eq!(
            codes
                .iter()
                .any(|code| code == "context.nullable_default_unavailable"),
            incomplete
        );
        assert_eq!(
            codes
                .iter()
                .any(|code| code == "consistency.const_excluded"),
            nullable.contains("false")
        );
    }
}

#[test]
fn context_requirements_are_undefined_and_never_satisfy_affordance_requirements() {
    for (required, property, undefined) in [
        ("serial", true, false),
        ("serial", false, true),
        ("#/sdfObject/O/sdfContext/serial", true, true),
    ] {
        let property = if property {
            r#", "sdfProperty":{"serial":{"type":"string"}}"#
        } else {
            ""
        };
        let model = format!(
            r#"{{"sdfObject":{{"O":{{"sdfContext":{{"serial":{{"type":"string"}}}},"sdfRequired":["{required}"]{property}}}}}}}"#
        );
        let catalogue = catalogue(model.as_bytes());
        let report = full(&catalogue);
        assert_eq!(report.outcome().errors(), 0, "{}", report.to_json());
        assert_eq!(
            codes(&report)
                .iter()
                .any(|code| code == "context.required_undefined"),
            undefined,
            "{}",
            report.to_json()
        );
        let id = catalogue.source_id("model").unwrap();
        let inspect = catalogue
            .inspect(
                InspectRequest::new(id, "/sdfObject/O")
                    .view(View::Effective)
                    .required_declarations(true),
            )
            .unwrap();
        if undefined {
            assert!(
                inspect
                    .diagnostics()
                    .conditions()
                    .any(|condition| condition.code() == "context.required_undefined")
            );
        } else {
            let declarations = inspect.required_declarations().require().unwrap();
            let mut targets = Vec::new();
            for owner in declarations.owners() {
                assert!(!matches!(owner.class(), Field::Present(Class::Context)));
                for reference in owner.references().present().into_iter().flatten() {
                    let Lookup::Found(found) = reference.targets() else {
                        panic!("the same-name Property is the required target");
                    };
                    targets.extend(found.map(|target| target.class()));
                }
            }
            assert_eq!(targets, [Class::Property]);
        }
    }
    let catalogue =
        catalogue(br#"{"sdfObject":{"O":{"sdfContext":{"c":{"sdfRequired":[true]}}}}}"#);
    assert!(
        codes(&full(&catalogue))
            .iter()
            .any(|code| code == "context.required_undefined")
    );
}

#[test]
fn link_meaning_and_property_characteristics_are_not_invented() {
    let catalogue = catalogue(
        br#"{"sdfObject":{"O":{"sdfContext":{
      "link":{"sdfType":"link"},
      "flags":{"type":"number","readable":true,"writable":false,"observable":true}
    }}}}"#,
    );
    let report = full(&catalogue);
    assert!(!report.outcome().complete(), "{}", report.to_json());
    let effective = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
        .unwrap();
    let Lookup::Found(object) = effective.definition(Class::Object, "O") else {
        panic!()
    };
    let mut rows = object.children(Class::Context).require().unwrap();
    let (_, link) = rows.next().unwrap();
    assert_eq!(
        link.require()
            .unwrap()
            .data_description()
            .require()
            .unwrap()
            .sdf_type()
            .require()
            .unwrap(),
        "link"
    );
    let (_, flags) = rows.next().unwrap();
    assert!(matches!(flags.require().unwrap().readable(), Field::Absent));
}

#[test]
fn recognizing_context_does_not_clear_an_unknown_critical_feature() {
    let catalogue = catalogue(br#"{"info":{"features":["https://example.com/unknown"]},"sdfObject":{"O":{"sdfContext":{"c":{"type":"integer"}}}}}"#);
    let report = full(&catalogue);
    assert!(!report.outcome().complete());
    assert!(
        codes(&report)
            .iter()
            .any(|code| code == "extension.critical_unsupported")
    );
}

#[test]
fn reference_reuse_cannot_turn_an_unresolved_context_default_into_a_property_default() {
    let catalogue = catalogue(br##"{"sdfObject":{"O":{
      "sdfContext":{"c":{"type":"string"}},
      "sdfProperty":{"p":{"sdfRef":"#/sdfObject/O/sdfContext/c"},"independent":{"type":"number","multipleOf":0}}
    }}}"##);
    let report = full(&catalogue);
    let codes = codes(&report);
    assert!(
        codes
            .iter()
            .any(|code| code == "reference.capability_unavailable")
    );
    assert!(codes.iter().any(|code| code == "parameter.positive_step"));
    assert!(!report.outcome().complete());
}

#[test]
fn nested_context_reference_reuse_keeps_the_withheld_default_unavailable() {
    let catalogue = catalogue(
        br##"{"sdfObject":{"O":{
      "sdfContext":{"c":{"type":"object","properties":{"x":{"type":"string"}}}},
      "sdfProperty":{"p":{"sdfRef":"#/sdfObject/O/sdfContext/c/properties/x","const":null}}
    }}}"##,
    );
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert!(!report.outcome().complete(), "{}", report.to_json());
    assert!(
        codes(&report)
            .iter()
            .any(|code| code == "reference.capability_unavailable")
    );
}

#[test]
fn selected_nested_context_descriptions_retain_their_default_boundary() {
    let catalogue = catalogue(
        br#"{"sdfObject":{"O":{"sdfContext":{"c":{
      "type":"object","properties":{"x":{"type":"string"}},"const":{"literal":1}
    }}}}}"#,
    );
    let id = catalogue.source_id("model").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfObject/O/sdfContext/c/properties/x"))
        .unwrap();
    assert!(matches!(
        model
            .json()
            .require()
            .unwrap()
            .default_explanation("nullable"),
        Field::Unavailable(_)
    ));
    let literal = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfObject/O/sdfContext/c/const"))
        .unwrap();
    assert!(matches!(
        literal
            .json()
            .require()
            .unwrap()
            .default_explanation("nullable"),
        Field::Absent
    ));
}

#[test]
fn nested_context_descriptions_and_alternatives_keep_explicit_nullability() {
    let catalogue = catalogue(
        br#"{"sdfObject":{"O":{"sdfContext":{
      "map":{"type":"object","properties":{"x":{"type":"string","const":null}}},
      "choice":{"sdfChoice":{"absent":{"type":"string","const":null},
        "explicit":{"type":"string","nullable":true,"const":null}}}
    }}}}"#,
    );
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert!(!report.outcome().complete(), "{}", report.to_json());
    let id = catalogue.source_id("model").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfObject/O"))
        .unwrap();
    let Lookup::Found(choice) = model.definition(Class::Context, "choice") else {
        panic!("Context choice")
    };
    let alternatives = choice
        .data_description()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap();
    for (id, description) in alternatives {
        let description = description.require().unwrap();
        match id {
            esdieff::AlternativeId::Choice("absent") => {
                assert!(matches!(description.nullable(), Field::Unavailable(_)))
            }
            esdieff::AlternativeId::Choice("explicit") => {
                assert!(description.nullable().require().unwrap())
            }
            _ => panic!("unexpected alternative"),
        }
    }
}

/// Small source files for the public CLI examples, removed when the test ends.
struct TempFile(std::path::PathBuf);
impl TempFile {
    fn new(bytes: &[u8]) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-context-{}-{}.sdf.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn cli_checks_and_source_and_effective_inspection_identify_the_draft() {
    let model = TempFile::new(include_bytes!(
        "../examples/context-inspection/device.sdf.json"
    ));
    for args in [
        vec![
            "check",
            "--assessment",
            "full-baseline",
            "--format",
            "json",
            model.path(),
        ],
        vec![
            "inspect",
            "--view",
            "source",
            "--document",
            model.path(),
            "--pointer",
            "/sdfObject/Device",
            "--format",
            "json",
            model.path(),
        ],
        vec![
            "inspect",
            "--view",
            "effective",
            "--document",
            model.path(),
            "--pointer",
            "/sdfObject/Device/sdfContext/installationLocation",
            "--format",
            "json",
            model.path(),
        ],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["baseline"]["extensions"][0]["id"], DRAFT, "{report}");
        assert_eq!(report["version"], "1.0");
    }
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "check",
            "--assessment",
            "full-baseline",
            "--color",
            "never",
            model.path(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8(output.stdout).unwrap().contains(DRAFT));
}

#[test]
fn cli_context_placement_errors_keep_their_location_and_draft_basis() {
    let model = TempFile::new(br#"{"sdfContext":{"serial":{"type":"string"}}}"#);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--format", "json", model.path()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let condition = report["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|condition| condition["code"] == "context.placement")
        .unwrap();
    assert_eq!(condition["primary"]["pointer"], "/sdfContext");
    assert_eq!(condition["basis"]["id"], DRAFT);
    assert_eq!(condition["basis"]["kind"], "interpretation");
}

#[test]
fn removing_context_preserves_client_interactions() {
    let old = TempFile::new(br#"{"sdfObject":{"O":{"sdfContext":{"c":{"type":"number"}}}}}"#);
    let new = TempFile::new(br#"{"sdfObject":{"O":{"sdfContext":{}}}}"#);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "diff",
            "--old",
            old.path(),
            "--new",
            new.path(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["result"]["differences"][0]["subject"], "sdfContext");
    assert_eq!(wire["result"]["differences"][0]["kind"], "removed");
    assert_eq!(
        wire["result"]["differences"][0]["classification"],
        "compatible"
    );
}

#[test]
fn inline_context_authoring_and_editing_do_not_require_a_model_check() {
    let source = br#"{"namespace":{"own":"https://example.com/context"},"defaultNamespace":"own","sdfObject":{"O":{"sdfContext":{"c":{"type":"string","label":"serial"}}}}}"#;
    let inputs = catalogue(source);
    let id = inputs.source_id("model").unwrap();
    let result = esdieff::AuthorRequest::new(inputs, "recipe", "result", "Device")
        .unwrap()
        .namespace("reuse", "https://example.com/context")
        .unwrap()
        .namespace("product", "https://example.com/product")
        .unwrap()
        .default_namespace("product")
        .unwrap()
        .object(esdieff::AuthorObject::new("object", id, "/sdfObject/O", "reuse", false).unwrap())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.generation(), esdieff::Generation::Complete);
    assert_eq!(result.fidelity(), esdieff::Fidelity::Established);
    assert!(result.document_deliverable());
    let inputs = catalogue(source);
    let id = inputs.source_id("model").unwrap();
    let result = esdieff::EditRequest::new(inputs, "edit", id, "result")
        .unwrap()
        .with_edit(esdieff::Edit::remove("/sdfObject/O/sdfContext/c", "label").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    assert_eq!(result.fidelity(), esdieff::Fidelity::Established);
    assert!(result.document_deliverable());
}

#[test]
fn draft_identity_and_error_counts_survive_diagnostic_detail_limits() {
    let model = br#"{"sdfContext":{},"sdfProperty":{"p":{"type":"number","multipleOf":0}}}"#;
    let normal = catalogue(model);
    let limited = Catalogue::with_limits(
        &[SourceInput::supplied("model", model)],
        esdieff::Limits::default().conditions(1).diagnostic_bytes(1),
    )
    .unwrap();
    let normal = full(&normal);
    let limited = full(&limited);
    assert_eq!(limited.outcome().errors(), normal.outcome().errors());
    assert_eq!(limited.outcome().complete(), normal.outcome().complete());
    let report: Value = serde_json::from_str(&limited.to_json()).unwrap();
    assert_eq!(report["baseline"]["extensions"][0]["id"], DRAFT);
    assert!(report["diagnostics"]["omitted"].as_u64().unwrap() > 0);
}

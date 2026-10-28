//! Published draft examples remain unchanged and usable as offline fixtures.
use esdieff::{Admission, Catalogue, SampleKind, SampleRequest, SourceInput};
use serde_json::Value;
use std::{path::Path, process::Command};

const DEVICE: &[u8] = include_bytes!("../examples/context-draft-06/device-with-context.sdf.json");
const REVISED: &[u8] =
    include_bytes!("../examples/context-draft-06/device-with-context-revised.sdf.json");
const ASSET: &[u8] = include_bytes!("../examples/context-draft-06/asset-container.sdf.json");
const ENVIRONMENT: &[u8] =
    include_bytes!("../examples/context-draft-06/environmental-sensor.sdf.json");
const METADATA: &[u8] = include_bytes!("../examples/context-draft-06/device-metadata.sdf.json");
const LINKS: &[u8] = include_bytes!("../examples/context-draft-06/device-with-links.sdf.json");
const CALIBRATION: &str = "/sdfObject/deviceWithContext/sdfContext/calibrationOffset";

#[test]
fn saved_plan_checks_all_five_figures_and_the_derived_revision() {
    let plan =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/context-draft-06/check-plan.json");
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--plan"])
        .arg(plan)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(3),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    let cases = wire["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for (index, case) in cases[..5].iter().enumerate() {
        let report = &case["report"];
        assert_eq!(report["outcome"]["errors"], 0, "{case}");
        assert_eq!(report["outcome"]["complete"], index < 4, "{case}");
        assert_eq!(
            report["outcome"]["disposition"],
            if index < 4 { "passed" } else { "undetermined" },
            "{case}"
        );
        assert_eq!(
            report["outcome"]["warnings"],
            usize::from(index > 0),
            "{case}"
        );
    }
    let links = &cases[4]["report"]["diagnostics"]["items"];
    assert_eq!(
        links
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["code"] == "extension.optional_unsupported")
            .count(),
        3
    );

    let diff = &cases[5]["report"];
    assert_eq!(diff["outcome"]["complete"], true, "{diff}");
    assert_eq!(diff["result"]["outcome"], "compatible");
    assert_eq!(diff["result"]["summary"]["breaking"], 0);
    assert_eq!(diff["result"]["summary"]["undetermined"], 0);
    assert_eq!(diff["policy"]["compatibility_scope"], "client_interactions");
    let differences = diff["result"]["differences"].as_array().unwrap();
    assert_eq!(differences.len(), 1);
    let difference = &differences[0];
    assert_eq!(difference["subject"], "sdfContext");
    assert_eq!(difference["kind"], "changed");
    assert_eq!(difference["classification"], "compatible");
    assert_eq!(difference["directions"], serde_json::json!([]));
    for side in ["earlier", "later"] {
        assert_eq!(difference[side]["location"]["pointer"], CALIBRATION);
    }
}

fn sample(model: &[u8], pointer: &str, value: &str) -> (Admission, bool) {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model),
        SourceInput::supplied("sample", value.as_bytes()).sample(),
    ])
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("model").unwrap(),
                SampleKind::Context,
                pointer,
            )
            .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap();
    (
        report.sample_result().unwrap().admission(),
        report.outcome().complete(),
    )
}

#[test]
fn published_context_shapes_accept_and_reject_independent_values() {
    let cases: &[(&[u8], &str, &str, Admission)] = &[
        (DEVICE, CALIBRATION, "6", Admission::Admitted),
        (REVISED, CALIBRATION, "5", Admission::Admitted),
        (REVISED, CALIBRATION, "6", Admission::Rejected),
        (
            ASSET,
            "/sdfObject/assetContainer/sdfContext/physicalSpecs",
            r#"{"length":6.1,"width":2.4,"height":2.6,"maxLoadKg":28000}"#,
            Admission::Admitted,
        ),
        (
            ASSET,
            "/sdfObject/assetContainer/sdfContext/physicalSpecs",
            r#"{"length":6.1,"width":2.4,"height":2.6}"#,
            Admission::Rejected,
        ),
        (
            ENVIRONMENT,
            "/sdfObject/envSensor/sdfContext/installationInfo",
            r#"{"floor":3,"mountType":"ceiling","indoorOutdoor":"indoor"}"#,
            Admission::Admitted,
        ),
        (
            ENVIRONMENT,
            "/sdfObject/envSensor/sdfContext/installationInfo",
            r#"{"floor":3,"mountType":"pole"}"#,
            Admission::Rejected,
        ),
        (
            ENVIRONMENT,
            "/sdfObject/envSensor/sdfContext/installationInfo",
            r#"{"mountType":"ceiling"}"#,
            Admission::Rejected,
        ),
        (
            METADATA,
            "/sdfThing/deviceMetadata/sdfContext/certifications",
            r#"[{"scheme":"CE","certId":"example","region":"EU"}]"#,
            Admission::Admitted,
        ),
        (
            METADATA,
            "/sdfThing/deviceMetadata/sdfContext/certifications",
            r#"[{"scheme":7}]"#,
            Admission::Rejected,
        ),
    ];
    for &(model, pointer, value, expected) in cases {
        assert_eq!(
            sample(model, pointer, value),
            (expected, true),
            "{pointer}: {value}"
        );
    }
}

#[test]
fn unchanged_examples_preserve_unknown_nullability_and_link_semantics() {
    assert_eq!(
        sample(DEVICE, CALIBRATION, "null"),
        (Admission::Undetermined, false)
    );
    for name in ["datasheetLink", "certificationLink", "digitalTwinLink"] {
        assert_eq!(
            sample(
                LINKS,
                &format!("/sdfObject/deviceWithLinks/sdfContext/{name}"),
                r#"{"href":"https://example.com/resource"}"#
            ),
            (Admission::Undetermined, false),
            "{name}"
        );
    }
}

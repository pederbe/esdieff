//! Link values follow `draft-ietf-asdf-sdftype-link-02` under ADR 0060: Link sdfType draft
//! support. Each expected admission is read from the draft's Section 2 and Appendix A.
use esdieff::{Admission, Catalogue, SampleKind, SampleRequest, SourceInput};
use serde_json::Value;

const DRAFT: &str = "draft-ietf-asdf-sdftype-link-02";
const FIGURE_6: &[u8] =
    include_bytes!("../../examples/context-draft-06/device-with-links.sdf.json");
/// Appendix A.1's exported `idlink`, including its `minvalue` and `maxvalue` spellings.
const IDLINK: &str = r#"{"type":"object","sdfType":"link","properties":{"href":{"type":"string"},"id":{"type":"integer","minvalue":0,"maxvalue":255}}}"#;
/// Appendix A.2's `objlink`, which declares no `href`.
const OBJLINK: &str = r#"{"type":"object","sdfType":"link","properties":{"object-id":{"type":"integer","minimum":0,"maximum":65535},"object-instance-id":{"type":"integer","minimum":0,"maximum":65535}}}"#;

fn check(model: &[u8], kind: SampleKind, pointer: &str, value: &str) -> (Admission, bool, Value) {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model),
        SourceInput::supplied("sample", value.as_bytes()).sample(),
    ])
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(catalogue.source_id("model").unwrap(), kind, pointer)
                .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap();
    let wire = serde_json::from_str(&report.to_json()).unwrap();
    (
        report.sample_result().unwrap().admission(),
        report.outcome().complete(),
        wire,
    )
}

fn property(description: &str, value: &str) -> (Admission, bool, Value) {
    let model =
        format!(r#"{{"sdfObject":{{"Device":{{"sdfProperty":{{"link":{description}}}}}}}}}"#);
    check(
        model.as_bytes(),
        SampleKind::Property,
        "/sdfObject/Device/sdfProperty/link",
        value,
    )
}

fn codes(wire: &Value) -> Vec<String> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["code"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn appendix_a_values_are_admitted_and_rejected_by_their_declared_members() {
    for (description, value, expected) in [
        (
            IDLINK,
            r#"{"href":"https://example.com/info","id":7}"#,
            Admission::Admitted,
        ),
        // `href` is optional, and `minvalue`/`maxvalue` carry no range meaning.
        (IDLINK, r#"{"id":300}"#, Admission::Admitted),
        (
            IDLINK,
            r#"{"href":"https://example.com/info","id":"7"}"#,
            Admission::Rejected,
        ),
        (IDLINK, r#"{"href":5}"#, Admission::Rejected),
        (IDLINK, r#""https://example.com/info""#, Admission::Rejected),
        (
            OBJLINK,
            r#"{"object-id":3,"object-instance-id":0}"#,
            Admission::Admitted,
        ),
        (OBJLINK, r#"{"object-id":70000}"#, Admission::Rejected),
    ] {
        let (admission, complete, wire) = property(description, value);
        assert_eq!(admission, expected, "{value}: {wire}");
        assert!(complete, "{value}: {wire}");
    }
}

#[test]
fn undeclared_and_untyped_members_must_be_lowercase_strings() {
    for (value, expected, code) in [
        (
            r#"{"href":"/sensors/temp","rel":"describedby"}"#,
            Admission::Admitted,
            None,
        ),
        (
            r#"{"href":"/sensors/temp","rel":3}"#,
            Admission::Rejected,
            Some("sample.link_member"),
        ),
        (
            r#"{"Href":"/sensors/temp"}"#,
            Admission::Rejected,
            Some("sample.link_member_name"),
        ),
        (
            r#"{"href":"/sensors/temp","hrefLang":"en"}"#,
            Admission::Rejected,
            Some("sample.link_member_name"),
        ),
        // Attribute values are not checked for case.
        (
            r#"{"href":"/Sensors/Temp","title":"Temperature"}"#,
            Admission::Admitted,
            None,
        ),
    ] {
        let (admission, complete, wire) = property(IDLINK, value);
        assert_eq!(admission, expected, "{value}: {wire}");
        assert!(complete, "{value}: {wire}");
        if let Some(code) = code {
            assert!(codes(&wire).iter().any(|c| c == code), "{value}: {wire}");
        }
    }
    // A declared member without `type` is a string too.
    let untyped = r#"{"type":"object","sdfType":"link","properties":{"title":{"description":"Human-readable label"}}}"#;
    for (value, expected) in [
        (r#"{"title":"Datasheet"}"#, Admission::Admitted),
        (r#"{"title":1}"#, Admission::Rejected),
    ] {
        let (admission, complete, wire) = property(untyped, value);
        assert_eq!(admission, expected, "{value}: {wire}");
        assert!(complete, "{value}: {wire}");
    }
}

#[test]
fn required_href_and_section_2_constants_use_ordinary_map_rules() {
    let section_2 = r#"{"type":"object","sdfType":"link","required":["href"],"properties":{"href":{"type":"string"},"rt":{"type":"string","const":"temperature-c"},"if":{"type":"string","const":"sensor"}}}"#;
    for (value, expected) in [
        (
            r#"{"href":"/sensors/temp","rt":"temperature-c","if":"sensor"}"#,
            Admission::Admitted,
        ),
        (
            r#"{"rt":"temperature-c","if":"sensor"}"#,
            Admission::Rejected,
        ),
        (
            r#"{"href":"/sensors/temp","rt":"temperature-f"}"#,
            Admission::Rejected,
        ),
    ] {
        let (admission, complete, wire) = property(section_2, value);
        assert_eq!(admission, expected, "{value}: {wire}");
        assert!(complete, "{value}: {wire}");
    }
}

#[test]
fn reports_record_the_link_draft_and_its_basis() {
    let (admission, complete, wire) = property(IDLINK, r#"{"href":"https://example.com/info"}"#);
    assert_eq!(admission, Admission::Admitted);
    assert!(complete, "{wire}");
    let extensions = wire["baseline"]["extensions"].as_array().unwrap();
    let link = extensions
        .iter()
        .find(|row| row["id"] == DRAFT)
        .expect("link draft row");
    assert_eq!(
        link["sha256"],
        "27c42209d743d020a26c6d5a35b0c70b80e07beeb149db674081c9809929c95d"
    );
    let (_, _, rejected) = property(IDLINK, r#"{"Href":"x"}"#);
    let finding = rejected["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == "sample.link_member_name")
        .expect("link name finding");
    assert_eq!(finding["basis"]["kind"], "interpretation", "{finding}");
    assert_eq!(finding["basis"]["id"], DRAFT, "{finding}");
}

#[test]
fn null_and_unsupported_link_descriptions_keep_their_boundaries() {
    // A Property keeps the base-SDF nullable default.
    let (admission, complete, wire) = property(IDLINK, "null");
    assert_eq!(admission, Admission::Admitted, "{wire}");
    assert!(complete, "{wire}");
    // Link meaning needs `type: "object"`.
    let (admission, complete, wire) = property(r#"{"sdfType":"link"}"#, "{}");
    assert_eq!(admission, Admission::Undetermined, "{wire}");
    assert!(!complete, "{wire}");
    let (admission, _, wire) = property(r#"{"type":"string","sdfType":"link"}"#, r#""/x""#);
    assert_eq!(admission, Admission::Undetermined, "{wire}");
}

#[test]
fn figure_6_context_links_are_checked_with_the_context_default_boundary() {
    let pointer = "/sdfObject/deviceWithLinks/sdfContext/datasheetLink";
    for (value, expected, complete) in [
        (
            r#"{"href":"https://example.com/datasheet.pdf"}"#,
            Admission::Admitted,
            true,
        ),
        (
            r#"{"href":"https://example.com/datasheet.pdf","type":"application/pdf"}"#,
            Admission::Admitted,
            true,
        ),
        ("{}", Admission::Admitted, true),
        (r#"{"href":42}"#, Admission::Rejected, true),
        (
            r#"{"href":"https://example.com/datasheet.pdf","Type":"application/pdf"}"#,
            Admission::Rejected,
            true,
        ),
        // Absent Context `nullable` stays unresolved under ADR 0046.
        ("null", Admission::Undetermined, false),
    ] {
        let (admission, done, wire) = check(FIGURE_6, SampleKind::Context, pointer, value);
        assert_eq!(admission, expected, "{value}: {wire}");
        assert_eq!(done, complete, "{value}: {wire}");
        if expected != Admission::Undetermined {
            let ids: Vec<_> = wire["baseline"]["extensions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["id"].as_str().unwrap().to_owned())
                .collect();
            assert_eq!(
                ids,
                ["draft-ietf-asdf-sdf-nonaffordance-06", DRAFT],
                "{wire}"
            );
        }
    }
}

fn model(version: &str, link: &str, maximum: u32) -> String {
    format!(
        r#"{{"info":{{"title":"Links","version":"{version}"}},"sdfObject":{{"Device":{{"sdfProperty":{{"link":{{"type":"object"{link},"properties":{{"id":{{"type":"integer","maximum":{maximum}}},"rt":{{"type":"string"}}}}}}}}}}}}}}"#
    )
}

fn compare(earlier: &str, later: &str) -> Value {
    let earlier =
        Catalogue::from_inputs(&[SourceInput::supplied("model", earlier.as_bytes())]).unwrap();
    let later =
        Catalogue::from_inputs(&[SourceInput::supplied("model", later.as_bytes())]).unwrap();
    let comparison = esdieff::ComparisonRequest::new(&earlier, &later)
        .compare()
        .unwrap();
    serde_json::from_str(&comparison.to_json()).unwrap()
}

#[test]
fn revision_comparison_classifies_link_constraints_and_conventions() {
    const LINK: &str = r#","sdfType":"link""#;
    // A narrowed member bound is classified as for any other data.
    for link in [LINK, ""] {
        let wire = compare(&model("1", link, 65535), &model("2", link, 255));
        let difference = &wire["result"]["differences"][0];
        assert_eq!(difference["quality"], "maximum", "{wire}");
        assert_eq!(difference["classification"], "breaking", "{wire}");
    }
    // Adding or removing the convention is breaking with `{"X":""}`.
    for (earlier, later) in [("", LINK), (LINK, "")] {
        let wire = compare(&model("1", earlier, 9), &model("2", later, 9));
        let difference = &wire["result"]["differences"][0];
        assert_eq!(difference["quality"], "sdfType", "{wire}");
        assert_eq!(difference["classification"], "breaking", "{wire}");
        assert_eq!(difference["witness"]["raw_json"], r#"{"X":""}"#, "{wire}");
    }
    // Swapping conventions is breaking without a witness.
    let wire = compare(
        &model("1", LINK, 9),
        &model("2", r#","sdfType":"byte-string""#, 9),
    );
    assert_eq!(
        wire["result"]["differences"][0]["classification"],
        "breaking"
    );
    let ids: Vec<_> = wire["baseline"]["extensions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].clone())
        .collect();
    assert_eq!(ids, ["draft-ietf-asdf-sdf-nonaffordance-06", DRAFT]);
    assert_eq!(
        wire["request"]["comparison"],
        "esdieff.revision-comparison/10"
    );
}

fn full_check(document: &str) -> Value {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", document.as_bytes())]).unwrap();
    let request = esdieff::CheckRequest::new().assessment(esdieff::Assessment::FullBaseline);
    serde_json::from_str(&catalogue.check(request).unwrap().to_json()).unwrap()
}

#[test]
fn model_checks_need_object_links_only_where_link_meaning_is_needed() {
    let wire = full_check(&model("1", r#","sdfType":"link""#, 9));
    assert_eq!(wire["outcome"]["complete"], true, "{wire}");
    assert_eq!(wire["outcome"]["errors"], 0, "{wire}");
    assert!(
        wire["baseline"]["extensions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == DRAFT),
        "{wire}"
    );
    // An untyped link has no model conclusion that needs its meaning. Its values stay
    // undetermined, as checked above.
    for (description, complete) in [
        (r#"{"sdfType":"link"}"#, true),
        (r#"{"type":"string","sdfType":"link"}"#, false),
        (
            r#"{"type":"object","sdfType":"link","format":"uri"}"#,
            false,
        ),
    ] {
        let document = format!(
            r#"{{"info":{{"title":"Links"}},"sdfObject":{{"Device":{{"sdfProperty":{{"link":{description}}}}}}}}}"#
        );
        let wire = full_check(&document);
        assert_eq!(
            wire["outcome"]["complete"], complete,
            "{description}: {wire}"
        );
        assert_eq!(wire["outcome"]["errors"], 0, "{description}: {wire}");
    }
}

#[test]
fn authored_trace_entries_replay_link_values_against_figure_6() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", FIGURE_6)]).unwrap();
    let pointer = "/sdfObject/deviceWithLinks/sdfContext/certificationLink";
    let entry = |value: &str, expected: &str| {
        format!(
            r#"{{"kind":"context","pointer":"{pointer}","value":{value},"expected":"{expected}"}}"#
        )
    };
    let trace = [
        entry(r#"{"href":"https://example.com/cert"}"#, "admitted"),
        entry(
            r#"{"href":"https://example.com/cert","title":"CE"}"#,
            "admitted",
        ),
        entry(
            r#"{"href":"https://example.com/cert","Title":"CE"}"#,
            "rejected",
        ),
        entry(
            r#"{"href":"https://example.com/cert","anchor":1}"#,
            "rejected",
        ),
        // A wrong expectation fails replay.
        entry(r#"{"href":["https://example.com/cert"]}"#, "admitted"),
    ]
    .join("\n")
        + "\n";
    let replay = catalogue
        .check_trace(esdieff::TraceRequest::new(
            catalogue.source_id("model").unwrap(),
            trace.as_bytes(),
        ))
        .unwrap();
    assert_eq!(
        replay.count(esdieff::TraceOutcome::Passed),
        4,
        "{}",
        replay.to_json()
    );
    assert_eq!(
        replay.count(esdieff::TraceOutcome::Failed),
        1,
        "{}",
        replay.to_json()
    );
    assert_eq!(replay.count(esdieff::TraceOutcome::Undetermined), 0);
    let wire: Value = serde_json::from_str(&replay.to_json()).unwrap();
    assert!(
        wire["baseline"]["extensions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == DRAFT),
        "{wire}"
    );
}

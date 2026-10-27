//! A breaking revision whose `info` `version` does not compare higher earns one warning per
//! document pair, which never changes the disposition. Follows ADR 0044: Versions of breaking
//! revisions, and ADR 0039: Client-side revision comparison.
use esdieff::{
    Class, Classification, ComparisonRequest, DifferenceAcknowledgement,
    DifferenceAcknowledgements, DifferenceKind, DifferenceSubject, Disposition, Revision,
    RevisionComparison,
};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{catalogue, revised};

const CODE: &str = "comparison.version_not_increased";

/// A fault interrupter switch modeled on the OneDM playground revision that removed
/// `"faulted"` from its state enum and kept its version.
fn switch(info: Value, states: &[&str]) -> String {
    let mut document = json!({
        "info": info,
        "sdfObject": {"switch.fault": {"sdfProperty": {"state": {
            "description": "The status of the switch.", "type": "string", "enum": states
        }}}}
    });
    if info.is_null() {
        document.as_object_mut().unwrap().remove("info");
    }
    serde_json::to_string_pretty(&document).unwrap()
}
fn versioned(version: &str) -> Value {
    json!({"title": "Fault Interrupter Switch", "version": version})
}
fn earlier_switch() -> String {
    switch(versioned("2019-11-14"), &["on", "off", "faulted"])
}
fn compare<'a>(
    earlier: &'a esdieff::Catalogue,
    later: &'a esdieff::Catalogue,
) -> RevisionComparison<'a> {
    ComparisonRequest::new(earlier, later).compare().unwrap()
}
fn wire(comparison: &RevisionComparison<'_>) -> Value {
    serde_json::from_str(&comparison.to_json()).unwrap()
}
/// Each warning as its later document key, earlier version, later version and breaking count.
fn warnings(comparison: &RevisionComparison<'_>) -> Vec<(String, String, Option<String>, u32)> {
    let later = comparison.catalogue(Revision::Later);
    comparison
        .version_warnings()
        .map(|warning| {
            let (_, document) = warning.documents();
            (
                later.source(document).unwrap().key().to_owned(),
                warning.earlier_version().to_owned(),
                warning.later_version().map(str::to_owned),
                warning.breaking(),
            )
        })
        .collect()
}
fn warning(
    key: &str,
    earlier: &str,
    later: Option<&str>,
    breaking: u32,
) -> (String, String, Option<String>, u32) {
    (
        key.to_owned(),
        earlier.to_owned(),
        later.map(str::to_owned),
        breaking,
    )
}

#[test]
fn a_breaking_revision_that_keeps_its_version_warns_once() {
    let earlier_text = earlier_switch();
    let later_text = switch(versioned("2019-11-14"), &["on", "off"]);
    let earlier = catalogue(&[("switch", &earlier_text)]);
    let later = catalogue(&[("switch", &later_text)]);
    let comparison = compare(&earlier, &later);
    assert_eq!(comparison.outcome(), Classification::Breaking);
    assert_eq!(comparison.disposition(), Disposition::Failed);
    assert_eq!(comparison.count(Classification::Breaking), 1);
    assert_eq!(
        warnings(&comparison),
        [warning("switch", "2019-11-14", Some("2019-11-14"), 1)]
    );
    let version = comparison.version_warnings().next().unwrap();
    assert_eq!(version.code(), CODE);
    assert_eq!(
        version
            .location(Revision::Later)
            .pointer(64)
            .require()
            .unwrap(),
        "/info/version"
    );
    assert_eq!(
        version.location(Revision::Later).source_id(),
        later.source_id("switch").unwrap()
    );
    assert_eq!(
        version
            .location(Revision::Earlier)
            .pointer(64)
            .require()
            .unwrap(),
        "/info/version"
    );
    assert_eq!(
        version.location(Revision::Earlier).raw_json(),
        "\"2019-11-14\""
    );

    let wire = wire(&comparison);
    assert_eq!(wire["version"], "1.5");
    assert_eq!(wire["outcome"]["warnings"], 1);
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(wire["outcome"]["disposition"], "failed");
    assert_eq!(wire["outcome"]["strict_warnings"], false);
    assert_eq!(wire["policy"]["strict_warnings"], false);
    assert_eq!(wire["diagnostics"]["retained"], 1);
    assert_eq!(wire["diagnostics"]["omitted"], 0);
    assert_eq!(wire["diagnostics"]["truncated"], false);
    let span = |text: &str, revision: &str| {
        let start = text.find("\"2019-11-14\"").unwrap();
        json!({"kind": "source", "source_id": 1, "pointer": "/info/version",
            "span": [start, start + 12], "revision": revision})
    };
    let condition = &wire["diagnostics"]["items"][0];
    assert_eq!(condition["code"], CODE);
    assert_eq!(condition["severity"], "warning");
    assert_eq!(
        condition["basis"],
        json!({"kind": "advice", "id": CODE, "citation": "RFC 9880 Section 3.1"})
    );
    assert_eq!(condition["primary"], span(&later_text, "later"));
    assert_eq!(
        condition["related"],
        json!([span(&earlier_text, "earlier")])
    );
    assert_eq!(
        condition["arguments"],
        json!({"earlier_version": "2019-11-14", "later_version": "2019-11-14",
            "breaking_differences": 1})
    );
    assert!(condition["message"].as_str().unwrap().contains("version"));
}

#[test]
fn an_acknowledged_break_still_warns_without_failing() {
    let earlier = catalogue(&[("switch", &earlier_switch())]);
    let later = catalogue(&[("switch", &switch(versioned("2019-11-14"), &["on", "off"]))]);
    let acknowledgement = DifferenceAcknowledgement::new(
        DifferenceKind::Changed,
        DifferenceSubject::Quality("enum"),
        Classification::Breaking,
    )
    .unwrap()
    .earlier("switch", "/sdfObject/switch.fault/sdfProperty/state/enum")
    .unwrap()
    .later("switch", "/sdfObject/switch.fault/sdfProperty/state/enum")
    .unwrap();
    let acknowledgements = DifferenceAcknowledgements::new(vec![acknowledgement]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    assert_eq!(comparison.acknowledged_count(Classification::Breaking), 1);
    assert_eq!(comparison.disposition(), Disposition::Passed);
    assert_eq!(
        warnings(&comparison),
        [warning("switch", "2019-11-14", Some("2019-11-14"), 1)]
    );
    let wire = wire(&comparison);
    assert_eq!(wire["outcome"]["disposition"], "passed");
    assert_eq!(wire["outcome"]["warnings"], 1);
    assert_eq!(wire["outcome"]["errors"], 0);
}

#[test]
fn a_missing_or_lower_later_version_warns_at_the_nearest_member() {
    let breaking = |info: Value| switch(info, &["on", "off"]);
    for (earlier_info, later_info, later_version, pointer) in [
        (versioned("1"), json!({"title": "Switch"}), None, "/info"),
        (versioned("1"), Value::Null, None, ""),
        (
            versioned("20200706"),
            versioned("2020-07-06"),
            Some("2020-07-06"),
            "/info/version",
        ),
        (versioned("b"), versioned("a"), Some("a"), "/info/version"),
    ] {
        let earlier = catalogue(&[(
            "switch",
            &switch(earlier_info.clone(), &["on", "off", "faulted"]),
        )]);
        let later = catalogue(&[("switch", &breaking(later_info))]);
        let comparison = compare(&earlier, &later);
        let expected = earlier_info["version"].as_str().unwrap();
        assert_eq!(
            warnings(&comparison),
            [warning("switch", expected, later_version, 1)],
            "{earlier_info} then {later_version:?}"
        );
        let version = comparison.version_warnings().next().unwrap();
        assert_eq!(
            version
                .location(Revision::Later)
                .pointer(64)
                .require()
                .unwrap(),
            pointer
        );
        let arguments = &wire(&comparison)["diagnostics"]["items"][0]["arguments"];
        assert_eq!(
            arguments.get("later_version").map(|v| v.as_str().unwrap()),
            later_version
        );
        assert_eq!(
            wire(&comparison)["diagnostics"]["items"][0]["primary"]["pointer"],
            pointer
        );
    }
}

#[test]
fn no_warning_without_a_breaking_change_to_a_versioned_document() {
    // The comparison's outcome and its warning count.
    let quiet = |earlier: &str, later: &str| {
        let earlier = catalogue(&[("switch", earlier)]);
        let later = catalogue(&[("switch", later)]);
        let comparison = compare(&earlier, &later);
        let wire = wire(&comparison);
        assert_eq!(
            wire["outcome"]["warnings"],
            comparison.version_warnings().len()
        );
        assert_eq!(
            wire["diagnostics"]["items"].as_array().unwrap().len(),
            comparison.version_warnings().len()
        );
        (comparison.outcome(), comparison.version_warnings().len())
    };
    use Classification::{Breaking, Compatible, Undetermined};
    // A higher version.
    assert_eq!(
        quiet(
            &earlier_switch(),
            &switch(versioned("2019-11-15"), &["on", "off"])
        ),
        (Breaking, 0)
    );
    // Compatible and undetermined differences only.
    let documented = revised(&earlier_switch(), |document| {
        document["sdfObject"]["switch.fault"]["sdfProperty"]["state"]["description"] =
            json!("Switch state");
    });
    assert_eq!(quiet(&earlier_switch(), &documented), (Compatible, 0));
    // Every enum value matches the added pattern, so the change stays unproven.
    let patterned = revised(&earlier_switch(), |document| {
        document["sdfObject"]["switch.fault"]["sdfProperty"]["state"]["pattern"] =
            json!("^[a-z]+$");
    });
    assert_eq!(quiet(&earlier_switch(), &patterned), (Undetermined, 0));
    // Neither revision has a version, or only the later one has.
    let unversioned = |states: &[&str]| switch(json!({"title": "Switch"}), states);
    assert_eq!(
        quiet(
            &unversioned(&["on", "off", "faulted"]),
            &unversioned(&["on", "off"])
        ),
        (Breaking, 0)
    );
    assert_eq!(
        quiet(
            &unversioned(&["on", "off", "faulted"]),
            &switch(versioned("1"), &["on", "off"])
        ),
        (Breaking, 0)
    );
    // A version that is not text in either revision.
    let numbered = |version: Value, states: &[&str]| switch(json!({"version": version}), states);
    assert_eq!(
        quiet(
            &numbered(json!(1), &["on", "off", "faulted"]),
            &numbered(json!(1), &["on", "off"])
        ),
        (Breaking, 0)
    );
    assert_eq!(
        quiet(
            &switch(versioned("1"), &["on", "off", "faulted"]),
            &numbered(json!(1), &["on", "off"])
        ),
        (Breaking, 0)
    );
}

#[test]
fn a_break_from_an_imported_document_stays_with_that_document() {
    let outlet = json!({"info": {"version": "1"},
        "namespace": {"meter": "https://example.test/meter"},
        "sdfObject": {"Outlet": {"sdfProperty": {"power": {"sdfRef": "meter:#/sdfData/Power"}}}}})
    .to_string();
    let meter = |minimum: u32| {
        json!({"info": {"version": "1"}, "namespace": {"meter": "https://example.test/meter"},
            "defaultNamespace": "meter",
            "sdfData": {"Power": {"type": "number", "minimum": minimum}}})
        .to_string()
    };
    let earlier = catalogue(&[("outlet", &outlet), ("meter", &meter(0))]);
    let later = catalogue(&[("outlet", &outlet), ("meter", &meter(10))]);
    let comparison = compare(&earlier, &later);
    assert_eq!(comparison.outcome(), Classification::Breaking);
    let breaking = comparison
        .differences()
        .find(|difference| difference.classification() == Classification::Breaking)
        .unwrap();
    let root = breaking.side(Revision::Earlier).unwrap().root_document();
    assert_eq!(earlier.source(root).unwrap().key(), "outlet");
    assert_eq!(comparison.version_warnings().len(), 0);
    assert_eq!(wire(&comparison)["outcome"]["warnings"], 0);
}

#[test]
fn directory_comparison_warns_only_for_the_breaking_pair() {
    let documented = revised(&earlier_switch(), |document| {
        document["sdfObject"]["switch.fault"]["description"] = json!("Fault switch");
    });
    let lamp = |states: &[&str]| {
        revised(&switch(versioned("3"), states), |document| {
            let object = document["sdfObject"]["switch.fault"].take();
            document["sdfObject"] = json!({"Lamp": object});
        })
    };
    let earlier = catalogue(&[
        ("a/switch", &earlier_switch()),
        ("b/lamp", &lamp(&["on", "off", "faulted"])),
    ]);
    let later = catalogue(&[("a/switch", &documented), ("b/lamp", &lamp(&["on", "off"]))]);
    let comparison = compare(&earlier, &later);
    assert_eq!(comparison.count(Classification::Breaking), 1);
    assert_eq!(
        warnings(&comparison),
        [warning("b/lamp", "3", Some("3"), 1)]
    );
    let wire = wire(&comparison);
    assert_eq!(wire["outcome"]["warnings"], 1);
    let lamp_id = later.source_id("b/lamp").unwrap().get();
    assert_eq!(
        wire["diagnostics"]["items"][0]["primary"]["source_id"],
        lamp_id
    );
}

#[test]
fn every_breaking_difference_of_the_pair_is_counted() {
    let earlier_text = revised(&earlier_switch(), |document| {
        document["sdfObject"]["switch.fault"]["sdfAction"] = json!({"reset": {}});
    });
    let later_text = switch(versioned("2019-11-14"), &["on", "off"]);
    let earlier = catalogue(&[("switch", &earlier_text)]);
    let later = catalogue(&[("switch", &later_text)]);
    let comparison = compare(&earlier, &later);
    assert_eq!(comparison.count(Classification::Breaking), 2);
    assert!(comparison.differences().any(|difference| {
        difference.subject() == DifferenceSubject::Definition(Class::Action)
            && difference.kind() == DifferenceKind::Removed
    }));
    assert_eq!(
        warnings(&comparison),
        [warning("switch", "2019-11-14", Some("2019-11-14"), 2)]
    );
}

#[test]
fn every_projection_shows_the_warning_without_changing_the_disposition() {
    let earlier = catalogue(&[("switch", &earlier_switch())]);
    let later = catalogue(&[("switch", &switch(versioned("2019-11-14"), &["on", "off"]))]);
    let comparison = compare(&earlier, &later);

    let human = comparison.to_human();
    assert!(human.contains(CODE), "{human}");
    assert!(human.contains("\"2019-11-14\""), "{human}");

    let markdown = comparison.to_markdown();
    assert!(markdown.contains("## Warnings"), "{markdown}");
    assert!(markdown.contains("`2019-11-14`"), "{markdown}");
    assert!(markdown.contains(&format!("`{CODE}`")), "{markdown}");
    assert!(
        markdown.starts_with("# Model changes\n\nDisposition: failed,"),
        "{markdown}"
    );

    let sarif: Value =
        serde_json::from_str(&comparison.to_sarif(std::path::Path::new("."))).unwrap();
    let run = &sarif["runs"][0];
    assert_eq!(run["properties"]["disposition"], "failed");
    assert_eq!(run["properties"]["omittedResults"], 0);
    let results = run["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    let warning = results
        .iter()
        .find(|result| result["ruleId"] == CODE)
        .unwrap();
    assert_eq!(warning["level"], "warning");
    assert_eq!(warning["suppressions"], json!([]));
    let logical = &warning["locations"][0]["logicalLocations"][0];
    assert_eq!(logical["fullyQualifiedName"], "/info/version");
    assert_eq!(logical["properties"]["view"], "source");
    assert_eq!(logical["properties"]["revision"], "later");
    let related = &warning["relatedLocations"][0];
    assert_eq!(
        related["logicalLocations"][0]["properties"]["revision"],
        "earlier"
    );
    assert!(
        warning["message"]["text"]
            .as_str()
            .unwrap()
            .contains("2019-11-14")
    );
}

#[test]
fn warning_details_that_do_not_fit_are_counted() {
    // Both copies of a 600,000-byte version exceed the 1 MiB detail allowance.
    let version = format!("v{}", "1".repeat(600_000));
    let earlier = catalogue(&[(
        "switch",
        &switch(versioned(&version), &["on", "off", "faulted"]),
    )]);
    let later = catalogue(&[("switch", &switch(versioned(&version), &["on", "off"]))]);
    let comparison = compare(&earlier, &later);
    assert_eq!(comparison.version_warnings().len(), 1);
    let wire = wire(&comparison);
    assert_eq!(wire["outcome"]["warnings"], 1);
    assert_eq!(wire["outcome"]["disposition"], "failed");
    let diagnostics = &wire["diagnostics"];
    assert_eq!(diagnostics["retained"], 0);
    assert_eq!(diagnostics["omitted"], 1);
    assert_eq!(diagnostics["truncated"], true);
    assert_eq!(diagnostics["reasons"], json!(["byte_limit"]));
    assert_eq!(diagnostics["items"], json!([]));
    // The breaking difference still fits after the warning that did not.
    assert_eq!(wire["result"]["differences"].as_array().unwrap().len(), 1);
    assert!(comparison.to_human().contains("Warning details omitted: 1"));
}

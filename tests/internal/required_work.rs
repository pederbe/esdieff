//! Requested relationships share the original inspection counters and mandatory envelope.
//! Follows ADR0026: Required declaration relationships and source evidence.
use esdieff::{
    Catalogue, Field, InspectRequest, Limits, RequiredDeclarationsState, SourceInput, View,
};
const SIMPLE: &[u8] = br#"{"info":{},"sdfObject":{"O":{"sdfRequired":[true]}}}"#;

fn load(text: &[u8], limits: Limits) -> Catalogue {
    Catalogue::with_limits(&[SourceInput::supplied("model", text)], limits).unwrap()
}

#[test]
fn source_record_nodes_are_cumulative_and_observed_target_count_survives_retention_failure() {
    for nodes in [1, 2] {
        let catalogue = load(SIMPLE, Limits::default().effective_nodes(nodes));
        let id = catalogue.source_id("model").unwrap();
        assert!(
            catalogue
                .inspect(InspectRequest::new(id, "/sdfObject/O"))
                .unwrap()
                .outcome()
                .complete()
        );
        let report = catalogue
            .inspect(InspectRequest::new(id, "/sdfObject/O").required_declarations(true))
            .unwrap();
        let collection = report.required_declarations().require().unwrap();
        assert!(!collection.complete());
        assert_eq!(collection.state(), RequiredDeclarationsState::Partial);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let summary = &wire["result"]["required_declarations"];
        assert_eq!(summary["owners_known"], 1);
        assert_eq!(summary["owners_retained"], 1);
        assert_eq!(summary["references_known"], 1);
        assert_eq!(summary["references_retained"], 0);
        assert_eq!(summary["targets_known"], u32::from(nodes == 2));
        assert_eq!(summary["targets_retained"], 0);
        assert_eq!(report.outcome().incomplete_checks(), 1);
    }
}

#[test]
fn primary_composition_does_not_reset_node_or_reference_allowance_for_relationships() {
    let minimum_nodes = (1..64)
        .find(|nodes| {
            let catalogue = load(SIMPLE, Limits::default().effective_nodes(*nodes));
            catalogue
                .inspect(
                    InspectRequest::new(catalogue.source_id("model").unwrap(), "/sdfObject/O")
                        .view(View::Effective),
                )
                .unwrap()
                .outcome()
                .complete()
        })
        .unwrap();
    let catalogue = load(SIMPLE, Limits::default().effective_nodes(minimum_nodes));
    let id = catalogue.source_id("model").unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(id, "/sdfObject/O")
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    assert!(!report.outcome().complete());
    assert_eq!(
        report.required_declarations().require().unwrap().state(),
        RequiredDeclarationsState::Unavailable
    );
    let text =
        br##"{"info":{},"sdfObject":{"B":{"sdfRequired":[true]},"C":{"sdfRef":"#/sdfObject/B"}}}"##;
    let catalogue = load(text, Limits::default().reference_applications(1));
    let id = catalogue.source_id("model").unwrap();
    let plain = catalogue
        .inspect(InspectRequest::new(id, "/sdfObject/C").view(View::Effective))
        .unwrap();
    assert!(plain.outcome().complete());
    let report = catalogue
        .inspect(
            InspectRequest::new(id, "/sdfObject/C")
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    assert!(!report.outcome().complete());
    assert_eq!(
        report.required_declarations().require().unwrap().state(),
        RequiredDeclarationsState::Partial
    );
}

#[test]
fn semantic_storage_exhaustion_keeps_an_unavailable_summary_and_source_capability_separate() {
    let catalogue = load(SIMPLE, Limits::default().effective_allocation_bytes(1));
    let id = catalogue.source_id("model").unwrap();
    assert!(matches!(
        catalogue.source(id).unwrap().json(),
        Field::Present(_)
    ));
    for view in [View::Source, View::Effective] {
        let report = catalogue
            .inspect(
                InspectRequest::new(id, "/sdfObject/O")
                    .view(view)
                    .required_declarations(true),
            )
            .unwrap();
        let collection = report.required_declarations().require().unwrap();
        assert!(!collection.complete());
        assert_eq!(collection.state(), RequiredDeclarationsState::Unavailable);
        assert_eq!(collection.owners().count(), 0);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["result"]["required_declarations"]["state"],
            "unavailable"
        );
        assert_eq!(wire["result"]["required_declarations"]["complete"], false);
        assert!(
            report
                .to_human()
                .contains("Required declarations: unavailable")
        );
    }
}

#[test]
fn finite_original_work_limits_relationship_traversal_after_successful_source_decoding() {
    let text = format!(
        "{{\"info\":{{}},\"sdfObject\":{{\"O\":{{\"sdfRequired\":[{}true]}}}}}}",
        "true,".repeat(100)
    );
    let mut found = false;
    for work in [512, 1024, 2048, 4096, 8192, 16384] {
        let catalogue = load(text.as_bytes(), Limits::default().decoding_work(work));
        let id = catalogue.source_id("model").unwrap();
        if !matches!(catalogue.source(id).unwrap().json(), Field::Present(_)) {
            continue;
        }
        let report = catalogue
            .inspect(InspectRequest::new(id, "/sdfObject/O").required_declarations(true))
            .unwrap();
        if !report.required_declarations().require().unwrap().complete() {
            found = true;
            assert_eq!(report.outcome().incomplete_checks(), 1);
        }
    }
    assert!(
        found,
        "finite original work must reach a decoded but incomplete relationship context"
    );
}

#[test]
fn first_owner_failure_omits_exact_suffix_and_keeps_one_global_reason() {
    let text = br#"{"info":{},"sdfObject":{"O":{"sdfRequired":[true],"sdfProperty":{"p":{}}}}}"#;
    for (limits, shown, reason) in [
        (Limits::default().diagnostic_bytes(5000), 0, "byte_limit"),
        (Limits::default().conditions(1), 1, "count_limit"),
    ] {
        let catalogue = load(text, limits);
        let report = catalogue
            .inspect(
                InspectRequest::new(catalogue.source_id("model").unwrap(), "/sdfObject/O")
                    .required_declarations(true),
            )
            .unwrap();
        assert!(report.outcome().complete());
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let summary = &wire["result"]["required_declarations"];
        assert_eq!(summary["owners"].as_array().unwrap().len(), shown);
        assert_eq!(summary["owners_omitted"], 2 - shown);
        assert_eq!(summary["omissions"].as_array().unwrap().len(), 1);
        assert_eq!(summary["omissions"][0]["reason"], reason);
        assert_eq!(summary["omissions"][0]["omitted_count"], 2 - shown);
        assert_eq!(wire["diagnostics"]["truncated"], true);
        assert_eq!(
            wire["diagnostics"]["reasons"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r.as_str() == Some(reason))
                .count(),
            1
        );
    }
}

#[test]
fn required_detail_pointer_omission_keeps_identity_completion_and_truthful_truncation() {
    let catalogue = load(SIMPLE, Limits::default().pointer_bytes(1));
    let id = catalogue.source_id("model").unwrap();
    for view in [View::Source, View::Effective] {
        let report = catalogue
            .inspect(
                InspectRequest::new(id, "/sdfObject/O")
                    .view(view)
                    .required_declarations(true),
            )
            .unwrap();
        assert!(report.outcome().complete());
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let owner = &wire["result"]["required_declarations"]["owners"][0];
        assert_eq!(owner["source_owner"]["source_id"], id.get());
        assert!(owner["source_owner"]["pointer"].is_null());
        let omission = &owner["source_owner"]["omission"];
        assert_eq!(omission["reason"], "location_limit");
        assert_eq!(omission["omitted_count"], 1);
        assert_eq!(
            omission["field"],
            "result.required_declarations.owners.source_owner.pointer"
        );
        assert_eq!(omission["source_id"], id.get());
        assert!(owner["source_owner"]["span"].is_array());
        assert!(serde_json::to_string(&owner["source_owner"]).unwrap().len() <= 512);
        if view == View::Effective {
            assert!(owner["effective_owner"]["pointer"].is_null());
            assert_eq!(
                owner["effective_owner"]["omission"]["reason"],
                "location_limit"
            );
            assert_eq!(owner["effective_owner"]["omission"]["omitted_count"], 1);
            assert_eq!(
                owner["effective_owner"]["omission"]["field"],
                "result.required_declarations.owners.effective_owner.pointer"
            );
            assert_eq!(owner["effective_owner"]["root"], id.get());
            assert!(
                serde_json::to_string(&owner["effective_owner"])
                    .unwrap()
                    .len()
                    <= 512
            );
        }
        assert_eq!(wire["diagnostics"]["truncated"], true);
        assert!(
            wire["diagnostics"]["reasons"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("location_limit"))
        );
    }
}

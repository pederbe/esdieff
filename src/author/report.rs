//! Author report 1.1 projection shares the common envelope and assessment meanings.
use super::*;
use crate::document_request::{RequestedChecking, RequestedCheckingView};
use crate::human::{HumanStyle, HumanText, Role};
use crate::report::projection::{DiagnosticProjection, Projection};
use serde_json::{Value, json};

pub(super) struct Checking {
    wire: Value,
    requested: RequestedChecking,
    reference_errors: u32,
    projection: Option<Projection>,
}
/// Requested checking stays separate from generation and Authoring fidelity.
pub struct AuthorChecking<'a>(RequestedCheckingView<'a>);
impl<'a> AuthorChecking<'a> {
    pub(super) fn new(value: Option<&'a Checking>) -> Self {
        Self(RequestedCheckingView::new(
            value.map(|check| &check.requested),
        ))
    }
    pub fn assessment(&self) -> Option<Assessment> {
        self.0.assessment()
    }
    pub fn outcome(&self) -> Option<&'a Outcome> {
        self.0.outcome()
    }
    pub fn requested_coverage(&self) -> Option<&'a crate::RequestedCoverage> {
        self.0.requested_coverage()
    }
    pub fn requirement_coverage(&self) -> crate::Field<&'a crate::RequestedCoverage> {
        self.0.requirement_coverage()
    }
    pub fn requested_source_ids(&self) -> Option<&'a [SourceId]> {
        self.0.requested_source_ids()
    }
}
impl Checking {
    pub(super) fn from_report(
        report: crate::Report<'_>,
        conditions: &mut AuthorConditions,
        known_reference_errors: u32,
    ) -> Self {
        let (wire, projection) = report.author_projection();
        let reference_errors = report.author_reference_errors();
        let mut checking = Self {
            wire,
            projection: Some(projection),
            requested: RequestedChecking::from_report(&report),
            reference_errors,
        };
        if reference_errors > 0 {
            let mut references =
                if reference_errors != known_reference_errors && !conditions.retained.is_empty() {
                    report.into_author_references(
                        conditions.retained.len(),
                        checking.projection.as_mut().expect("checking projection"),
                    )
                } else {
                    None
                };
            conditions.deduplicate_references(
                references.as_mut(),
                reference_errors,
                known_reference_errors,
            );
        }
        checking
    }

    pub(super) fn unavailable(assessment: Assessment, source: SourceId, strict: bool) -> Self {
        Self {
            wire: RequestedChecking::unavailable_wire(assessment, source, strict),
            requested: RequestedChecking::unavailable(assessment, source, strict),
            reference_errors: 0,
            projection: None,
        }
    }
    pub(super) fn complete(&self) -> bool {
        self.requested.outcome().complete()
    }
    pub(super) fn errors(&self) -> u32 {
        self.requested.outcome().errors()
    }
    pub(super) fn warnings(&self) -> u32 {
        self.requested.outcome().warnings()
    }
    pub(super) fn notices(&self) -> u32 {
        self.requested.outcome().notices()
    }
    pub(super) fn reference_errors(&self) -> u32 {
        self.reference_errors
    }
}
fn outcome(outcome: &Outcome) -> Value {
    json!({"complete":outcome.complete(),"errors":outcome.errors(),"warnings":outcome.warnings(),"notices":outcome.notices(),"incomplete_checks":outcome.incomplete_checks(),"strict_warnings":outcome.strict_warnings(),"disposition":outcome.disposition().as_str()})
}
fn wire(result: &AuthorResult) -> Value {
    let cat = result.catalogue();
    let mut value = crate::report::author_envelope(cat, result.generated, result.request.strict);
    value["version"] = json!("1.1");
    value["operation"] = json!("author");
    let mut scope = value["request"]["scope"].take();
    let checking = match &result.checking {
        Some(check) => {
            scope = check.wire["request"]["scope"].clone();
            if check.wire["baseline"].is_object() {
                value["baseline"] = check.wire["baseline"].clone();
            }
            if check.wire["policy"].is_object() {
                value["policy"] = check.wire["policy"].clone();
            }
            json!({"state":"requested","assessment":check.wire["request"]["assessment"],"inventory":check.wire["request"]["inventory"],"scope":scope,"outcome":check.wire["outcome"],"coverage":check.wire["coverage"]})
        }
        None => json!({"state":"not_requested"}),
    };
    for (field, known) in [
        ("dependencies", &result.scope.dependencies),
        ("assessed", &result.scope.assessed),
        ("unavailable_required", &result.scope.unavailable),
    ] {
        let ids = scope[field].as_array_mut().expect("scope IDs");
        for (id, present) in known.iter().enumerate().skip(1) {
            let id = json!(id);
            if *present && !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let mut summary = json!({"key":result.request.key,"thing_name":result.request.thing,"generated_source":result.generated.get(),"declaration_count":result.request.objects.len(),"checking":if result.checking.is_some() {"requested"}else{"not_requested"}});
    if let Some(label) = &result.request.label {
        summary["label"] = json!(label);
    }
    value["request"] =
        json!({"assessment":null,"inventory":null,"scope":scope,"authoring":summary});
    value["outcome"] = outcome(&result.outcome);
    value["coverage"]["requested"] = json!({"inventory":null,"count_unit":"authoring","total":1,"assessed":u32::from(result.outcome.complete()),"not_applicable":0,"unassessed":0,"undetermined":u32::from(!result.outcome.complete())});
    value["coverage"]["rows"] = json!([]);
    value["coverage"]["rows_omitted"] = json!(0);
    value["coverage"]["omissions"] = json!([]);
    let copied_items = result
        .checking
        .as_ref()
        .and_then(|check| check.wire["diagnostics"]["items"].as_array())
        .cloned()
        .unwrap_or_default();
    let mut diagnostics = diagnostic_projection(result);
    for mut condition in copied_items {
        relate_request_origins(result, &mut condition, &mut diagnostics);
        diagnostics.push(condition);
    }
    for condition in result.conditions.iter() {
        if !diagnostics.allow_item(cat.limits.conditions) {
            continue;
        }
        let pointer_bytes = condition.affected.as_ref().map_or(32, |location| {
            location.effective_pointer.as_ref().map_or(0, String::len)
                + location.source_pointer.as_ref().map_or(0, String::len)
        });
        // Admit the condition, cloned pointers, inline omissions and serializer growth first.
        let wire = 2048 + result.request.key.len() * 6 + pointer_bytes * 6;
        let Some(charge) = diagnostics.admit_item(wire, 32768 + wire * 8) else {
            continue;
        };
        let pointer = format!("/objects/{}", condition.object);
        let span = result
            .request
            .plan
            .as_ref()
            .and_then(|plan| plan.json().present())
            .and_then(|root| root.get("objects").present())
            .and_then(|objects| objects.at(condition.object).present())
            .map(|object| {
                let span = object.span();
                [span.start, span.end]
            });
        let mut item = json!({"code":condition.code,"severity":if condition.error {"error"}else{"incomplete"},"basis":{"kind":"operational","id":"reference-preserving-authoring/1"},"message":condition.message,
            "primary":{"kind":"author_request","key":result.request.key,"pointer":pointer,"span":span},"related":[],"arguments":{},"domain":"operation","assessment_unit":"reference-preserving-authoring/1"});
        if condition.error && condition.code.starts_with("reference.") {
            item["basis"] =
                json!({"kind":"rfc","citation":"RFC 9880 Section 4.3; W3C CURIE Section 3"});
        }
        if let Some(location) = &condition.affected {
            item["primary"] = json!({"kind":"effective","root":location.root.get(),"pointer":location.effective_pointer});
            item["related"] = json!([{"kind":"source","source_id":location.source.get(),"pointer":location.source_pointer,"span":location.span}]);
        }
        present_author_locations(result, &mut item, &mut diagnostics);
        let cost = serde_json::to_string(&item)
            .expect("bounded condition")
            .len()
            + 1;
        assert!(cost <= wire, "author condition exceeds its precharge");
        diagnostics.settle(charge, cost);
        relate_request_origins(result, &mut item, &mut diagnostics);
        diagnostics.push(item);
    }
    value["diagnostics"] = diagnostics.finish();
    let bytes = result.generated_document().map_or(0, <[u8]>::len);
    let complete = result.generation == Generation::Complete;
    value["result"] = json!({"kind":"author","generation":result.generation.as_str(),
        "fidelity":{"state":result.fidelity.as_str(),"known_semantics":if result.known_fidelity {"established"}else{"undetermined"},"opaque_content":if result.opaque {"retained"}else{"none"},"limitation_count":result.conditions.limitations},
        "documents":[{"source_id":result.generated.get(),"state":if bytes==0 {"unavailable"}else if complete {"available"}else{"partial"},"content_complete":bytes>0,"retained_bytes":bytes,"faithful":complete && result.fidelity == Fidelity::Established}],"checking":checking,"omissions":[]});
    value
}
pub(super) fn to_json(result: &AuthorResult) -> String {
    crate::report::projection::encode_author(wire(result), result.catalogue().limits.report_bytes)
}
pub(super) fn to_human(result: &AuthorResult, style: HumanStyle) -> String {
    let value: Value = serde_json::from_str(&to_json(result)).expect("author report");
    let mut text = HumanText::new(style);
    text.heading("Author");
    text.push_str(&format!(" {}: ", result.request.thing));
    text.span(
        Role::disposition(result.outcome.disposition()),
        result.outcome.disposition().as_str(),
    );
    text.push('\n');
    text.heading("Generation:");
    text.push(' ');
    text.span(
        match result.generation {
            Generation::Complete => Role::Passed,
            Generation::Partial | Generation::Unavailable => Role::Incomplete,
        },
        result.generation.as_str(),
    );
    text.push('\n');
    text.heading("Fidelity:");
    text.push(' ');
    text.span(
        match result.fidelity {
            Fidelity::Established => Role::Passed,
            Fidelity::Undetermined => Role::Incomplete,
        },
        result.fidelity.as_str(),
    );
    text.push('\n');
    text.heading("Broader RFC coverage:");
    text.push_str(" partial\n");
    text.push_str(&format!(
        "Errors: {}; warnings: {}; notices: {}; unfinished authoring units: {}\n",
        result.outcome.errors(),
        result.outcome.warnings(),
        result.outcome.notices(),
        result.outcome.incomplete_checks()
    ));
    text.heading("Baseline:");
    text.push_str(" rfc9880/base-1\n");
    text.heading("Selected generated root:");
    text.push(' ');
    text.push_str(&result.generated.get().to_string());
    text.push('\n');
    for (field, label) in [
        ("dependencies", "Dependency sources"),
        ("assessed", "Sources with attempted work"),
        ("unavailable_required", "Unavailable required sources"),
    ] {
        let ids = value["request"]["scope"][field]
            .as_array()
            .expect("scope IDs");
        text.heading(label);
        text.push_str(&format!(
            ": {}\n",
            if ids.is_empty() {
                "none".to_owned()
            } else {
                ids.iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
    }
    for source in result.catalogue().sources() {
        text.heading("Source");
        text.push_str(&format!(
            " {} {}: {:?}, {:?}\n",
            source.id().get(),
            serde_json::to_string(source.key()).expect("source key"),
            source.role(),
            source.acquisition()
        ));
    }
    if let Some(check) = &result.checking {
        text.heading("Requested checking");
        text.push_str(&format!(
            " {}: ",
            check.wire["request"]["assessment"]
                .as_str()
                .unwrap_or("unavailable"),
        ));
        let outcome = check.requested.outcome();
        text.span(
            Role::disposition(outcome.disposition()),
            outcome.disposition().as_str(),
        );
        text.push_str(&format!(
            " ({} incomplete assessment units)\n",
            outcome.incomplete_checks()
        ));
        let coverage = check.requested.coverage();
        text.heading("Requested check inventory:");
        text.push_str(&format!(
            " {}\n",
            coverage.inventory().unwrap_or("unavailable")
        ));
        text.heading("Requested check coverage");
        text.push_str(&format!(
            " ({}): total {}; assessed {}; not applicable {}; unassessed {}; undetermined {}\n",
            coverage.count_unit(),
            coverage
                .total()
                .map_or_else(|| "unavailable".to_owned(), |total| total.to_string()),
            coverage.assessed(),
            coverage.not_applicable(),
            coverage.unassessed(),
            coverage.undetermined()
        ));
        if let Some(requirements) = check.requested.requirements() {
            text.heading("Requirement coverage:");
            text.push_str(&format!(
                " total {}; assessed {}; not applicable {}; unassessed {}; undetermined {}\n",
                requirements.total().expect("full requirement inventory"),
                requirements.assessed(),
                requirements.not_applicable(),
                requirements.unassessed(),
                requirements.undetermined()
            ));
        }
    } else {
        text.heading("Checking:");
        text.push_str(" not requested\n");
    }
    for condition in value["diagnostics"]["items"].as_array().expect("items") {
        let severity = condition["severity"].as_str().expect("severity");
        text.span(
            Role::severity(crate::Severity::from_report(severity)),
            severity,
        );
        text.push_str(&format!(
            " {}: {}\n",
            condition["code"].as_str().expect("code"),
            condition["message"].as_str().expect("message")
        ));
        for related in condition["related"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|location| location["kind"] == "author_request")
        {
            text.push_str(&format!(
                "  Author request {} at {}",
                related["key"].as_str().expect("request key"),
                related["pointer"].as_str().unwrap_or("unavailable")
            ));
            if let Some(span) = related["span"].as_array() {
                text.push_str(&format!(" bytes {}..{}", span[0], span[1]));
            }
            text.push('\n');
        }
    }
    if value["diagnostics"]["truncated"] == true {
        text.heading("Diagnostic details omitted:");
        text.push_str(&format!(" {}\n", value["diagnostics"]["omitted"]));
    }
    text.finish(result.catalogue().limits.report_bytes)
}
fn diagnostic_projection(result: &AuthorResult) -> DiagnosticProjection {
    let copied = result
        .checking
        .as_ref()
        .map(|check| &check.wire["diagnostics"]);
    let allowance = result
        .checking
        .as_ref()
        .and_then(|check| check.projection.clone())
        .unwrap_or_else(|| Projection::new(&result.catalogue().limits, result.catalogue().reserve));
    let mut projection = DiagnosticProjection::new(copied, allowance);
    projection.add_omitted(u64::from(result.conditions.omitted));
    for (count, reason) in [
        (result.conditions.count_omitted, "count_limit"),
        (result.conditions.bytes_omitted, "byte_limit"),
        (
            result.conditions.mixed_reference_omitted,
            "byte_or_count_limit",
        ),
        (result.conditions.presentation_omitted, "presentation_limit"),
    ] {
        projection.record("diagnostics.items", reason, u64::from(count));
    }
    projection
}

fn present_author_locations(
    result: &AuthorResult,
    condition: &mut Value,
    projection: &mut DiagnosticProjection,
) {
    fn present(
        result: &AuthorResult,
        location: &mut Value,
        field: &'static str,
        projection: &mut DiagnosticProjection,
    ) {
        let available = location["pointer"].as_str().is_some_and(|pointer| {
            projection.present_pointer(pointer, result.catalogue().limits.pointer_bytes)
        });
        if !available {
            location["pointer"] = Value::Null;
            let mut omission = json!({"field":field,"reason":"location_limit","omitted_count":1});
            if let Some(source) = location.get("source_id").or_else(|| location.get("root")) {
                omission["source_id"] = source.clone();
            }
            location["omission"] = omission;
            projection.record(field, "location_limit", 1);
        }
    }
    present(
        result,
        &mut condition["primary"],
        "diagnostics.items.primary.pointer",
        projection,
    );
    for related in condition["related"]
        .as_array_mut()
        .expect("related locations")
    {
        present(
            result,
            related,
            "diagnostics.items.related.pointer",
            projection,
        );
    }
}

fn relate_request_origins(
    result: &AuthorResult,
    condition: &mut Value,
    projection: &mut DiagnosticProjection,
) {
    let mut locations = Vec::new();
    let existing = condition["related"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for location in std::iter::once(&condition["primary"]).chain(existing) {
        if location["kind"] != "source"
            || location["source_id"].as_u64() != Some(u64::from(result.generated.get()))
        {
            continue;
        }
        let Some(pointer) = location["pointer"].as_str() else {
            projection.record("diagnostics.related.author_request", "location_limit", 1);
            continue;
        };
        // The logical pointer cannot exceed the generated path plus its fixed recipe prefix.
        let wire = 256 + result.request.key.len() * 6 + (pointer.len() + 64) * 6;
        let Some(charge) = projection.admit(wire, 8192 + wire * 8) else {
            projection.record("diagnostics.related.author_request", "byte_limit", 1);
            continue;
        };
        let origin = projection.lookup(|work, pointer_bytes| {
            if !crate::allowance::charge(work, pointer.len() * 8 + 1) {
                return crate::Field::Unavailable("author request origin work limit");
            }
            let Some(root) = result
                .catalogue()
                .source(result.generated)
                .and_then(|source| source.json().present())
            else {
                return crate::Field::Unavailable("generated source unavailable");
            };
            let Ok(Some(source)) = crate::report::select_with_budget(root, pointer, work) else {
                return crate::Field::Unavailable("author request origin work limit");
            };
            result.request_origin_with_budget(source, work, pointer_bytes)
        });
        let origin = match origin {
            crate::Field::Present(origin) => origin,
            crate::Field::Absent => {
                projection.settle(charge, 0);
                continue;
            }
            crate::Field::Unavailable(reason) => {
                projection.settle(charge, 0);
                projection.record(
                    "diagnostics.related.author_request",
                    if reason.contains("limit") {
                        "location_limit"
                    } else {
                        "not_available"
                    },
                    1,
                );
                continue;
            }
        };
        let pointer = origin.pointer().present().expect("owned logical pointer");
        let span = origin.span().map(|span| [span.start, span.end]);
        let location =
            json!({"kind":"author_request","key":origin.key(),"pointer":pointer,"span":span});
        if !locations.contains(&location) && !existing.contains(&location) {
            let cost = serde_json::to_string(&location)
                .expect("bounded request location")
                .len()
                + 1;
            assert!(cost <= wire, "author request location exceeds precharge");
            projection.settle(charge, cost);
            locations.push(location);
        } else {
            projection.settle(charge, 0);
        }
    }
    if let Some(related) = condition["related"].as_array_mut() {
        related.extend(locations);
    }
}
pub(super) fn diagnostics(result: &AuthorResult) -> crate::Diagnostics {
    crate::report::author_diagnostics(wire(result)["diagnostics"].take())
}

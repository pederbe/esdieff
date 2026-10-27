//! The `diff` report 1.5 keeps every source, count, the outcome and the disposition when
//! difference or warning details are omitted, following ADR 0009: Bounded processing and
//! reporting, ADR 0011: Machine-readable report contracts, ADR 0040: Acknowledged revision
//! differences and ADR 0044: Versions of breaking revisions. The human report renders the same
//! wire value.
use super::{
    COMPARISON_CONTRACT, Classification, DifferenceSubject, Record, Revision, RevisionComparison,
    Side,
};
use crate::human::{HumanText, Role};
use crate::json::{PointerBudget, PointerError};
use serde_json::{Value, json};

const REVISIONS: [Revision; 2] = [Revision::Earlier, Revision::Later];
const ORDER: [Classification; 3] = [
    Classification::Breaking,
    Classification::Undetermined,
    Classification::Compatible,
];

/// Render a side's effective pointer, appending the absent member it names.
pub(super) fn render_pointer(
    comparison: &RevisionComparison<'_>,
    side: &Side<'_>,
    cap: usize,
    work: &mut PointerBudget,
) -> Result<String, PointerError> {
    let Some(node) = side.node else {
        return Ok(String::new());
    };
    let location = comparison.json(side, node).location();
    let size = location.pointer_size(cap, work)?;
    let trailing = side.trailing.map_or(0, |key| {
        1 + key.len() + key.bytes().filter(|b| matches!(b, b'~' | b'/')).count()
    });
    if size.bytes + trailing > cap {
        return Err(PointerError::Limit);
    }
    work.debit(size.render_work() + 2 * trailing)?;
    let mut pointer = location.render_pointer(size);
    if let Some(key) = side.trailing {
        pointer.push('/');
        crate::json::push_pointer_token(&mut pointer, key);
    }
    Ok(pointer)
}

pub(super) fn to_json(comparison: &RevisionComparison<'_>) -> String {
    encode(
        wire(comparison),
        comparison.catalogues[0].limits.report_bytes,
    )
}

fn wire(comparison: &RevisionComparison<'_>) -> Value {
    let limits = &comparison.catalogues[0].limits;
    let reserve = super::reserve(comparison.catalogues, comparison.acknowledgements);
    let mut omissions = Vec::new();
    let mut sources = Vec::new();
    let mut scope = serde_json::Map::new();
    for revision in REVISIONS {
        let catalogue = comparison.catalogue(revision);
        for source in catalogue.retained_sources() {
            let (mut row, omission) = crate::report::source_wire(source);
            row["revision"] = json!(revision.as_str());
            sources.push(row);
            omissions.extend(omission.map(|mut omission| {
                omission["revision"] = json!(revision.as_str());
                omission
            }));
        }
        let models: Vec<_> = catalogue
            .sources()
            .filter(|source| source.role.is_model())
            .collect();
        scope.insert(
            revision.as_str().to_owned(),
            json!({"kind":"catalogue","roots":[],"dependencies":[],
                "assessed":models.iter().map(|source| source.id.get()).collect::<Vec<_>>(),
                "unavailable_required":models.iter().filter(|source| source.json().present().is_none()).map(|source| source.id.get()).collect::<Vec<_>>()}),
        );
    }
    let outcome = comparison.outcome();
    let breaking = comparison.count(Classification::Breaking);
    let undetermined = comparison.count(Classification::Undetermined);
    let acknowledged = |classification| comparison.acknowledged_count(classification);
    // Only unacknowledged differences decide the outcome fields and the disposition.
    let errors = breaking - acknowledged(Classification::Breaking);
    let complete = comparison.complete();
    // The content-format meaning rules and the value interpretations of witness checks, as
    // `check` names them, then the comparison's own.
    let mut interpretations: Vec<_> = comparison.registry.interpretation_ids().collect();
    interpretations.extend(comparison.interpretations.ids());
    if comparison.relocation {
        interpretations.push("copied-reference-relocation/1");
    }
    if comparison.undeclared {
        interpretations.push("undeclared-extensions-ignorable/1");
    }

    // Version warnings, at most one per compared document pair, then differences are optional
    // detail within the allowance the mandatory envelope leaves, most important first.
    // Difference location omissions are counted once per field and reason.
    let mut remaining = limits.detail_bytes.min(limits.report_bytes - reserve);
    let mut items = Vec::new();
    for warning in &comparison.warnings {
        let value = warning.wire();
        if !fits(&value, &mut remaining) {
            break;
        }
        items.push(value);
    }
    let warnings = comparison.warnings.len();
    let warnings_omitted = warnings - items.len();
    let mut pointers = PointerBudget::new(limits.pointer_total);
    let mut differences = Vec::new();
    for record in &comparison.records {
        let mut limited = Vec::new();
        let value = difference(
            comparison,
            record,
            &mut pointers,
            limits.pointer_bytes,
            &mut limited,
        );
        if !fits(&value, &mut remaining) {
            break;
        }
        differences.push(value);
        for (field, reason) in limited {
            note(&mut omissions, field, reason, 1);
        }
    }
    let omitted = comparison.records.len() - differences.len() + comparison.unretained as usize;
    let mut result_omissions = Vec::new();
    if omitted > 0 {
        note(
            &mut result_omissions,
            "result.differences",
            "byte_limit",
            omitted,
        );
        note(&mut omissions, "result.differences", "byte_limit", omitted);
    }
    let mut reasons: Vec<&str> = Vec::new();
    if warnings_omitted > 0 {
        reasons.push("byte_limit");
    }
    for omission in &omissions {
        let reason = omission["reason"].as_str().expect("omission reason");
        if !reasons.contains(&reason) {
            reasons.push(reason);
        }
    }
    json!({
        "schema":"esdieff.report","version":"1.5","operation":"diff",
        "request":{"assessment":null,"inventory":null,"comparison":COMPARISON_CONTRACT,"scope":scope},
        "sources":sources,
        // At most two fixed unit and three fixed content-format registry records, well within
        // both Catalogues' fixed reserves.
        "baseline":{"id":"rfc9880/base-1","rfc":"9880","corrections":[],"interpretations":interpretations,"deviations":[],"registry_knowledge":comparison.registry.wire(false)},
        "policy":{"restrictions":["A002"],"strict_warnings":false,"recorded_registry":comparison.registry.policy.as_str(),
            "acknowledgements":comparison.acknowledgements.map_or(0, |supplied| supplied.entries().len())},
        "outcome":{"complete":complete,"errors":errors,"warnings":warnings,"notices":0,"incomplete_checks":u32::from(!complete),"strict_warnings":false,"disposition":comparison.disposition().as_str()},
        "coverage":{"requested":{"inventory":null,"count_unit":"comparison","total":1,"assessed":u32::from(complete),"not_applicable":0,"unassessed":0,"undetermined":u32::from(!complete)},
            "broader":{"inventory":null,"state":"partial","inventory_status":"unavailable"},"rows":[],"rows_omitted":0,"omissions":[]},
        "diagnostics":{"retained":items.len(),"omitted":warnings_omitted,"truncated":warnings_omitted > 0 || !omissions.is_empty(),"reasons":reasons,"omissions":omissions,"items":items},
        "result":{"kind":"diff","outcome":outcome.as_str(),
            "summary":{"breaking":breaking,"undetermined":undetermined,"compatible":comparison.count(Classification::Compatible),
                "acknowledged":{"breaking":acknowledged(Classification::Breaking),"undetermined":acknowledged(Classification::Undetermined)},
                "documents":{"compared":comparison.compared.len(),"removed":comparison.removed.len(),"added":comparison.added.len()}},
            "documents":{"compared":comparison.compared.iter().map(|(earlier, later)| json!({"earlier":earlier.get(),"later":later.get()})).collect::<Vec<_>>(),
                "removed":comparison.removed.iter().map(|id| id.get()).collect::<Vec<_>>(),
                "added":comparison.added.iter().map(|id| id.get()).collect::<Vec<_>>()},
            "unused_acknowledgements":comparison.unused,
            "differences":differences,"differences_omitted":omitted,"omissions":result_omissions}
    })
}

/// Whether an optional detail item and its array separator fit the remaining allowance, which
/// it then spends.
fn fits(value: &Value, remaining: &mut usize) -> bool {
    let size = serde_json::to_string(value).expect("detail encoding").len() + 1;
    let fits = size <= *remaining;
    if fits {
        *remaining -= size;
    }
    fits
}

/// Count omitted presentation in one record per field and reason.
fn note(omissions: &mut Vec<Value>, field: &'static str, reason: &'static str, count: usize) {
    match omissions
        .iter_mut()
        .find(|omission| omission["field"] == field && omission["reason"] == reason)
    {
        Some(omission) => {
            omission["omitted_count"] =
                json!(omission["omitted_count"].as_u64().expect("omitted count") + count as u64)
        }
        None => omissions.push(json!({"field":field,"reason":reason,"omitted_count":count})),
    }
}

/// A difference's wire record, noting each location whose pointer it could not present.
fn difference(
    comparison: &RevisionComparison<'_>,
    record: &Record<'_>,
    pointers: &mut PointerBudget,
    cap: usize,
    limited: &mut Vec<(&'static str, &'static str)>,
) -> Value {
    let mut value = json!({"kind":record.kind.as_str(),"subject":subject(record.subject),
        "directions":super::DIRECTIONS.iter().filter(|direction| record.directions & direction.bit() != 0).map(|direction| direction.as_str()).collect::<Vec<_>>(),
        "classification":record.verdict.classification().as_str(),"acknowledged":record.acknowledged});
    if let DifferenceSubject::Quality(quality) = record.subject {
        value["quality"] = json!(quality);
    }
    for revision in REVISIONS {
        value[revision.as_str()] = record.sides[revision.index()]
            .as_ref()
            .map_or(Value::Null, |side| {
                side_wire(comparison, revision, side, pointers, cap, limited)
            });
    }
    if let Some(entry) = super::acknowledgement::entry(comparison, record) {
        value["acknowledgement"] = entry.wire();
    }
    if let Some(reason) = record.verdict.reason() {
        value["reason"] = json!({"code":reason.code(),"message":reason.message()});
    }
    if let Some(witness) = &record.witness {
        value["witness"] = json!({"direction":witness.direction().as_str(),"raw_json":witness.value(),
            "admission":{"earlier":witness.admission(Revision::Earlier).as_str(),"later":witness.admission(Revision::Later).as_str()}});
    }
    value
}

fn side_wire(
    comparison: &RevisionComparison<'_>,
    revision: Revision,
    side: &Side<'_>,
    pointers: &mut PointerBudget,
    cap: usize,
    limited: &mut Vec<(&'static str, &'static str)>,
) -> Value {
    let (pointer, reason) =
        crate::report::location_pointer(render_pointer(comparison, side, cap, pointers));
    let mut location = json!({"kind":"effective","root":side.root.get(),"pointer":pointer});
    if let Some(reason) = reason {
        const FIELD: &str = "result.differences.location.pointer";
        location["omission"] = json!({"field":FIELD,"reason":reason,"omitted_count":1,"source_id":side.root.get(),"revision":revision.as_str()});
        limited.push((FIELD, reason));
    }
    let origin = side
        .origin
        .and_then(|node| {
            crate::Explanation::selected(comparison.json(side, node))
                .sources()
                .next()
        })
        .map_or(Value::Null, |source| {
            let pointer = source.pointer_size(cap, pointers).and_then(|size| {
                pointers.debit(size.render_work())?;
                Ok(source.render_pointer(size))
            });
            const FIELD: &str = "result.differences.origin.pointer";
            let reason = match pointer {
                Ok(_) => None,
                Err(PointerError::NotAvailable) => Some("not_available"),
                Err(PointerError::Limit) => Some("location_limit"),
            };
            let mut origin =
                crate::report::source_location(source, pointer, FIELD, &mut Vec::new());
            if let Some(reason) = reason {
                origin["omission"]["revision"] = json!(revision.as_str());
                limited.push((FIELD, reason));
            }
            origin
        });
    json!({"presence":side.presence.as_str(),"location":location,"origin":origin})
}

pub(super) fn subject(subject: DifferenceSubject<'_>) -> &'static str {
    match subject {
        DifferenceSubject::Document => "document",
        DifferenceSubject::Definition(class) => class.keyword(),
        DifferenceSubject::Entry => "properties_entry",
        DifferenceSubject::Quality(_) => "quality",
    }
}

/// Serialize within the report allowance. The mandatory envelope fits the admitted reserve,
/// so only difference details can need omitting here.
fn encode(mut value: Value, limit: usize) -> String {
    loop {
        let text = serde_json::to_string(&value).expect("bounded diff report");
        if text.len() <= limit {
            return text;
        }
        let differences = value["result"]["differences"]
            .as_array_mut()
            .expect("differences");
        assert!(differences.pop().is_some(), "diff mandatory reservation");
        let omitted = value["result"]["differences_omitted"]
            .as_u64()
            .expect("omitted count")
            + 1;
        value["result"]["differences_omitted"] = json!(omitted);
        for owner in ["result", "diagnostics"] {
            let omissions = value[owner]["omissions"].as_array_mut().expect("omissions");
            note(omissions, "result.differences", "report_limit", 1);
        }
        let diagnostics = &mut value["diagnostics"];
        diagnostics["truncated"] = json!(true);
        let reasons = diagnostics["reasons"].as_array_mut().expect("reasons");
        if !reasons.contains(&json!("report_limit")) {
            reasons.push(json!("report_limit"));
        }
    }
}

pub(super) fn to_human(comparison: &RevisionComparison<'_>, style: crate::HumanStyle) -> String {
    let value: Value = serde_json::from_str(&to_json(comparison)).expect("diff report");
    let result = &value["result"];
    let key = |revision: &str, id: &Value| {
        value["sources"]
            .as_array()
            .expect("sources")
            .iter()
            .find(|source| source["revision"] == revision && source["id"] == *id)
            .and_then(|source| source["key"].as_str())
            .unwrap_or("?")
            .to_owned()
    };
    let mut text = HumanText::new(style);
    text.heading("Revision comparison");
    text.push_str(&format!(" {COMPARISON_CONTRACT}: "));
    let outcome = comparison.outcome();
    text.span(classification_role(outcome), outcome.as_str());
    text.push('\n');
    if value["policy"]["acknowledgements"] != 0 {
        let acknowledged = &result["summary"]["acknowledged"];
        text.heading("Disposition:");
        text.push(' ');
        let disposition = comparison.disposition();
        text.span(Role::disposition(disposition), disposition.as_str());
        text.push_str(&format!(
            ", with {} breaking and {} undetermined differences acknowledged\n",
            acknowledged["breaking"], acknowledged["undetermined"]
        ));
    }
    let documents = &result["summary"]["documents"];
    text.heading("Documents:");
    text.push_str(&format!(
        " {} compared, {} removed, {} added\n",
        documents["compared"], documents["removed"], documents["added"]
    ));
    for (label, field, revision, role) in [
        ("Removed documents:", "removed", "earlier", Role::Failed),
        ("Added documents:", "added", "later", Role::Passed),
    ] {
        let ids = result["documents"][field].as_array().expect("document ids");
        if ids.is_empty() {
            continue;
        }
        text.heading(label);
        for id in ids {
            text.push(' ');
            text.span(role, &key(revision, id));
        }
        text.push('\n');
    }
    text.heading("Differences:");
    for (index, classification) in ORDER.into_iter().enumerate() {
        text.push_str(if index == 0 { " " } else { ", " });
        text.push_str(&format!("{} ", result["summary"][classification.as_str()]));
        text.span(classification_role(classification), classification.as_str());
    }
    text.push('\n');
    // The recorded unit evidence, as in `check`, from the state the wire records also use.
    comparison.registry.write_human(&mut text);
    let diagnostics = &value["diagnostics"];
    let warnings = diagnostics["items"].as_array().expect("warnings");
    if !warnings.is_empty() {
        text.heading("Warnings:");
        text.push('\n');
    }
    for warning in warnings {
        let arguments = &warning["arguments"];
        let quoted = |name: &str| arguments.get(name).map(Value::to_string);
        text.push_str("  ");
        text.span(Role::Warning, warning["code"].as_str().expect("code"));
        let earlier = quoted("earlier_version").expect("earlier version");
        let breaking = super::wording::breaking_differences(
            arguments["breaking_differences"]
                .as_u64()
                .expect("breaking count"),
        );
        text.push_str(&format!(
            " in {}: {}\n",
            key("later", &warning["primary"]["source_id"]),
            match quoted("later_version") {
                Some(later) => format!(
                    "later version {later} does not compare higher than earlier version {earlier}, after {breaking}"
                ),
                None => format!(
                    "later version is absent, after earlier version {earlier} and {breaking}"
                ),
            }
        ));
    }
    if diagnostics["omitted"] != 0 {
        text.push_str(&format!(
            "Warning details omitted: {}\n",
            diagnostics["omitted"]
        ));
    }
    let differences = result["differences"].as_array().expect("differences");
    // Unacknowledged breaking and undetermined differences, acknowledged ones, whose rows name
    // their classification, then compatible ones.
    let sections = [
        (
            Some(Classification::Breaking),
            false,
            "Breaking differences:",
        ),
        (
            Some(Classification::Undetermined),
            false,
            "Undetermined differences:",
        ),
        (None, true, "Acknowledged differences:"),
        (
            Some(Classification::Compatible),
            false,
            "Compatible differences:",
        ),
    ];
    for (classification, acknowledged, heading) in sections {
        let mut rows = differences
            .iter()
            .filter(|difference| {
                difference["acknowledged"] == acknowledged
                    && classification.is_none_or(|classification| {
                        difference["classification"] == classification.as_str()
                    })
            })
            .peekable();
        if rows.peek().is_none() {
            continue;
        }
        match classification {
            Some(classification) => text.span(classification_role(classification), heading),
            None => text.heading(heading),
        }
        text.push('\n');
        for difference in rows {
            text.push_str(&format!("  {}", named(difference)));
            let directions: Vec<_> = difference["directions"]
                .as_array()
                .expect("directions")
                .iter()
                .map(|direction| direction.as_str().expect("direction").replace('_', "-"))
                .collect();
            if !directions.is_empty() {
                text.push_str(&format!(" ({} data)", directions.join(" and ")));
            }
            if acknowledged {
                classified(&mut text, difference);
            }
            if let Some(message) = difference["reason"]["message"].as_str() {
                text.push_str(&format!(": {message}"));
            }
            text.push('\n');
            let witness = &difference["witness"];
            if !witness.is_null() {
                let admission = |revision: &str| {
                    witness["admission"][revision]
                        .as_str()
                        .expect("admission")
                        .to_owned()
                };
                text.push_str(&format!(
                    "    witness {} for {} data: earlier {}, later {}\n",
                    witness["raw_json"].as_str().expect("witness value"),
                    witness["direction"]
                        .as_str()
                        .expect("direction")
                        .replace('_', "-"),
                    admission("earlier"),
                    admission("later")
                ));
            }
            for revision in ["earlier", "later"] {
                let side = &difference[revision];
                if side.is_null() {
                    continue;
                }
                let place = |root: &Value, pointer: &Value| {
                    format!(
                        "{} {}",
                        key(revision, root),
                        pointer.as_str().unwrap_or("(pointer omitted)")
                    )
                };
                let location = place(&side["location"]["root"], &side["location"]["pointer"]);
                text.push_str(&format!("    {revision} {location}"));
                if side["presence"] != "present" {
                    text.push_str(&format!(
                        " ({})",
                        side["presence"].as_str().expect("presence")
                    ));
                }
                text.push('\n');
                let origin = &side["origin"];
                if !origin.is_null() {
                    // Name the origin only where it differs, such as a shared definition.
                    let origin = place(&origin["source_id"], &origin["pointer"]);
                    if origin != location {
                        text.push_str(&format!("      from {origin}\n"));
                    }
                }
            }
        }
    }
    if result["differences_omitted"] != 0 {
        text.push_str(&format!(
            "Difference details omitted: {}\n",
            result["differences_omitted"]
        ));
    }
    let unused = result["unused_acknowledgements"]
        .as_array()
        .expect("unused acknowledgements");
    if let (Some(supplied), false) = (comparison.acknowledgements, unused.is_empty()) {
        text.heading("Unused acknowledgements:");
        text.push('\n');
        for position in unused {
            let position = position.as_u64().expect("entry position");
            let entry = supplied.entries()[position as usize].wire();
            text.push_str(&format!("  /differences/{position} {}", named(&entry)));
            classified(&mut text, &entry);
            text.push('\n');
            for revision in ["earlier", "later"] {
                let side = &entry[revision];
                if !side.is_null() {
                    text.push_str(&format!(
                        "    {revision} {} {}\n",
                        side["document"].as_str().expect("document"),
                        side["pointer"].as_str().expect("pointer")
                    ));
                }
            }
        }
    }
    // The human report shares the report allowance. When the differences the JSON report
    // kept are too long as text, the JSON report is given instead, as for model reports.
    let limit = comparison.catalogues[0].limits.report_bytes;
    if text.len() > limit {
        return to_json(comparison);
    }
    text.finish(limit)
}

/// A difference or acknowledgement entry's kind and subject, as human text.
fn named(value: &Value) -> String {
    let kind = value["kind"].as_str().expect("kind");
    match value["subject"].as_str().expect("subject") {
        "quality" => format!(
            "{kind} quality {}",
            value["quality"].as_str().expect("quality")
        ),
        "properties_entry" => format!("{kind} properties entry"),
        subject => format!("{kind} {subject}"),
    }
}

/// Name the classification of a row that is not listed under it.
fn classified(text: &mut HumanText, value: &Value) {
    let classification = match value["classification"].as_str().expect("classification") {
        "breaking" => Classification::Breaking,
        _ => Classification::Undetermined,
    };
    text.push_str(", ");
    text.span(classification_role(classification), classification.as_str());
}

fn classification_role(classification: Classification) -> Role {
    match classification {
        Classification::Breaking => Role::Failed,
        Classification::Undetermined => Role::Incomplete,
        Classification::Compatible => Role::Passed,
    }
}

// Actual mandatory producers, independent of declaration scanning.
// Follows ADR 0029: Content-format declarations and recorded registry guidance.
use super::*;

fn owned_dto(value: &Value) -> usize {
    match value {
        Value::Object(map) => {
            assert!(map.len() <= 14, "renew common map qualification for a larger graph");
            let structure = if map.len() <= 7 { 1536 } else { 4096 };
            structure + map.iter().map(|(key, value)| key.len() + owned_dto(value)).sum::<usize>()
        }
        Value::Array(values) => values.capacity() * std::mem::size_of::<Value>()
            + values.iter().map(owned_dto).sum::<usize>(),
        Value::String(text) => text.capacity(),
        Value::Number(_) => 32,
        _ => 0,
    }
}

#[test]
fn common_header_and_simultaneous_owners_fit_unchanged_c() {
    use crate::{InspectRequest, Limits, SourceInput};
    assert!(crate::source::common_report_supported());
    for count in [1, 256] {
        let keys: Vec<_> = (0..count).map(|i| format!("{i:04}{}", "k".repeat(252))).collect();
        let label = "l".repeat(512);
        let uri = "u".repeat(2048);
        let version = "v".repeat(128);
        let digest = "d".repeat(256);
        for omitted_label in [false, true] {
            let metadata = count * (256 + 2048 + 128 + 256 + if omitted_label { 0 } else { 512 });
            let c = 6 * metadata + count * 4096 + 16384;
            let inputs: Vec<_> = keys.iter().map(|key| {
                let input = SourceInput::supplied(key, br#"{"info":{}}"#)
                    .acquisition_uri(&uri).version_token(&version).digest(&digest);
                if omitted_label { input.omit_label() } else { input.label(&label) }
            }).collect();
            for (profile, extra) in [(Assessment::Initial, 0), (Assessment::ExpandedModified, 1024)] {
                let cat = Catalogue::with_limits(&inputs, Limits::default().report_bytes(c + extra)).unwrap();
                assert_eq!(cat.reserve, c);
                let report = cat.check(CheckRequest::new().assessment(profile)).unwrap();
                let (base, result, presentation) = report.metadata(true);
                assert!(base["coverage"]["rows"].as_array().unwrap().is_empty());
                assert!(base["diagnostics"]["items"].as_array().unwrap().is_empty());
                let owners = owned_dto(&base) + 2 * owned_dto(&result)
                    + (std::mem::size_of_val(&report) + projection::owner_bytes()) + 4096 + std::mem::size_of_val(&presentation);
                assert!(owners <= 4 * (c + extra));
                let json = report.to_json();
                let human = report.to_human();
                let fallback = report.render_human(0);
                assert!(json.len() <= c + extra && human.len() <= c + extra && fallback.len() <= c + extra);
                let forced: Value = serde_json::from_str(&fallback).unwrap();
                assert_eq!(forced["coverage"]["requested"]["total"], if profile == Assessment::Initial { 8 } else { 9 });
                assert!(forced["baseline"]["registry_knowledge"].as_array().unwrap().is_empty());
                assert!(forced["policy"].get("recorded_registry").is_none());
                println!("common profile={profile:?} sources={count} omitted_label={omitted_label} C={c} header={} source_bound_common={} observed_graph_bound={owners} ownership_limit={} json={} human={} forced_fallback={}", std::mem::size_of_val(&report), common_owner_bytes(), 4 * (c + extra), json.len(), human.len(), fallback.len());
            }
            let cat = Catalogue::with_limits(&inputs, Limits::default().report_bytes(c)).unwrap();
            let report = cat.inspect(InspectRequest::new(cat.source_id(&keys[0]).unwrap(), "")).unwrap();
            let (base, result, presentation) = report.metadata(true);
            let owners = owned_dto(&base) + 2 * owned_dto(&result)
                + (std::mem::size_of_val(&report) + projection::owner_bytes()) + 4096 + std::mem::size_of_val(&presentation);
            assert!(owners <= 4 * c);
            let fallback = report.render_human(0);
            assert!(serde_json::from_str::<Value>(&fallback).is_ok());
            assert!(fallback.len() <= c);
            println!("common inspect sources={count} omitted_label={omitted_label} C={c} header={} observed_graph_bound={owners} ownership_limit={} forced_fallback={}", std::mem::size_of_val(&report), 4*c, fallback.len());
        }
    }
}

#[test]
fn maximum_inspection_selector_and_old_result_keep_separate_charges() {
    use crate::{InspectRequest, Limits, SourceInput};
    let name = "\u{1}".repeat(8192 - "/sdfProperty/".len());
    let pointer = format!("/sdfProperty/{name}");
    let model = json!({"info":{},"sdfProperty":{name:{"type":"number"}}}).to_string();
    let c = 6 * "fixture".len() + 4096 + 16384;
    let cap = c + 12 * pointer.len();
    let inputs = [SourceInput::supplied("fixture", model.as_bytes())];
    let small = Catalogue::with_limits(&inputs, Limits::default().report_bytes(cap-1)).unwrap();
    assert_eq!(small.inspect(InspectRequest::new(small.source_id("fixture").unwrap(), &pointer)).err(), Some("report cannot reserve its mandatory inspection selector"));
    let cat = Catalogue::with_limits(&inputs, Limits::default().report_bytes(cap)).unwrap();
    let report = cat.inspect(InspectRequest::new(cat.source_id("fixture").unwrap(), &pointer)).unwrap();
    let (base, result, presentation) = report.metadata(true);
    let owners = owned_dto(&base) + 2 * owned_dto(&result) + (std::mem::size_of_val(&report) + projection::owner_bytes()) + 4096 + std::mem::size_of_val(&presentation);
    assert!(owners <= 4 * cap);
    let json = report.to_json();
    let human = report.to_human();
    let fallback = report.render_human(0);
    for encoded in [&json, &fallback] {
        let wire: Value = serde_json::from_str(encoded).unwrap();
        assert_eq!(wire["request"]["selector"]["pointer"], pointer);
        assert_eq!(wire["result"]["pointer"], pointer);
        assert!(encoded.len() <= cap);
    }
    assert!(human.len() <= cap);
    println!("maximum inspection selector={} cap={cap} observed_graph_bound={owners} ownership_limit={} json={} human={} forced_fallback={}", pointer.len(), 4*cap, json.len(), human.len(), fallback.len());
}

#[test]
fn actual_registry_producers_survive_exact_admission_omission_and_fallback() {
    use crate::{Limits, RecordedRegistryPolicy, SourceInput};
    for key in ["authored", "fixture"] {
    let c = 6 * key.len() + 4096 + 16384;
    for (profile, e, mask) in [
        (Assessment::ExpandedContentFormat, crate::registry::CONTENT_RESERVE, 28),
        (Assessment::ExpandedUnits, crate::registry::UNIT_RESERVE, 3),
        (Assessment::FullBaseline, crate::registry::FULL_RESERVE, 63),
    ] {
        let input = [SourceInput::supplied(key, br#"{"info":{}}"#)];
        let p = if profile == Assessment::FullBaseline { crate::pattern::REPORT_RESERVE } else { 0 };
        let cap = c + e + p;
        let small = Catalogue::with_limits(&input, Limits::default().report_bytes(cap - 1)).unwrap();
        assert_eq!(small.check(CheckRequest::new().assessment(profile)).err(), Some(if p > 0 { "report cannot reserve the pattern assessment envelope" } else { "report cannot reserve the registry assessment envelope" }));
        let cat = Catalogue::with_limits(&input, Limits::default().report_bytes(cap).diagnostic_bytes(1).conditions(1)).unwrap();
        assert_eq!(cat.reserve, c);
        for policy in [RecordedRegistryPolicy::UseRecorded, RecordedRegistryPolicy::WithholdRecorded] {
            for consulted in [0, 1, 3, 4, 12, 15, 28, 31, 32, 63] {
                let used = consulted & mask;
                let mut report = cat.check(CheckRequest::new().assessment(profile).recorded_registry_policy(policy)).unwrap();
                // Supply admitted owner observations to the real producer. This is
                // deliberately not a declaration lookup or scanner observation.
                if policy == RecordedRegistryPolicy::UseRecorded { report.registry.consulted = used; }
                else { report.registry.withheld = used; }
                report.registry.interpretations = if profile == Assessment::FullBaseline { 7 } else { 4 };
                let json = report.to_json();
                let human = report.to_human();
                let fallback = report.render_human(0);
                for encoded in [&json, &fallback] {
                    assert!(encoded.len() <= cap);
                    let wire: Value = serde_json::from_str(encoded).unwrap();
                    assert_eq!(wire["policy"]["recorded_registry"], policy.as_str());
                    let records = wire["baseline"]["registry_knowledge"].as_array().unwrap();
                    assert_eq!(records.len(), used.count_ones() as usize);
                    for record in records {
                        assert_eq!(record.as_object().unwrap().len(), if policy == RecordedRegistryPolicy::UseRecorded { 10 } else { 4 });
                        assert_eq!(record["snapshot"].is_null(), policy == RecordedRegistryPolicy::WithholdRecorded);
                    }
                    assert!(wire["coverage"]["rows"].as_array().unwrap().is_empty());
                    assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
                    assert_eq!(wire["outcome"]["complete"], report.outcome().complete());
                }
                assert!(human.len() <= cap);
                println!("extension key={key} profile={profile:?} policy={policy:?} records={} cap={cap} json={} human={} forced_fallback={}", used.count_ones(), json.len(), human.len(), fallback.len());
            }
        }
        let limited = Catalogue::with_limits(&input, Limits::default().report_bytes(cap).effective_allocation_bytes(1)).unwrap();
        let report = limited.check(CheckRequest::new().assessment(profile)).unwrap();
        assert!(report.registry.owner_failed);
        for index in 0..crate::registry::RECORDS {
            let facts = crate::registry::facts(index).unwrap();
            if mask & (1 << index) != 0 { assert!(matches!(report.registry_knowledge(facts.registry), Field::Unavailable(_))); }
            else { assert!(matches!(report.registry_knowledge(facts.registry), Field::Absent)); }
        }
        let wire: Value = serde_json::from_str(&report.render_human(0)).unwrap();
        assert!(wire["baseline"]["registry_knowledge"].as_array().unwrap().is_empty());
        if profile != Assessment::ExpandedContentFormat { assert!(wire["result"]["unit_guidance"]["counts"].is_null()); }
        if profile == Assessment::ExpandedContentFormat {
            for area in crate::content_format::AREAS { assert!(matches!(report.assessment_area_status(area), Field::Present(AssessmentStatus::Undetermined))); }
        }
    }
    }
}

#[test]
fn actual_sample_header_and_retained_result_fit_unchanged_allowance() {
    use crate::{Limits, SampleKind, SampleRequest, SourceInput};
    const C: usize = 12 + 2 * 4096 + 16384 + crate::sample::MANDATORY_WIRE;
    let inputs = [SourceInput::supplied("m", br#"{"sdfProperty":{"p":{"type":"string","nullable":false}}}"#), SourceInput::supplied("s", b"false").sample()];
    let cat = Catalogue::with_limits(&inputs, Limits::default().report_bytes(C)).unwrap();
    let report = cat.check_sample(SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Property, "/sdfProperty/p").sample(cat.source_id("s").unwrap())).unwrap();
    assert!(crate::sample::mandatory_storage_supported());
    assert_eq!(report.outcome().errors(), 1);
    let json = report.to_json();
    let human = report.to_human();
    let fallback = report.render_human(0);
    for encoded in [&json, &fallback] {
        let wire: Value = serde_json::from_str(encoded).unwrap();
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        assert!(wire["baseline"]["registry_knowledge"].as_array().unwrap().is_empty());
        assert!(encoded.len() <= C);
    }
    assert!(human.len() <= C);
    println!("sample Report={} cap={C} json={} human={} forced_fallback={} mandatory_supported=true", std::mem::size_of_val(&report), json.len(), human.len(), fallback.len());
}

#[test]
fn real_two_phase_counts_and_fallback_keep_each_coordinate_once() {
    use crate::{Limits, RecordedRegistryPolicy, SourceInput};
    let bytes = br#"{"info":{},"sdfData":{"A":{"contentFormat":"application/json;p=x@gzip@gzip"},"B":{"contentFormat":"65536"},"C":{"contentFormat":"text/plain;"},"D":{"contentFormat":"60","sdfChoice":{"Inherited":{},"Override":{"contentFormat":"application/json@zstd"}}}}}"#;
    for policy in [RecordedRegistryPolicy::UseRecorded, RecordedRegistryPolicy::WithholdRecorded] {
        let original = Catalogue::from_inputs(&[SourceInput::supplied("authored", bytes)]).unwrap();
        let original = original.check(CheckRequest::new().assessment(Assessment::ExpandedContentFormat).recorded_registry_policy(policy)).unwrap();
        let counts = original.content.as_ref().unwrap().counts;
        assert_eq!(&counts[..2], &[6,6]);
        assert_eq!(counts[2], 7); // Three A tokens, D and its two independent alternatives.
        assert_eq!(counts[3], if policy == RecordedRegistryPolicy::UseRecorded { 0 } else { 7 });
        assert_eq!(original.outcome().errors(), 2);
        assert_eq!(original.diagnostics().conditions().filter(|c| c.code() == "content_format.syntax").count(), 1);
        assert_eq!(original.diagnostics().conditions().filter(|c| c.code() == "content_format.number_range").count(), 1);
        assert_eq!(original.diagnostics().conditions().filter(|c| c.code() == "content_format.parameter_semantics_unassessed").count(), 1);
        let catalogue = Catalogue::with_limits(&[SourceInput::supplied("authored", bytes)], Limits::default().report_bytes(39644).diagnostic_bytes(1).conditions(1)).unwrap();
        let report = catalogue.check(CheckRequest::new().assessment(Assessment::ExpandedContentFormat).recorded_registry_policy(policy)).unwrap();
        assert_eq!(report.content.as_ref().unwrap().counts, counts);
        assert_eq!(report.outcome().errors(), 2);
        assert_eq!(report.structure.findings.len(), 0);
        let json = report.to_json();
        let human = report.to_human();
        let fallback = report.render_human(0);
        for encoded in [&json, &fallback] {
            let wire: Value = serde_json::from_str(encoded).unwrap();
            assert_eq!(wire["outcome"]["errors"], 2);
            assert_eq!(wire["baseline"]["registry_knowledge"].as_array().unwrap().len(), 3);
            assert!(encoded.len() <=39644);
        }
        println!("real two-phase policy={policy:?} counts={counts:?} errors2 json={} human={} forced_fallback={}", json.len(), human.len(), fallback.len());
    }
}

#[test]
fn required_summary_and_retained_fallback_fit_actual_common_header() {
    use crate::{InspectRequest, Limits, SourceInput};
    let bytes = br##"{"info":{},"sdfObject":{"O":{"sdfProperty":{"P":{"type":"number"}},"sdfRequired":["#/sdfObject/O/sdfProperty/P"]}}}"##;
    let c = 20522;
    let pointer = "/sdfObject/O";
    let cap = c + 12*pointer.len() + 1024;
    let small = Catalogue::with_limits(&[SourceInput::supplied("fixture", bytes)], Limits::default().report_bytes(cap-1)).unwrap();
    assert_eq!(small.inspect(InspectRequest::new(small.source_id("fixture").unwrap(), pointer).required_declarations(true)).err(), Some("report cannot reserve required declaration summary"));
    let catalogue = Catalogue::with_limits(&[SourceInput::supplied("fixture", bytes)], Limits::default().report_bytes(cap)).unwrap();
    let report = catalogue.inspect(InspectRequest::new(catalogue.source_id("fixture").unwrap(), pointer).required_declarations(true)).unwrap();
    let (base, result, presentation) = report.metadata(true);
    let owners = owned_dto(&base)+2*owned_dto(&result)+(std::mem::size_of_val(&report) + projection::owner_bytes())+4096+std::mem::size_of_val(&presentation);
    assert!(owners <=4*cap);
    for encoded in [report.to_json(), report.render_human(0)] {
        let wire: Value = serde_json::from_str(&encoded).unwrap();
        assert!(wire["result"]["required_declarations"].is_object());
        assert!(encoded.len()<=cap);
    }
    assert!(report.to_human().len()<=cap);
    println!("actual required header={} C={c} cap={cap} simultaneous_owner_bound={owners} ownership_limit={} retained_fallback=true", std::mem::size_of_val(&report), 4*cap);
}

#[test]
fn maximum_full_interpretations_fit_original_owners_growth_and_fallback() {
    use crate::{Limits, SourceInput};
    const IDS: [&str;11] = ["unit-index-mapping/1", "unit-marker-identity/1", "I-CF-RANGE-1",
        "content-format-model-meaning/1", "ADR0018-selected-representation", "byte-string-json/1",
        "unix-time-posix-coordinate/1", "uuid-bare-json/1", "uuid-opaque-value/1",
        "copied-reference-relocation/1", "undeclared-extensions-ignorable/1"];
    assert!(IDS.iter().all(|id|id.len()<=40));
    let c=6*"fixture".len()+4096+16384;
    let e=crate::registry::FULL_RESERVE;
    let cap=c+e+crate::pattern::REPORT_RESERVE;
    let cat=Catalogue::with_limits(&[SourceInput::supplied("fixture",br#"{"info":{},"sdfData":{"D":{"type":"boolean"}}}"#)],
        Limits::default().report_bytes(cap).diagnostic_bytes(1).conditions(1)).unwrap();
    let mut report=cat.check(CheckRequest::new().assessment(Assessment::FullBaseline)).unwrap();
    // Exercise the actual shared report producer with every admitted fixed
    // observation. Scanner facts and source-dependent locations are separate.
    report.registry.consulted=63;
    report.registry.interpretations=15;
    report.structure.temporal_use=crate::civil::CONVENTION_USED|crate::civil::KNOWLEDGE_USED;
    report.structure.abstract_use=3;
    report.structure.format_use=crate::formats::UUID_TEXT_USED|crate::formats::UUID_VALUE_USED;
    report.effective[0].relocation_interpretation=true;
    report.structure.set_aside.classify(crate::role::Role::Data,"acme:note",false);
    let (base,result,_)=report.metadata(true);
    let ids=base["baseline"]["interpretations"].as_array().unwrap();
    assert_eq!(ids.len(),11);
    assert_eq!(ids.capacity(),16);
    for id in IDS { assert!(ids.iter().any(|value|value==id)); }
    assert_eq!(base["baseline"]["registry_knowledge"].as_array().unwrap().len(),6);
    assert!(base["baseline"]["temporal_knowledge"].is_object());
    let d=owned_dto(&base["baseline"])+owned_dto(&base["policy"])+owned_dto(&result["unit_guidance"]);
    let growth=8*std::mem::size_of::<Value>();
    let owners=2*d+growth+208;
    assert!(owners<=4*e,"combined fixed extension generations: {owners} > {}",4*e);
    let whole=owned_dto(&base)+2*owned_dto(&result)+(std::mem::size_of_val(&report) + projection::owner_bytes())+4096;
    assert!(whole<=4*cap,"whole graph including retained result: {whole}");
    for encoded in [report.to_json(),report.render_human(0)] {
        assert!(encoded.len()<=cap);
        let wire:Value=serde_json::from_str(&encoded).unwrap();
        assert_eq!(wire["baseline"]["interpretations"].as_array().unwrap().len(),11);
        assert_eq!(wire["baseline"]["registry_knowledge"].as_array().unwrap().len(),6);
        assert!(wire["coverage"]["rows"].as_array().unwrap().is_empty());
        assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
    }
    assert!(report.to_human().len()<=cap);
    println!("maximum combined Full: ids11 capacity16 original_E={e} one_extension_D={d} two_D_plus_growth_owners={owners} limit={} whole_with_retained_result={whole} whole_limit={} cap={cap}",4*e,4*cap);
}

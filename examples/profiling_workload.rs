//! A reproducible profiling workload over the offline device Catalogue.
//!
//! Each round runs every Check plan of the six device families as `check --plan` would:
//! acquisition and parsing of the plan's sources, an effective view of each model document,
//! then the plan's Model checking, Sample-value checking and Revision comparison cases. The
//! rich door lock plan's comparison of the minimal and rich models is the round's only one.
//! Each round ends with two Recorded traces and one generation request: thermostat readings
//! checked against the rich thermostat model, door lock credentials checked against the rich
//! door lock model's length and pattern constraints, and generated values for every affordance
//! description of the rich door lock model. Build with the `hotpath` feature for a profile,
//! or without it for a plain timing. See docs/profiling.md.

use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, ComparisonRequest, DifferenceAcknowledgements,
    EffectiveRequest, GenerationRequest, SampleKind, SampleRequest, SourceInput, TraceOutcome,
    TraceRequest,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

const FAMILIES: [&str; 6] = [
    "thermostat-check",
    "rgb-lamp-check",
    "door-lock-check",
    "power-strip-check",
    "irrigation-check",
    "coffee-machine-check",
];
const DEFAULT_ROUNDS: u32 = 200;
/// The Trace entries in each of the round's Recorded traces.
const TRACE_ENTRIES: u32 = 1000;

struct Source {
    key: String,
    path: PathBuf,
    sample: bool,
}
enum Case {
    Check {
        roots: Vec<usize>,
        assessment: Assessment,
        strict: bool,
    },
    Sample {
        document: usize,
        kind: SampleKind,
        pointer: String,
        sample: usize,
        strict: bool,
    },
    Diff {
        /// Each revision's documents as source keys and paths.
        revisions: [Vec<(String, PathBuf)>; 2],
        acknowledged: Option<DifferenceAcknowledgements>,
    },
}
struct Plan {
    sources: Vec<Source>,
    cases: Vec<Case>,
}

/// Work and results summed over all rounds. Printed per round, they repeat exactly for
/// unchanged inputs and code.
#[derive(Default)]
struct Tally {
    sources: usize,
    effective_views: usize,
    checks: usize,
    samples: usize,
    request_errors: usize,
    errors: usize,
    incomplete: usize,
    differences: usize,
    trace_entries: usize,
    failed_entries: usize,
    generated: usize,
    rejected_values: usize,
}

fn text<'a>(row: &'a Value, member: &str) -> &'a str {
    row[member]
        .as_str()
        .unwrap_or_else(|| panic!("plan member {member}"))
}

/// Decode one Check plan, accepting only the plan vocabulary the device families use.
fn plan(path: &Path) -> Plan {
    let value: Value =
        serde_json::from_slice(&fs::read(path).expect("readable plan")).expect("plan is JSON");
    let directory = path.parent().expect("plan directory");
    let sources: Vec<_> = value["sources"]
        .as_array()
        .expect("plan sources")
        .iter()
        .map(|row| Source {
            key: text(row, "key").to_owned(),
            path: directory.join(text(row, "path")),
            sample: text(row, "role") == "sample",
        })
        .collect();
    let index = |key: &str| {
        sources
            .iter()
            .position(|source| source.key == key)
            .expect("plan source key")
    };
    let cases = value["cases"]
        .as_array()
        .expect("plan cases")
        .iter()
        .map(|row| {
            let strict = row["strict_warnings"].as_bool().unwrap_or(false);
            match text(row, "operation") {
                "check" => Case::Check {
                    roots: row["scope"]["roots"]
                        .as_array()
                        .map_or(Vec::new(), |roots| {
                            roots
                                .iter()
                                .map(|root| index(root.as_str().expect("root key")))
                                .collect()
                        }),
                    assessment: match row["assessment"].as_str().unwrap_or("initial-v1") {
                        "initial-v1" => Assessment::Initial,
                        "full-baseline" => Assessment::FullBaseline,
                        other => panic!("assessment {other} is outside the workload"),
                    },
                    strict,
                },
                "sample" => Case::Sample {
                    document: index(text(row, "document")),
                    kind: match text(row, "kind") {
                        "property" => SampleKind::Property,
                        "data" => SampleKind::Data,
                        "action_input" => SampleKind::ActionInput,
                        "action_output" => SampleKind::ActionOutput,
                        "event_output" => SampleKind::EventOutput,
                        other => panic!("sample kind {other}"),
                    },
                    pointer: text(row, "pointer").to_owned(),
                    sample: index(text(row, "sample")),
                    strict,
                },
                "diff" => Case::Diff {
                    revisions: ["earlier", "later"].map(|revision| {
                        row[revision]
                            .as_object()
                            .expect("revision documents")
                            .iter()
                            .map(|(key, file)| {
                                let file = file.as_str().expect("document path");
                                (key.clone(), directory.join(file))
                            })
                            .collect()
                    }),
                    acknowledged: row["acknowledged"].as_str().map(|file| {
                        let bytes = fs::read(directory.join(file)).expect("acknowledgements");
                        DifferenceAcknowledgements::from_json(&bytes)
                            .expect("decoded acknowledgements")
                    }),
                },
                other => panic!("operation {other} is outside the workload"),
            }
        })
        .collect();
    Plan { sources, cases }
}

/// Every Check plan of the device families, in a fixed order.
fn plans(examples: &Path) -> Vec<Plan> {
    let mut paths = Vec::new();
    for family in FAMILIES {
        let mut found: Vec<_> = fs::read_dir(examples.join(family))
            .expect("device family directory")
            .map(|entry| entry.expect("directory entry").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with("plan.json"))
            })
            .collect();
        found.sort();
        paths.extend(found);
    }
    paths.iter().map(|path| plan(path)).collect()
}

fn run_plan(plan: &Plan, tally: &mut Tally) {
    let inputs = plan
        .sources
        .iter()
        .map(|source| {
            let input = SourceInput::file(&source.key, &source.path);
            if source.sample { input.sample() } else { input }
        })
        .collect();
    let catalogue = Catalogue::take_inputs(inputs).expect("admitted Catalogue");
    let ids: Vec<_> = catalogue.sources().map(|source| source.id()).collect();
    tally.sources += ids.len();
    for (source, &id) in plan.sources.iter().zip(&ids) {
        if source.sample {
            continue;
        }
        match catalogue.effective(EffectiveRequest::new(id)) {
            Ok(model) => {
                tally.effective_views += 1;
                tally.incomplete += usize::from(!model.complete());
            }
            Err(_) => tally.request_errors += 1,
        }
    }
    for case in &plan.cases {
        let report = match case {
            Case::Check {
                roots,
                assessment,
                strict,
            } => {
                tally.checks += 1;
                let mut request = CheckRequest::new()
                    .assessment(*assessment)
                    .strict_warnings(*strict);
                for root in roots {
                    request = request.root(ids[*root]);
                }
                catalogue.check(request)
            }
            Case::Sample {
                document,
                kind,
                pointer,
                sample,
                strict,
            } => {
                tally.samples += 1;
                catalogue.check_sample(
                    SampleRequest::new(ids[*document], *kind, pointer)
                        .sample(ids[*sample])
                        .strict_warnings(*strict),
                )
            }
            Case::Diff {
                revisions,
                acknowledged,
            } => {
                compare(revisions, acknowledged.as_ref(), tally);
                continue;
            }
        };
        match report {
            Ok(report) => {
                tally.errors += report.outcome().errors() as usize;
                tally.incomplete += usize::from(!report.outcome().complete());
            }
            Err(_) => tally.request_errors += 1,
        }
    }
}

/// A diff case acquires each revision into its own Catalogue, apart from the plan's sources.
fn compare(
    revisions: &[Vec<(String, PathBuf)>; 2],
    acknowledged: Option<&DifferenceAcknowledgements>,
    tally: &mut Tally,
) {
    let [earlier, later] = revisions.each_ref().map(|documents| {
        let inputs = documents
            .iter()
            .map(|(key, path)| SourceInput::file(key, path))
            .collect();
        Catalogue::take_inputs(inputs).expect("admitted revision")
    });
    let mut request = ComparisonRequest::new(&earlier, &later);
    if let Some(acknowledged) = acknowledged {
        request = request.acknowledgements(acknowledged);
    }
    let comparison = request.compare().expect("comparison");
    tally.differences += comparison.differences().len();
}

/// One Trace entry line for the property at `pointer`, with the JSON `value`.
fn entry(pointer: &str, value: &str, index: u32) -> String {
    format!(
        "{{\"kind\":\"property\",\"pointer\":\"{pointer}\",\"value\":{value},\"label\":\"reading {index}\"}}\n"
    )
}

/// A Recorded trace of thermostat readings: target temperatures from 5 to 30 in half steps,
/// with every hundredth reading at 35, which the thermostat model rejects.
fn readings() -> String {
    let mut trace = String::new();
    for index in 0..TRACE_ENTRIES {
        let steps = index % 51;
        let value = if index % 100 == 99 {
            "35".to_owned()
        } else if steps % 2 == 0 {
            format!("{}", 5 + steps / 2)
        } else {
            format!("{}.5", 5 + steps / 2)
        };
        trace.push_str(&entry(
            "/sdfObject/Thermostat/sdfProperty/targetTemperature",
            &value,
            index,
        ));
    }
    trace
}

/// Door lock credential readings against the credential's length and pattern constraints.
/// Every hundredth reading holds a space, which the pattern rejects.
fn credentials() -> String {
    let mut trace = String::new();
    for index in 0..TRACE_ENTRIES {
        let value = if index % 100 == 99 {
            format!("\"k {index:04}\"")
        } else {
            format!("\"k{index:05}\"")
        };
        trace.push_str(&entry(
            "/sdfObject/Lock/sdfProperty/credential",
            &value,
            index,
        ));
    }
    trace
}

/// Generate values for every definition of the rich door lock model, as
/// `generate --document` would.
fn run_generation(model: &Path, tally: &mut Tally) {
    let catalogue = Catalogue::take_inputs(vec![SourceInput::file("model", model)])
        .expect("admitted Catalogue");
    let document = catalogue.source_id("model").expect("model source");
    let result = catalogue
        .generate(GenerationRequest::new(document))
        .expect("generation");
    for admission in [
        Admission::Admitted,
        Admission::Rejected,
        Admission::Undetermined,
    ] {
        tally.generated += result.count(admission) as usize;
    }
    tally.rejected_values += result.count(Admission::Rejected) as usize;
}

/// Acquire `model` and check the Recorded trace against it.
fn run_trace(model: &Path, trace: &str, tally: &mut Tally) {
    let catalogue = Catalogue::take_inputs(vec![SourceInput::file("model", model)])
        .expect("admitted Catalogue");
    let document = catalogue.source_id("model").expect("model source");
    let result = catalogue
        .check_trace(TraceRequest::new(document, trace.as_bytes()))
        .expect("trace check");
    for outcome in [
        TraceOutcome::Passed,
        TraceOutcome::Failed,
        TraceOutcome::Undetermined,
    ] {
        tally.trace_entries += result.count(outcome) as usize;
    }
    tally.failed_entries += result.count(TraceOutcome::Failed) as usize;
}

#[cfg_attr(feature = "hotpath", hotpath::main)]
fn main() {
    let rounds = std::env::args().nth(1).map_or(DEFAULT_ROUNDS, |value| {
        value.parse().expect("rounds is a positive whole number")
    });
    assert!(rounds > 0, "rounds is a positive whole number");
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let plans = plans(&examples);
    let thermostat = examples.join("thermostat-check/rich.sdf.json");
    let trace = readings();
    let door_lock = examples.join("door-lock-check/rich.sdf.json");
    let credential_trace = credentials();
    let mut tally = Tally::default();
    let start = Instant::now();
    for _ in 0..rounds {
        for plan in &plans {
            run_plan(plan, &mut tally);
        }
        run_trace(&thermostat, &trace, &mut tally);
        run_trace(&door_lock, &credential_trace, &mut tally);
        run_generation(&door_lock, &mut tally);
    }
    let elapsed = start.elapsed();
    let per_round = |total: usize| total / rounds as usize;
    println!(
        "{} plans per round: {} sources, {} effective views, {} model checks, {} sample checks, \
         {} request errors, {} error findings, {} incomplete results, {} comparison differences, \
         {} trace entries, {} failed entries, {} generated values, {} rejected values",
        plans.len(),
        per_round(tally.sources),
        per_round(tally.effective_views),
        per_round(tally.checks),
        per_round(tally.samples),
        per_round(tally.request_errors),
        per_round(tally.errors),
        per_round(tally.incomplete),
        per_round(tally.differences),
        per_round(tally.trace_entries),
        per_round(tally.failed_entries),
        per_round(tally.generated),
        per_round(tally.rejected_values),
    );
    println!(
        "{rounds} rounds in {:.1} ms, {:.3} ms per round",
        elapsed.as_secs_f64() * 1e3,
        elapsed.as_secs_f64() * 1e3 / f64::from(rounds)
    );
}

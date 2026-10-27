use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::{Command, Output},
};

#[derive(Clone, Copy)]
pub enum Admission {
    Admitted,
    Rejected,
    Undetermined,
}
impl Admission {
    fn text(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Rejected => "rejected",
            Self::Undetermined => "undetermined",
        }
    }
}
#[derive(Clone, Copy)]
pub enum Disposition {
    Passed,
    Failed,
    Undetermined,
}
impl Disposition {
    fn text(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Undetermined => "undetermined",
        }
    }
}
#[derive(Clone, Copy)]
pub enum Classification {
    Breaking,
}
impl Classification {
    fn text(self) -> &'static str {
        match self {
            Self::Breaking => "breaking",
        }
    }
}
#[derive(Clone, Copy)]
pub enum State {
    Report,
}
#[derive(Clone, Copy)]
pub enum ModelAssessment {
    InitialCatalogue,
    FullSelected(&'static str),
}
#[derive(Clone)]
pub struct Location {
    pub kind: LocationKind,
    pub source: &'static str,
    pub pointer: String,
}
impl Location {
    pub fn new(kind: LocationKind, source: &'static str, pointer: impl Into<String>) -> Self {
        Self {
            kind,
            source,
            pointer: pointer.into(),
        }
    }
}
#[derive(Clone, Copy)]
pub enum LocationKind {
    Source,
    Effective,
    Sample,
}
impl LocationKind {
    fn wire(self) -> (&'static str, &'static str) {
        match self {
            Self::Source => ("source", "source_id"),
            Self::Effective => ("effective", "root"),
            Self::Sample => ("sample", "source_id"),
        }
    }
}
#[derive(Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub primary: Location,
    pub related: Vec<Location>,
}
impl Diagnostic {
    pub fn constraint(code: &'static str, sample: Location, source: Location) -> Self {
        Self {
            code,
            primary: sample,
            related: vec![
                Location::new(
                    LocationKind::Effective,
                    source.source,
                    source.pointer.clone(),
                ),
                source,
            ],
        }
    }

    pub fn unavailable_reference(source: Location) -> Self {
        Self {
            code: "reference.unavailable",
            primary: Location::new(
                LocationKind::Effective,
                source.source,
                source.pointer.clone(),
            ),
            related: vec![Location::new(
                LocationKind::Source,
                source.source,
                format!("{}/sdfRef", source.pointer),
            )],
        }
    }
}
#[derive(Clone)]
pub struct Case {
    pub name: &'static str,
    pub rationale: &'static str,
    pub disposition: Disposition,
    pub complete: bool,
    pub admission: Option<Admission>,
    pub classification: Option<Classification>,
    pub state: State,
    pub model_assessment: Option<ModelAssessment>,
    pub diagnostics: Vec<Diagnostic>,
}
impl Case {
    pub fn model(name: &'static str, rationale: &'static str) -> Self {
        Self {
            name,
            rationale,
            disposition: Disposition::Passed,
            complete: true,
            admission: None,
            classification: None,
            state: State::Report,
            model_assessment: Some(ModelAssessment::InitialCatalogue),
            diagnostics: vec![],
        }
    }
    /// A complete Revision comparison, whose disposition follows its acknowledgements.
    pub fn diff(
        name: &'static str,
        classification: Classification,
        disposition: Disposition,
        rationale: &'static str,
    ) -> Self {
        Self {
            classification: Some(classification),
            model_assessment: None,
            disposition,
            ..Self::model(name, rationale)
        }
    }
    pub fn sample(name: &'static str, admission: Admission, rationale: &'static str) -> Self {
        Self {
            admission: Some(admission),
            model_assessment: None,
            disposition: match admission {
                Admission::Admitted => Disposition::Passed,
                Admission::Rejected => Disposition::Failed,
                Admission::Undetermined => Disposition::Undetermined,
            },
            complete: !matches!(admission, Admission::Undetermined),
            ..Self::model(name, rationale)
        }
    }
    pub fn selected(mut self, root: &'static str) -> Self {
        self.model_assessment = Some(ModelAssessment::FullSelected(root));
        self
    }
    pub fn diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }
    pub fn enum_rejection(
        mut self,
        sample: Location,
        source: Location,
        alternatives: usize,
    ) -> Self {
        // Each enumerated literal becomes an effective const with its original token as origin.
        for index in 0..alternatives {
            self = self.diagnostic(Diagnostic {
                code: "sample.const",
                primary: sample.clone(),
                related: vec![
                    Location::new(
                        LocationKind::Effective,
                        source.source,
                        format!("{}/{index}/const", source.pointer),
                    ),
                    Location::new(
                        LocationKind::Source,
                        source.source,
                        format!("{}/{index}", source.pointer),
                    ),
                ],
            });
        }
        self
    }
}
pub struct Plan {
    pub device: &'static str,
    pub variant: &'static str,
    pub file: &'static str,
    pub exit: i32,
    pub source_keys: &'static [&'static str],
    pub failed_sources: &'static [&'static str],
    pub cases: Vec<Case>,
}
pub fn run(path: &Path, format: &str) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--plan"])
        .arg(path)
        .args(["--format", format, "--color", "never"])
        .output()
        .map_err(|e| e.to_string())
}
pub fn compare(plan: &Plan, output: &Output) -> Result<Value, String> {
    let label = format!("{}/{} ({})", plan.device, plan.variant, plan.file);
    if !output.stderr.is_empty() {
        return Err(format!(
            "{label}: CLI stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let exit = output
        .status
        .code()
        .ok_or_else(|| format!("{label}: host terminated the CLI"))?;
    let wire: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("{label}: malformed or unavailable report: {e}"))?;
    compare_report(plan, &wire, exit).map_err(|e| format!("{label}: {e}"))?;
    Ok(wire)
}

fn equal(actual: &Value, expected: impl Into<Value>, label: &str) -> Result<(), String> {
    let expected = expected.into();
    if *actual == expected {
        Ok(())
    } else {
        Err(format!("{label}: expected {expected}, observed {actual}"))
    }
}

fn array<'a>(value: &'a Value, label: &str) -> Result<&'a [Value], String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{label}: missing or malformed array"))
}

fn location_matches(actual: &Value, expected: &Location, sources: &BTreeMap<&str, u64>) -> bool {
    let (kind, identity) = expected.kind.wire();
    actual["kind"] == kind
        && actual["pointer"] == expected.pointer
        && sources
            .get(expected.source)
            .is_some_and(|id| actual[identity].as_u64() == Some(*id))
}

pub fn compare_report(plan: &Plan, wire: &Value, exit: i32) -> Result<(), String> {
    equal(&wire["schema"], "esdieff.check-run", "run schema")?;
    equal(&wire["version"], "1.1", "run version")?;
    equal(&Value::from(exit), plan.exit, "CLI exit")?;
    let mut expected = BTreeMap::new();
    for case in &plan.cases {
        if case.rationale.trim().is_empty() {
            return Err(format!("{}: missing independent rationale", case.name));
        }
        if expected.insert(case.name, case).is_some() {
            return Err(format!("{}: duplicate expectation", case.name));
        }
    }
    let cases = array(&wire["cases"], "cases")?;
    let mut observed = BTreeSet::new();
    for case in cases {
        let name = case["name"]
            .as_str()
            .ok_or("case identity is missing or malformed")?;
        if !observed.insert(name) {
            return Err(format!("{name}: duplicate observed case identity"));
        }
        if !expected.contains_key(name) {
            return Err(format!(
                "{name}: missing expectation for unexpected observed case"
            ));
        }
    }
    for name in expected.keys() {
        if !observed.contains(name) {
            return Err(format!("{name}: unused expectation"));
        }
    }
    let mut sources = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for row in array(&wire["sources"], "sources")? {
        let key = row["key"].as_str().ok_or("source key is missing")?;
        if !plan.source_keys.contains(&key) {
            return Err(format!("{key}: unexpected source acquisition attempt"));
        }
        let id = row["id"].as_u64().ok_or("source id is missing")?;
        if sources.insert(key, id).is_some() || !ids.insert(id) {
            return Err(format!("{key}: duplicate source identity"));
        }
        let failed = plan.failed_sources.contains(&key);
        equal(
            &row["acquisition"],
            if failed { "failed" } else { "acquired" },
            &format!("{key}: source acquisition"),
        )?;
        equal(
            &row["content_complete"],
            !failed,
            &format!("{key}: source bytes"),
        )?;
        equal(
            &row["decode"],
            if failed { "unavailable" } else { "usable" },
            &format!("{key}: source decoding"),
        )?;
    }
    for key in plan.source_keys {
        if !sources.contains_key(key) {
            return Err(format!("{key}: missing source acquisition attempt"));
        }
    }
    for actual in cases {
        let name = actual["name"].as_str().expect("validated case identity");
        let case = expected[name];
        let label = format!("{}/{}/{name}", plan.device, plan.variant);
        let state = match case.state {
            State::Report => "report",
        };
        equal(&actual["state"], state, &format!("{label}: state"))?;
        let operation = if case.admission.is_some() {
            "sample"
        } else if case.classification.is_some() {
            "diff"
        } else {
            "check"
        };
        equal(
            &actual["operation"],
            operation,
            &format!("{label}: operation"),
        )?;
        equal(
            &actual["outcome"]["disposition"],
            case.disposition.text(),
            &format!("{label}: disposition ({})", case.rationale),
        )?;
        equal(
            &actual["outcome"]["complete"],
            case.complete,
            &format!("{label}: completion ({})", case.rationale),
        )?;
        if let Some(admission) = case.admission {
            equal(
                &actual["admission"],
                admission.text(),
                &format!("{label}: admission ({})", case.rationale),
            )?;
        } else if actual.get("admission").is_some() {
            return Err(format!("{label}: unexpected sample admission"));
        }
        if let Some(classification) = case.classification {
            equal(
                &actual["classification"],
                classification.text(),
                &format!("{label}: classification ({})", case.rationale),
            )?;
        } else if actual.get("classification").is_some() {
            return Err(format!("{label}: unexpected comparison classification"));
        }
        let (assessment, inventory, total) = match case.model_assessment {
            Some(ModelAssessment::InitialCatalogue) => {
                equal(
                    &actual["request"]["scope"]["kind"],
                    "catalogue",
                    &format!("{label}: scope"),
                )?;
                equal(
                    &actual["request"]["scope"]["roots"],
                    serde_json::json!([]),
                    &format!("{label}: roots"),
                )?;
                (Some("esdieff.initial/1"), Some("esdieff.initial/1"), 8)
            }
            Some(ModelAssessment::FullSelected(root)) => {
                let id = sources
                    .get(root)
                    .ok_or_else(|| format!("{label}: missing selected root {root}"))?;
                equal(
                    &actual["request"]["scope"]["kind"],
                    "selected",
                    &format!("{label}: scope"),
                )?;
                equal(
                    &actual["request"]["scope"]["roots"],
                    serde_json::json!([id]),
                    &format!("{label}: roots"),
                )?;
                (Some("rfc9880/base-1"), Some("esdieff.full-model/1"), 13)
            }
            None if case.admission.is_some() => {
                let selector = &actual["request"]["selector"];
                if !selector["kind"].is_string()
                    || !selector["pointer"].is_string()
                    || !selector["document"].is_u64()
                    || !actual["request"]["sample"].is_u64()
                {
                    return Err(format!("{label}: malformed sample request"));
                }
                (Some("esdieff.sample/1"), Some("esdieff.sample/1"), 9)
            }
            None if case.classification.is_some() => {
                equal(
                    &actual["request"]["comparison"],
                    "esdieff.revision-comparison/2",
                    &format!("{label}: comparison"),
                )?;
                // A comparison requests no model assessment and counts itself once.
                (None, None, 1)
            }
            None => return Err(format!("{label}: missing expected model assessment")),
        };
        equal(
            &actual["request"]["assessment"],
            assessment,
            &format!("{label}: assessment"),
        )?;
        equal(
            &actual["request"]["inventory"],
            inventory,
            &format!("{label}: inventory"),
        )?;
        let report = actual
            .get("report")
            .filter(|v| v.is_object())
            .ok_or_else(|| format!("{label}: native report unavailable"))?;
        equal(
            &report["schema"],
            "esdieff.report",
            &format!("{label}: native schema"),
        )?;
        equal(
            &report["version"],
            match operation {
                "sample" => "1.1",
                "diff" => "1.5",
                _ => "1.0",
            },
            &format!("{label}: native version"),
        )?;
        equal(
            &report["operation"],
            operation,
            &format!("{label}: native operation"),
        )?;
        for field in ["request", "outcome", "baseline", "policy"] {
            if !actual[field].is_object() {
                return Err(format!("{label}: malformed {field}"));
            }
            equal(
                &report[field],
                actual[field].clone(),
                &format!("{label}: native {field}"),
            )?;
        }
        if let Some(admission) = case.admission {
            equal(
                &report["result"]["admission"],
                admission.text(),
                &format!("{label}: native admission"),
            )?;
        }
        if let Some(classification) = case.classification {
            equal(
                &report["result"]["outcome"],
                classification.text(),
                &format!("{label}: native classification"),
            )?;
        }
        let requested = &report["coverage"]["requested"];
        equal(
            &requested["inventory"],
            inventory,
            &format!("{label}: coverage inventory"),
        )?;
        equal(
            &requested["total"],
            total,
            &format!("{label}: coverage total"),
        )?;
        let mut statuses = Vec::new();
        for field in ["assessed", "not_applicable", "unassessed", "undetermined"] {
            let count = requested[field]
                .as_u64()
                .filter(|v| *v <= total as u64)
                .ok_or_else(|| format!("{label}: malformed coverage {field}"))?;
            statuses.push(count);
        }
        equal(
            &Value::from(statuses.iter().sum::<u64>()),
            total,
            &format!("{label}: coverage accounting"),
        )?;
        equal(
            &Value::from(statuses[2] + statuses[3] == 0),
            case.complete,
            &format!("{label}: coverage completion"),
        )?;
        equal(
            &report["diagnostics"]["truncated"],
            false,
            &format!("{label}: diagnostic availability"),
        )?;
        let diagnostics = array(
            &report["diagnostics"]["items"],
            &format!("{label}: diagnostics"),
        )?;
        for diagnostic in &case.diagnostics {
            if !diagnostics.iter().any(|row| {
                row["code"] == diagnostic.code
                    && location_matches(&row["primary"], &diagnostic.primary, &sources)
                    && row["related"].as_array().is_some_and(|related| {
                        diagnostic.related.iter().all(|location| {
                            related
                                .iter()
                                .any(|row| location_matches(row, location, &sources))
                        })
                    })
            }) {
                return Err(format!(
                    "{label}: missing diagnostic {} with expected source/effective/sample pointers",
                    diagnostic.code
                ));
            }
        }
    }
    let count =
        |predicate: fn(&Case) -> bool| plan.cases.iter().filter(|c| predicate(c)).count() as u64;
    let complete = count(|c| c.complete);
    for (field, value) in [
        ("total", plan.cases.len() as u64),
        (
            "passed",
            count(|c| matches!(c.disposition, Disposition::Passed)),
        ),
        (
            "failed",
            count(|c| matches!(c.disposition, Disposition::Failed)),
        ),
        (
            "undetermined",
            count(|c| matches!(c.disposition, Disposition::Undetermined)),
        ),
        ("request_errors", 0),
        ("omitted_reports", 0),
        ("complete_cases", complete),
        ("incomplete_cases", plan.cases.len() as u64 - complete),
        (
            "admitted",
            count(|c| matches!(c.admission, Some(Admission::Admitted))),
        ),
        (
            "rejected",
            count(|c| matches!(c.admission, Some(Admission::Rejected))),
        ),
        (
            "admission_undetermined",
            count(|c| matches!(c.admission, Some(Admission::Undetermined))),
        ),
    ] {
        equal(&wire["summary"][field], value, &format!("summary {field}"))?;
    }
    equal(
        &wire["summary"]["complete"],
        complete == plan.cases.len() as u64,
        "summary completion",
    )?;
    equal(
        &wire["summary"]["verdict"],
        match plan.exit {
            0 => "passed",
            1 => "failed",
            3 => "undetermined",
            _ => return Err("invalid expected CLI exit".into()),
        },
        "summary verdict",
    )?;
    Ok(())
}

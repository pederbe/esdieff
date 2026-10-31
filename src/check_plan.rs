//! Saved Check plans orchestrate ordinary model checks, sample checks and Revision
//! comparisons, following ADR 0004: Supplied catalogues and checking scope, ADR 0009:
//! Bounded processing and reporting, ADR 0013: Shared workflow and report guarantees,
//! ADR 0039: Client-side revision comparison and ADR 0040: Acknowledged revision
//! differences.
use crate::{
    assessment,
    cli_style::{ColorMode, Role},
    sample_kind, usage,
};
use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, Classification, ComparisonRequest,
    DifferenceAcknowledgements, Disposition, Field, HumanStyle, InspectRequest, JsonKind, JsonView,
    Limits, Report, RevisionComparison, SampleKind, SampleRequest, SourceId, SourceInput, View,
};
use serde_json::{json, value::RawValue};
use std::{
    collections::{BTreeMap, HashSet},
    fmt::Write as _,
    io::Write,
    path::{Path, PathBuf},
};

type Error = (u8, String);
const PLAN_BYTES: usize = 1024 * 1024;
const REPORT_BYTES: usize = 8 * 1024 * 1024;
// Native String growth can retain twice the serialized byte ceiling. This
// independent transient allowance is charged before calling either renderer.
const SERIALIZATION_BYTES: usize = 2 * REPORT_BYTES;
const CASE_BYTES: usize = 16 * 1024;
const CASE_STAGING_BYTES: usize = 128 * 1024;
const SOURCE_BYTES: usize = 4096;
const FIXED_BYTES: usize = 16 * 1024;
const ACKNOWLEDGED_BYTES: usize = 4 * 1024 * 1024;

/// A literal file: a plan source, or one revision's document in a diff case.
struct Source<'a> {
    key: &'a str,
    path: PathBuf,
    sample: bool,
}
impl Source<'_> {
    /// Its Source input, labelled with the resolved path when that is UTF-8 and at most
    /// 512 bytes.
    fn input(&self) -> SourceInput<'_> {
        let mut input = SourceInput::file(self.key, &self.path);
        if self.sample {
            input = input.sample();
        }
        if self.path.as_os_str().as_encoded_bytes().len() <= 512 {
            // Plan paths are UTF-8 beneath a possibly non-UTF-8 CLI parent.
            // A lossy presentation label is omitted instead of changing identity.
            if let Some(label) = self.path.to_str() {
                return input.label(label);
            }
        }
        input.omit_label()
    }
}
struct Case<'a> {
    name: &'a str,
    operation: Operation<'a>,
    strict: bool,
}
enum Operation<'a> {
    Check {
        roots: Vec<usize>,
        assessment: Assessment,
    },
    Sample {
        document: usize,
        kind: SampleKind,
        pointer: &'a str,
        sample: usize,
    },
    Diff {
        perspective: esdieff::Perspective,
        /// The earlier and later revisions' documents, by source key.
        documents: [Vec<Source<'a>>; 2],
        acknowledged: Option<DifferenceAcknowledgements>,
    },
}
impl Operation<'_> {
    fn name(&self) -> &'static str {
        match self {
            Self::Check { .. } => "check",
            Self::Sample { .. } => "sample",
            Self::Diff { .. } => "diff",
        }
    }
    /// Run the case. Checks and samples use the plan Catalogue. A diff case acquires each
    /// revision only now, into `revisions`, so its documents never join the plan Catalogue.
    fn execute<'a>(
        &'a self,
        catalogue: &'a Catalogue,
        ids: &[SourceId],
        strict: bool,
        revisions: &'a mut Option<[Catalogue; 2]>,
    ) -> Result<Native<'a>, &'static str> {
        match self {
            Self::Check { roots, assessment } => {
                let mut request = CheckRequest::new()
                    .assessment(*assessment)
                    .strict_warnings(strict);
                for root in roots {
                    request = request.root(ids[*root]);
                }
                catalogue.check(request).map(Native::report)
            }
            Self::Sample {
                document,
                kind,
                pointer,
                sample,
            } => catalogue
                .check_sample(
                    SampleRequest::new(ids[*document], *kind, pointer)
                        .sample(ids[*sample])
                        .strict_warnings(strict),
                )
                .map(Native::report),
            Self::Diff {
                perspective,
                documents: [earlier, later],
                acknowledged,
            } => {
                let acquire = |documents: &[Source<'_>]| {
                    Catalogue::take_inputs(documents.iter().map(Source::input).collect())
                };
                let [earlier, later] = &*revisions.insert([acquire(earlier)?, acquire(later)?]);
                let mut request = ComparisonRequest::new(earlier, later).perspective(*perspective);
                if let Some(acknowledged) = acknowledged {
                    request = request.acknowledgements(acknowledged);
                }
                request.compare().map(Native::comparison)
            }
        }
    }
}

/// A case's native result: a model or sample report, or a Revision comparison.
enum Native<'a> {
    Report(Box<Report<'a>>),
    Comparison(Box<RevisionComparison<'a>>),
}
impl<'a> Native<'a> {
    fn report(report: Report<'a>) -> Self {
        Self::Report(Box::new(report))
    }
    fn comparison(comparison: RevisionComparison<'a>) -> Self {
        Self::Comparison(Box::new(comparison))
    }
    fn disposition(&self) -> Disposition {
        match self {
            Self::Report(report) => report.outcome().disposition(),
            Self::Comparison(comparison) => comparison.disposition(),
        }
    }
    fn complete(&self) -> bool {
        match self {
            Self::Report(report) => report.outcome().complete(),
            Self::Comparison(comparison) => comparison.complete(),
        }
    }
    fn tag(&self) -> Option<Tag> {
        match self {
            Self::Report(report) => report
                .sample_result()
                .map(|sample| Tag::Admission(sample.admission())),
            Self::Comparison(comparison) => Some(Tag::Classification(comparison.outcome())),
        }
    }
    fn to_json(&self) -> String {
        match self {
            Self::Report(report) => report.to_json(),
            Self::Comparison(comparison) => comparison.to_json(),
        }
    }
    fn to_human(&self, style: HumanStyle) -> String {
        match self {
            Self::Report(report) => report.to_human_with_style(style),
            Self::Comparison(comparison) => comparison.to_human_with_style(style),
        }
    }
}
/// The member a case record keeps beside the native fields.
enum Tag {
    /// A sample's admission.
    Admission(Admission),
    /// A comparison's classification, which acknowledgements leave unchanged.
    Classification(Classification),
}
impl Tag {
    /// The record member, its human heading and its value.
    fn parts(&self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Admission(admission) => ("admission", "Admission", admission.as_str()),
            Self::Classification(classification) => {
                ("classification", "Classification", classification.as_str())
            }
        }
    }
}
struct Plan<'a> {
    sources: Vec<Source<'a>>,
    cases: Vec<Case<'a>>,
    reserve: usize,
}

fn text<'a>(node: JsonView<'a>, member: &str) -> Result<&'a str, Error> {
    node.get(member)
        .require()
        .and_then(|n| n.text().require())
        .map_err(usage)
}
fn object(node: JsonView<'_>, allowed: &[&str]) -> Result<(), Error> {
    if node.kind() != JsonKind::Map {
        return Err(usage("expected plan object"));
    }
    for child in node.children() {
        if !allowed.contains(&child.member_name().require().map_err(usage)?) {
            return Err(usage("unknown plan member"));
        }
    }
    Ok(())
}
fn array(node: JsonView<'_>, max: usize) -> Result<JsonView<'_>, Error> {
    if node.kind() != JsonKind::Array
        || node.children().count() == 0
        || node.children().count() > max
    {
        return Err(usage("plan array is empty or exceeds its count ceiling"));
    }
    Ok(node)
}
/// One diff revision: an object naming 1 through 256 documents by source key.
fn revision(node: JsonView<'_>) -> Result<JsonView<'_>, Error> {
    if node.kind() != JsonKind::Map || node.children().count() == 0 || node.children().count() > 256
    {
        return Err(usage(
            "each diff revision is an object naming 1 through 256 documents",
        ));
    }
    Ok(node)
}
/// The control metadata bytes of one literal path: its plan text, its resolved form and a
/// presentation label of at most 512 bytes.
fn path_bytes(base: &Path, file: &str) -> Result<usize, Error> {
    if file.is_empty() {
        return Err(usage("empty plan path"));
    }
    let resolved = if Path::new(file).is_absolute() {
        file.len()
    } else {
        base.as_os_str().as_encoded_bytes().len() + 1 + file.len()
    };
    Ok(file.len() + resolved + resolved.min(512))
}
/// Read and decode a diff case's acknowledgement file, charging its bytes to the plan's
/// 4 MiB for acknowledgement files.
fn acknowledgements(
    name: &str,
    path: &Path,
    remaining: &mut usize,
) -> Result<DifferenceAcknowledgements, Error> {
    let refused = |reason: &str| {
        usage(&format!(
            "cannot read acknowledgements of diff case {name:?}: {reason}"
        ))
    };
    let (acknowledgements, size) =
        crate::read_acknowledgements(path).map_err(|reason| refused(&reason))?;
    *remaining = remaining
        .checked_sub(size)
        .ok_or_else(|| refused("acknowledgement files exceed 4 MiB per plan"))?;
    Ok(acknowledgements)
}
fn identity(value: &str) -> Result<(), Error> {
    if value.is_empty() || value.len() > 256 {
        Err(usage(
            "plan identities must contain 1 through 256 UTF-8 bytes",
        ))
    } else {
        Ok(())
    }
}
fn capable(node: JsonView<'_>) -> bool {
    node.capability_complete() && node.children().all(capable)
}
fn metadata(total: &mut usize, size: usize) -> Result<(), Error> {
    *total = total
        .checked_add(size)
        .ok_or_else(|| usage("plan metadata overflow"))?;
    if *total > PLAN_BYTES {
        Err(usage("plan control metadata exceeds whole-request budget"))
    } else {
        Ok(())
    }
}
fn alias<'a>(sources: JsonView<'a>, key: &str, role: &str) -> Result<usize, Error> {
    let (index, source) = sources
        .children()
        .enumerate()
        .find(|(_, row)| text(*row, "key").ok() == Some(key))
        .ok_or_else(|| usage("unknown source key"))?;
    if text(source, "role")? != role {
        return Err(usage("source alias has the wrong role"));
    }
    Ok(index)
}
fn pointer(value: &str) -> Result<(), Error> {
    // The ordinary selector contract permits an empty root and RFC 6901 escapes.
    // This checks configuration syntax only. The native operation owns selection.
    if value.len() > 8192 || (!value.is_empty() && !value.starts_with('/')) {
        return Err(usage("invalid or oversized JSON pointer selector"));
    }
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return Err(usage("invalid JSON pointer escape"));
        }
    }
    Ok(())
}
fn decode<'a>(root: JsonView<'a>, path: &Path) -> Result<Plan<'a>, Error> {
    if !capable(root) {
        return Err(usage("plan JSON capability unavailable"));
    }
    object(root, &["schema", "version", "sources", "cases"])?;
    if text(root, "schema")? != "esdieff.check-plan" || text(root, "version")? != "1" {
        return Err(usage("unknown check plan schema or version"));
    }
    let base = path
        .parent()
        .ok_or_else(|| usage("plan has no parent directory"))?;
    let source_rows = array(root.get("sources").require().map_err(usage)?, 256)?;
    let case_rows = array(root.get("cases").require().map_err(usage)?, 64)?;
    // Admission uses borrowed decoded text. Resolved paths, control vectors and
    // the output buffer are allocated only after the entire envelope fits.
    let mut source_keys = HashSet::new();
    let mut names = HashSet::new();
    let mut control = path.as_os_str().as_encoded_bytes().len();
    metadata(&mut control, 0)?;
    let mut models = 0;
    for row in source_rows.children() {
        object(row, &["key", "path", "role"])?;
        let key = text(row, "key")?;
        identity(key)?;
        if !source_keys.insert(key) {
            return Err(usage("duplicate source key"));
        }
        let path_size = path_bytes(base, text(row, "path")?)?;
        match text(row, "role")? {
            "model" => models += 1,
            "sample" => (),
            _ => return Err(usage("unknown source role")),
        }
        metadata(&mut control, key.len() + path_size)?;
    }
    if models == 0 {
        return Err(usage("plan requires a model source"));
    }
    // Each diff document is reserved as a source is, beside the plan sources.
    let mut documents = 0;
    for row in case_rows.children() {
        let name = text(row, "name")?;
        identity(name)?;
        if !names.insert(name) {
            return Err(usage("duplicate case name"));
        }
        strict(row)?;
        metadata(&mut control, name.len())?;
        match text(row, "operation")? {
            "check" => {
                object(
                    row,
                    &[
                        "name",
                        "operation",
                        "scope",
                        "assessment",
                        "strict_warnings",
                    ],
                )?;
                let profile = profile(row)?;
                assessment(Some(profile))?;
                metadata(&mut control, profile.len())?;
                let scope = row.get("scope").require().map_err(usage)?;
                match text(scope, "kind")? {
                    "catalogue" => object(scope, &["kind"])?,
                    "selected" => {
                        object(scope, &["kind", "roots"])?;
                        let mut seen = HashSet::new();
                        for root in
                            array(scope.get("roots").require().map_err(usage)?, 256)?.children()
                        {
                            let key = root.text().require().map_err(usage)?;
                            if !seen.insert(key) {
                                return Err(usage("duplicate selected root"));
                            }
                            alias(source_rows, key, "model")?;
                            metadata(&mut control, key.len())?;
                        }
                    }
                    _ => return Err(usage("unknown check scope")),
                }
            }
            "sample" => {
                object(
                    row,
                    &[
                        "name",
                        "operation",
                        "document",
                        "kind",
                        "pointer",
                        "sample",
                        "strict_warnings",
                    ],
                )?;
                let document = text(row, "document")?;
                let sample = text(row, "sample")?;
                alias(source_rows, document, "model")?;
                alias(source_rows, sample, "sample")?;
                sample_kind(text(row, "kind")?)?;
                let selector = text(row, "pointer")?;
                pointer(selector)?;
                metadata(&mut control, document.len() + sample.len() + selector.len())?;
            }
            "diff" => {
                object(
                    row,
                    &[
                        "name",
                        "operation",
                        "earlier",
                        "later",
                        "acknowledged",
                        "perspective",
                    ],
                )?;
                if let Field::Present(value) = row.get("perspective") {
                    crate::diff_cli::parse_perspective(value.text().require().map_err(usage)?)?;
                }
                for member in ["earlier", "later"] {
                    let side = row.get(member).require().map_err(usage)?;
                    for document in revision(side)?.children() {
                        let key = document.member_name().require().map_err(usage)?;
                        identity(key)?;
                        let path_size =
                            path_bytes(base, document.text().require().map_err(usage)?)?;
                        metadata(&mut control, key.len() + path_size)?;
                        documents += 1;
                    }
                }
                match row.get("acknowledged") {
                    Field::Absent => (),
                    Field::Present(file) => {
                        let path_size = path_bytes(base, file.text().require().map_err(usage)?)?;
                        metadata(&mut control, path_size)?;
                    }
                    _ => return Err(usage("unavailable acknowledgement path")),
                }
            }
            _ => return Err(usage("unknown case operation")),
        }
    }
    // Six bytes per metadata byte covers JSON escaping, including repeated
    // identities. Fixed case space covers native scope IDs, baseline/knowledge,
    // policy, authoritative outcome, coverage disclosure and an omission record.
    let reserve = control * 6
        + (source_rows.children().count() + documents) * SOURCE_BYTES
        + case_rows.children().count() * CASE_BYTES
        + FIXED_BYTES;
    if reserve > REPORT_BYTES {
        return Err(usage("run report cannot retain its mandatory envelope"));
    }
    let sources: Vec<_> = source_rows
        .children()
        .map(|row| {
            let key = text(row, "key").expect("validated source key");
            let file = text(row, "path").expect("validated source path");
            Source {
                key,
                path: base.join(file),
                sample: text(row, "role").expect("validated role") == "sample",
            }
        })
        .collect();
    // Acknowledgement files are read and decoded last, before any source is acquired.
    let mut acknowledged = ACKNOWLEDGED_BYTES;
    let cases = case_rows
        .children()
        .map(|row| {
            let name = text(row, "name").expect("case name");
            let operation = match text(row, "operation").expect("operation") {
                "diff" => {
                    let side = |revision| {
                        let side = row.get(revision).require().expect("validated side");
                        side.children()
                            .map(|document| Source {
                                key: document.member_name().require().expect("validated key"),
                                path: base.join(document.text().require().expect("validated path")),
                                sample: false,
                            })
                            .collect()
                    };
                    let file = row.get("acknowledged").present();
                    Operation::Diff {
                        perspective: row
                            .get("perspective")
                            .present()
                            .map(|value| {
                                crate::diff_cli::parse_perspective(
                                    value.text().require().expect("validated perspective"),
                                )
                            })
                            .transpose()?
                            .unwrap_or_default(),
                        documents: [side("earlier"), side("later")],
                        acknowledged: file
                            .map(|file| {
                                let file = file.text().require().expect("validated path");
                                acknowledgements(name, &base.join(file), &mut acknowledged)
                            })
                            .transpose()?,
                    }
                }
                "sample" => Operation::Sample {
                    document: alias(
                        source_rows,
                        text(row, "document").expect("document"),
                        "model",
                    )
                    .expect("validated alias"),
                    kind: sample_kind(text(row, "kind").expect("kind")).expect("validated kind"),
                    pointer: text(row, "pointer").expect("pointer"),
                    sample: alias(source_rows, text(row, "sample").expect("sample"), "sample")
                        .expect("validated alias"),
                },
                _ => {
                    let scope = row.get("scope").require().expect("validated scope");
                    let roots = if text(scope, "kind").expect("scope kind") == "selected" {
                        scope
                            .get("roots")
                            .require()
                            .expect("roots")
                            .children()
                            .map(|root| {
                                alias(
                                    source_rows,
                                    root.text().require().expect("root key"),
                                    "model",
                                )
                                .expect("validated alias")
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    Operation::Check {
                        roots,
                        assessment: assessment(Some(profile(row).expect("profile")))
                            .expect("assessment"),
                    }
                }
            };
            Ok(Case {
                name,
                operation,
                strict: strict(row).expect("strict warnings"),
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(Plan {
        sources,
        cases,
        reserve,
    })
}
fn profile(row: JsonView<'_>) -> Result<&str, Error> {
    match row.get("assessment") {
        Field::Absent => Ok("initial-v1"),
        Field::Present(value) => value.text().require().map_err(usage),
        _ => Err(usage("unavailable assessment")),
    }
}
fn strict(row: JsonView<'_>) -> Result<bool, Error> {
    match row.get("strict_warnings") {
        Field::Absent => Ok(false),
        Field::Present(value) => value.boolean().require().map_err(usage),
        _ => Err(usage("unavailable strict warning policy")),
    }
}

#[derive(Default)]
struct Summary {
    total: usize,
    passed: usize,
    failed: usize,
    undetermined: usize,
    complete: usize,
    omitted: usize,
    request_errors: usize,
    admitted: usize,
    rejected: usize,
    admission_undetermined: usize,
}
impl Summary {
    fn exit(&self) -> u8 {
        if self.request_errors > 0 {
            2
        } else if self.failed > 0 {
            1
        } else if self.undetermined > 0 {
            3
        } else {
            0
        }
    }
    fn wire(&self) -> String {
        serde_json::to_string(&json!({"total":self.total,"passed":self.passed,"failed":self.failed,"undetermined":self.undetermined,"request_errors":self.request_errors,"complete_cases":self.complete,"incomplete_cases":self.total-self.complete,"omitted_reports":self.omitted,"admitted":self.admitted,"rejected":self.rejected,"admission_undetermined":self.admission_undetermined,"complete":self.complete==self.total,"verdict":match self.exit(){2=>"invalid",1=>"failed",3=>"undetermined",_=>"passed"}})).expect("fixed summary")
    }
}
fn host(reason: impl Into<String>) -> Error {
    (4, reason.into())
}
fn encode_text(value: &str) -> String {
    serde_json::to_string(value).expect("text encoding")
}
fn native_json(report: &Native<'_>) -> Result<String, Error> {
    let native = report.to_json();
    if native.len() > REPORT_BYTES || native.capacity() > SERIALIZATION_BYTES {
        return Err(host("native report exceeds serialization allowance"));
    }
    Ok(native)
}
fn native_fields(native: &str) -> Result<BTreeMap<&str, &RawValue>, Error> {
    serde_json::from_str(native).map_err(|e| host(format!("cannot retain native envelope: {e}")))
}
fn request_error(
    case: &Case<'_>,
    ids: &[SourceId],
    index: usize,
    message: &str,
) -> Result<String, Error> {
    // Library request failures are fixed text, independent of source content.
    // The admitted fixed case space covers the DTO, encoding and plan location.
    if message.len() > 1024 {
        return Err(host("request error exceeds reserved message space"));
    }
    let request = match &case.operation {
        Operation::Sample {
            document,
            kind,
            pointer,
            sample,
        } => {
            // Two escaped selector copies plus fixed DTO/map and encoding
            // space fit the separate staging allowance before allocation.
            if 12 * pointer.len() + CASE_BYTES > CASE_STAGING_BYTES {
                return Err(host("request error exceeds case staging allowance"));
            }
            json!({
            "selector":{"document":ids[*document].get(), "kind":kind.as_str(), "pointer":pointer},
            "sample":ids[*sample].get(), "strict_warnings":case.strict
            })
        }
        Operation::Check { .. } => return Err(host("cannot execute admitted model check")),
        // Admission charges each diff document at least the metadata its Catalogue admits.
        // The slack this leaves in the plan reserve exceeds the acknowledgement reserve, so
        // both revisions' native envelopes fit once admitted.
        Operation::Diff { .. } => {
            return Err(host("cannot run admitted revision comparison"));
        }
    };
    Ok(serde_json::to_string(&json!({"name":case.name,"operation":case.operation.name(),"request":request,"state":"request_error",
        "request_error":{"code":"sample.request","message":message,"location":{"kind":"plan","pointer":format!("/cases/{index}")}}
    })).expect("admitted request error"))
}

pub(crate) fn run(path: &Path, json_output: bool, color: ColorMode) -> Result<u8, Error> {
    // Making the supplied path absolute, without canonicalizing, preserves the
    // parent of a symlink rather than switching to the target's directory.
    let path =
        std::path::absolute(path).map_err(|e| usage(&format!("cannot resolve plan path: {e}")))?;
    let configuration = Catalogue::take_inputs_with_limits(
        vec![SourceInput::file("plan", &path).omit_label().sample()],
        Limits::default().document_bytes(PLAN_BYTES),
    )
    .map_err(usage)?;
    let source = configuration.sources().next().expect("plan source");
    if !source.content_complete() {
        return Err(usage("plan unreadable or exceeds 1 MiB"));
    }
    let plan = decode(source.json().require().map_err(usage)?, &path)?;
    let inputs = plan.sources.iter().map(Source::input).collect();
    let catalogue = Catalogue::take_inputs(inputs).map_err(usage)?;
    let ids: Vec<_> = catalogue.sources().map(|s| s.id()).collect();
    // Additional ownership is finite: the native plan decoder keeps its normal
    // 64 MiB parse allowance, control is <=1 MiB plus <=64/256 bounded slots,
    // aggregate output reserves 8 MiB, and one native serialization has a
    // separate 16 MiB capacity allowance for <=8 MiB of serialized content.
    // One mandatory case DTO/encoding has a separate 128 KiB staging allowance.
    // Human rendering begins only after dropping that JSON serialization.
    let mut output = String::with_capacity(REPORT_BYTES);
    let mut detail = REPORT_BYTES - plan.reserve;
    let mut summary = Summary::default();
    let identity = format!(
        "{{\"path\":{},\"schema\":\"esdieff.check-plan\",\"version\":\"1\"}}",
        encode_text(&path.to_string_lossy())
    );
    // Source-view inspection supplies the existing acquisition/decode metadata
    // even when every requested sample selector fails. It evaluates no SDF
    // meaning and is released before any case begins. Its outcome is not a case.
    {
        let source_report = catalogue
            .inspect(InspectRequest::new(ids[0], "").view(View::Source))
            .map_err(|e| host(format!("cannot present acquired sources: {e}")))?;
        let native = native_json(&Native::report(source_report))?;
        let fields = native_fields(&native)?;
        if json_output {
            output.push_str("{\"schema\":\"esdieff.check-run\",\"version\":\"1.1\",\"plan\":");
            output.push_str(&identity);
            output.push_str(",\"sources\":");
            output.push_str(fields["sources"].get());
            output.push_str(",\"cases\":[");
        } else {
            output.push_str("Esdieff check run\nPlan: ");
            output.push_str(&identity);
            output.push_str("\nSources: ");
            output.push_str(fields["sources"].get());
            output.push('\n');
        }
    }
    for (index, case) in plan.cases.iter().enumerate() {
        summary.total += 1;
        if json_output && index > 0 {
            output.push(',');
        }
        let mut revisions = None;
        let report = match case
            .operation
            .execute(&catalogue, &ids, case.strict, &mut revisions)
        {
            Ok(report) => report,
            Err(message) => {
                let record = request_error(case, &ids, index, message)?;
                summary.request_errors += 1;
                if !json_output {
                    write!(
                        output,
                        "\n{} ",
                        color.stdout().span(Role::Failure, "Case request error:")
                    )
                    .expect("String write");
                }
                output.push_str(&record);
                if !json_output {
                    output.push('\n');
                }
                continue;
            }
        };
        let (disposition, complete) = (report.disposition(), report.complete());
        summary.complete += usize::from(complete);
        match disposition.exit_code() {
            0 => summary.passed += 1,
            1 => summary.failed += 1,
            _ => summary.undetermined += 1,
        }
        let tag = report.tag();
        match tag {
            Some(Tag::Admission(Admission::Admitted)) => summary.admitted += 1,
            Some(Tag::Admission(Admission::Rejected)) => summary.rejected += 1,
            Some(Tag::Admission(Admission::Undetermined)) => summary.admission_undetermined += 1,
            _ => (),
        }
        let tag = tag.as_ref().map(Tag::parts);
        let native = native_json(&report)?;
        // Borrow only the native envelope fields. RawValue never rounds embedded
        // numbers or copies the full report into an aggregation JSON graph.
        let fields = native_fields(&native)?;
        if json_output {
            write!(
                output,
                "{{\"name\":{},\"operation\":{},\"state\":\"report\"",
                encode_text(case.name),
                encode_text(case.operation.name())
            )
            .expect("String write");
            for key in ["request", "outcome", "baseline", "policy"] {
                write!(output, ",\"{key}\":{}", fields[key].get()).expect("String write");
            }
            if let Some((member, _, value)) = tag {
                write!(output, ",\"{member}\":{}", encode_text(value)).expect("String write");
            }
            if native.len() + 12 <= detail {
                detail -= native.len() + 12;
                output.push_str(",\"report\":");
                output.push_str(&native);
            } else {
                summary.omitted += 1;
                output.push_str(",\"report_omission\":{\"reason\":\"run-report-budget\"}");
            }
            output.push('}');
        } else {
            let coverage: BTreeMap<&str, &RawValue> =
                serde_json::from_str(fields["coverage"].get())
                    .map_err(|e| host(format!("cannot disclose native coverage: {e}")))?;
            let broader: BTreeMap<&str, &RawValue> =
                serde_json::from_str(coverage["broader"].get())
                    .map_err(|e| host(format!("cannot disclose broader coverage: {e}")))?;
            output.push_str(&format!("\n{} {}\nOperation: {}\nScope/profile/selector: {}\nDisposition: {}\nComplete: {}\nOutcome: {}\nBroader RFC coverage: {}\nBaseline: {}\nPolicy: {}\n", color.stdout().span(Role::Heading, "Case:"), encode_text(case.name), case.operation.name(), fields["request"].get(), disposition.as_str(), complete, fields["outcome"].get(), broader["state"].get().trim_matches('"'), fields["baseline"].get(), fields["policy"].get()));
            if let Some((_, heading, value)) = tag {
                output.push_str(&format!("{heading}: {value}\n"));
            }
            drop(fields);
            drop(native);
            let human = report.to_human(if color.stdout().enabled() {
                HumanStyle::Ansi
            } else {
                HumanStyle::Plain
            });
            if human.len() > REPORT_BYTES || human.capacity() > SERIALIZATION_BYTES {
                return Err(host("native human report exceeds serialization allowance"));
            }
            if human.len() <= detail {
                detail -= human.len();
                output.push_str(&human);
            } else {
                summary.omitted += 1;
                output.push_str("Native report omitted: run-report-budget\n");
            }
        }
    }
    if json_output {
        output.push_str(&format!("],\"summary\":{}}}\n", summary.wire()));
    } else {
        output.push_str(&format!("\nCheck case summary: {}\n", summary.wire()));
    }
    if output.len() > REPORT_BYTES {
        return Err(host("run report exceeds admitted envelope"));
    }
    std::io::stdout()
        .lock()
        .write_all(output.as_bytes())
        .map_err(|e| host(format!("cannot emit requested run report: {e}")))?;
    Ok(summary.exit())
}

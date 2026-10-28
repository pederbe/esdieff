//! SARIF 2.1.0 logs project `check` reports and revision comparisons for code-scanning
//! annotations, following ADR 0043: SARIF and Markdown report projections and ADR 0009:
//! Bounded processing and reporting. A log renders only what its report retained, within the
//! same report byte limit, and counts what it leaves out.
use crate::comparison::{version, wording};
use crate::{
    Catalogue, Classification, Condition, DifferenceSide, JsonView, Report, ReportLocation,
    Revision, RevisionComparison, Severity, Source,
};
use serde_json::{Map, Value, json};
use std::{
    collections::HashMap,
    ops::Range,
    path::{Component, Path},
};

const SCHEMA: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";
/// The base of relative artifact URIs, which stands for the invocation directory.
const BASE: &str = "%SRCROOT%";
const CHECK_HELP: &str = "The Esdieff report contract describes condition codes, severities and bases under Outcomes, conditions and truncation. Each message explains its finding.";
const DIFF_HELP: &str = "The Esdieff revision comparison contract describes each classification and undetermined reason. Each message names the difference and its witness or reason.";

/// The log of a check report: one result per retained error, warning or notice condition
/// and one tool execution notification per retained incomplete condition.
pub(crate) fn check(report: &Report<'_>, catalogue: &Catalogue, directory: &Path) -> String {
    let diagnostics = report.diagnostics();
    let outcome = report.outcome();
    let mut log = Log::new(directory, catalogue.limits.report_bytes, CHECK_HELP);
    for condition in diagnostics.conditions() {
        let level = match condition.severity() {
            Severity::Error => Some("error"),
            Severity::Warning => Some("warning"),
            Severity::Notice => Some("note"),
            Severity::Incomplete => None,
        };
        // The physical location is the primary source location, or else the first related
        // one. An effective primary location becomes a logical location beside it.
        let primary = condition.primary().present();
        let related: Vec<_> = condition.related().collect();
        let promoted = match primary {
            Some(ReportLocation::Source(_) | ReportLocation::Sample(_)) => None,
            _ => related.iter().position(|location| {
                matches!(
                    location,
                    ReportLocation::Source(_) | ReportLocation::Sample(_)
                )
            }),
        };
        let mut place =
            primary.map_or_else(Place::default, |location| check_place(catalogue, &location));
        if let Some(index) = promoted {
            place.source = check_place(catalogue, &related[index]).source;
        }
        let item = Item {
            rule: condition.code().to_owned(),
            description: condition_description(condition),
            level: level.unwrap_or("warning"),
            message: condition.message().to_owned(),
            primary: place,
            related: related
                .iter()
                .enumerate()
                .filter(|(index, _)| Some(*index) != promoted)
                .map(|(_, location)| check_place(catalogue, location))
                .collect(),
            suppressed: None,
        };
        match level {
            Some(_) => log.results.push(item),
            None => log.notifications.push(item),
        }
    }
    // Errors first, so a byte limit drops notes before errors.
    log.results.sort_by_key(|item| match item.level {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    });
    let findings =
        u64::from(outcome.errors()) + u64::from(outcome.warnings()) + u64::from(outcome.notices());
    // Every condition the report established, retained or omitted, beyond its findings is
    // incomplete work.
    let conditions = diagnostics.conditions().len() as u64 + u64::from(diagnostics.omitted());
    log.render(
        outcome.disposition().as_str(),
        findings,
        conditions.saturating_sub(findings),
    )
}

/// A code's descriptor text: its severity and basis kind, which a code keeps.
fn condition_description(condition: &Condition) -> String {
    format!(
        "Esdieff {} condition {}, with basis kind {}.",
        condition.severity().as_str(),
        condition.code(),
        condition.basis().kind().as_str()
    )
}

fn check_place<'a>(catalogue: &'a Catalogue, location: &ReportLocation<'_>) -> Place<'a> {
    match location {
        ReportLocation::Source(location) | ReportLocation::Sample(location) => Place {
            source: catalogue
                .source(location.source_id())
                .map(|source| SourcePlace {
                    key: (0, source.id().get()),
                    source,
                    pointer: location.pointer().present().map(str::to_owned),
                    span: location.span(),
                    revision: None,
                }),
            ..Place::default()
        },
        ReportLocation::Effective(location) => Place {
            effective: location.pointer().present().map(|pointer| Logical {
                pointer: pointer.to_owned(),
                view: "effective",
                document: catalogue
                    .source(location.root_document())
                    .map_or_else(String::new, document),
                revision: None,
            }),
            ..Place::default()
        },
        _ => Place::default(),
    }
}

/// The log of a revision comparison: one `warning` result per version warning, then one result
/// per retained difference, breaking as `error`, undetermined as `warning` and compatible as
/// `note`. Every result carries its suppressions, an accepted external one when a Difference
/// acknowledgement accepts a difference.
pub(crate) fn diff(comparison: &RevisionComparison<'_>, directory: &Path) -> String {
    let limit = comparison.catalogue(Revision::Earlier).limits.report_bytes;
    let mut log = Log::new(directory, limit, DIFF_HELP);
    // Version warnings come first, at most one per compared document pair. The later
    // document's version locates each, and the earlier version is related.
    for warning in comparison.version_warnings() {
        let place = |revision| Place {
            source: source_place(
                comparison.catalogue(revision),
                revision,
                warning.location(revision),
                Some(warning.pointer(revision).to_owned()),
            ),
            ..Place::default()
        };
        let mut related = place(Revision::Earlier);
        related.message = Some("earlier revision");
        log.results.push(Item {
            rule: warning.code().to_owned(),
            description: format!("A warning that a document with a breaking difference has an info version that is absent or does not compare higher than the earlier revision's, although {} recommends a lexicographically increasing version.", version::CITATION),
            level: "warning",
            message: format!(
                "Version not increased: {}.",
                wording::describe_warning(comparison, warning, &wording::PLAIN)
            ),
            primary: place(Revision::Later),
            related: vec![related],
            suppressed: Some(false),
        });
    }
    for difference in comparison.differences() {
        let (rule, description, level, label) = match difference.classification() {
            Classification::Breaking => (
                "comparison.breaking",
                "A breaking difference between two Model revisions, after which an interaction the earlier revision described can fail for a Client, or yield data the Client was not described to receive.".to_owned(),
                "error",
                "Breaking",
            ),
            Classification::Compatible => (
                "comparison.compatible",
                format!("A compatible difference between two Model revisions, which Esdieff established cannot break client interactions of the earlier revision. {}", crate::comparison::SCOPE_NOTICE),
                "note",
                "Compatible",
            ),
            _ => {
                let code = difference.reason().map_or("", |reason| reason.code());
                (
                    code,
                    format!("An undetermined difference between two Model revisions, which Esdieff could not classify, with reason code {code}."),
                    "warning",
                    "Undetermined",
                )
            }
        };
        let place = |revision| {
            difference
                .side(revision)
                .map(|side| diff_place(comparison, revision, side))
        };
        let (earlier, later) = (place(Revision::Earlier), place(Revision::Later));
        // The later side locates the result when it has a content origin, or when the earlier
        // side has none either. The other side is related.
        let later_first = later.as_ref().is_some_and(|later| {
            later.source.is_some()
                || earlier
                    .as_ref()
                    .is_none_or(|earlier| earlier.source.is_none())
        });
        let (primary, related) = if later_first {
            (later, earlier.map(|place| (place, "earlier revision")))
        } else {
            (earlier, later.map(|place| (place, "later revision")))
        };
        let label = if difference.acknowledged() {
            format!("Acknowledged {}", label.to_lowercase())
        } else {
            label.to_owned()
        };
        log.results.push(Item {
            rule: rule.to_owned(),
            description,
            level,
            message: format!(
                "{label} difference: {}.",
                wording::describe(comparison, &difference, &wording::PLAIN)
            ),
            primary: primary.unwrap_or_default(),
            related: related
                .map(|(mut place, message)| {
                    place.message = Some(message);
                    place
                })
                .into_iter()
                .collect(),
            suppressed: Some(difference.acknowledged()),
        });
    }
    let findings = comparison.total() + comparison.version_warnings().len() as u64;
    log.render(comparison.disposition().as_str(), findings, 0)
}

/// The source location of `json` in one revision, with its byte span.
fn source_place<'a>(
    catalogue: &'a Catalogue,
    revision: Revision,
    json: JsonView<'_>,
    pointer: Option<String>,
) -> Option<SourcePlace<'a>> {
    let index = match revision {
        Revision::Earlier => 1,
        _ => 2,
    };
    let source = catalogue.source(json.source_id())?;
    let span = json.span();
    Some(SourcePlace {
        key: (index, source.id().get()),
        source,
        pointer,
        span: Some(span.start as u32..span.end as u32),
        revision: Some(revision.as_str()),
    })
}

/// One revision's side of a difference: its effective location and nearest content origin.
fn diff_place<'a>(
    comparison: &RevisionComparison<'a>,
    revision: Revision,
    side: DifferenceSide<'_>,
) -> Place<'a> {
    let catalogue = comparison.catalogue(revision);
    let limit = catalogue.limits.pointer_bytes as u32;
    let source = side
        .origin()
        .and_then(|origin| origin.sources().next())
        .and_then(|origin| {
            source_place(catalogue, revision, origin, origin.pointer(limit).present())
        });
    let effective = side.pointer(limit).present().map(|pointer| Logical {
        pointer,
        view: "effective",
        document: catalogue
            .source(side.root_document())
            .map_or_else(String::new, document),
        revision: Some(revision.as_str()),
    });
    Place {
        source,
        effective,
        message: None,
    }
}

/// A source's name for logical locations: its label, or else its key.
fn document(source: &Source) -> String {
    source.label().unwrap_or(source.key()).to_owned()
}

/// Identifies a source across revisions: 0 for a check report, 1 for the earlier and 2 for
/// the later revision, and the source ID within it.
type Key = (u8, u32);

/// One location of a result or notification before rendering.
#[derive(Default)]
struct Place<'a> {
    source: Option<SourcePlace<'a>>,
    effective: Option<Logical>,
    message: Option<&'static str>,
}
struct SourcePlace<'a> {
    key: Key,
    source: &'a Source,
    pointer: Option<String>,
    span: Option<Range<u32>>,
    revision: Option<&'static str>,
}
struct Logical {
    pointer: String,
    view: &'static str,
    document: String,
    revision: Option<&'static str>,
}

/// A result, or a notification when it is incomplete work.
struct Item<'a> {
    rule: String,
    /// The full description of the item's rule or notification descriptor.
    description: String,
    level: &'static str,
    message: String,
    primary: Place<'a>,
    related: Vec<Place<'a>>,
    /// Whether an external suppression accepts the result, or `None` when the run carries no
    /// suppression information.
    suppressed: Option<bool>,
}

struct Log<'a, 'd> {
    directory: &'d Path,
    limit: usize,
    /// The help text of every descriptor.
    help: &'static str,
    results: Vec<Item<'a>>,
    notifications: Vec<Item<'a>>,
}
impl<'a, 'd> Log<'a, 'd> {
    fn new(directory: &'d Path, limit: usize, help: &'static str) -> Self {
        Self {
            directory,
            limit,
            help,
            results: Vec::new(),
            notifications: Vec::new(),
        }
    }

    /// Render the log within the byte limit. `findings` counts every result the report
    /// established and `incomplete` every notification, so the counts of omitted ones
    /// include what the report itself could not retain.
    fn render(self, disposition: &str, findings: u64, incomplete: u64) -> String {
        let mut spans: HashMap<Key, (&Source, Vec<u32>)> = HashMap::new();
        for item in self.results.iter().chain(&self.notifications) {
            for place in std::iter::once(&item.primary).chain(&item.related) {
                if let Some(SourcePlace {
                    key,
                    source,
                    span: Some(span),
                    ..
                }) = &place.source
                {
                    let offsets = &mut spans.entry(*key).or_insert((source, Vec::new())).1;
                    offsets.extend([span.start, span.end]);
                }
            }
        }
        let lines: HashMap<Key, Lines> = spans
            .into_iter()
            .map(|(key, (source, offsets))| (key, Lines::new(source.raw_bytes(), offsets)))
            .collect();
        let mut uris: HashMap<Key, Option<(String, bool)>> = HashMap::new();
        // A place's SARIF location, absent when it names nothing.
        let mut locate = |place: &Place<'_>| -> Option<Value> {
            let mut location = Map::new();
            let mut logical = Vec::new();
            if let Some(source) = &place.source {
                let uri = uris
                    .entry(source.key)
                    .or_insert_with(|| artifact_uri(source.source, self.directory));
                match uri {
                    Some((uri, relative)) => {
                        let mut physical = json!({"artifactLocation":{"uri":uri}});
                        if *relative {
                            physical["artifactLocation"]["uriBaseId"] = json!(BASE);
                        }
                        // A location without a span is the whole file, at its first line.
                        physical["region"] = source
                            .span
                            .as_ref()
                            .and_then(|span| {
                                let lines = lines.get(&source.key)?;
                                let (start_line, start_column) = lines.at(span.start)?;
                                let (end_line, end_column) = lines.at(span.end)?;
                                Some(json!({"startLine":start_line,"startColumn":start_column,
                                    "endLine":end_line,"endColumn":end_column,
                                    "byteOffset":span.start,"byteLength":span.end - span.start}))
                            })
                            .unwrap_or_else(|| json!({"startLine":1}));
                        location.insert("physicalLocation".to_owned(), physical);
                    }
                    // A source without a path is named by its pointer only.
                    None => logical.extend(source.pointer.as_ref().map(|pointer| {
                        logical_location(&Logical {
                            pointer: pointer.clone(),
                            view: "source",
                            document: document(source.source),
                            revision: source.revision,
                        })
                    })),
                }
            }
            logical.extend(place.effective.as_ref().map(logical_location));
            if !logical.is_empty() {
                location.insert("logicalLocations".to_owned(), Value::Array(logical));
            }
            if location.is_empty() {
                return None;
            }
            if let Some(message) = place.message {
                location.insert("message".to_owned(), json!({"text":message}));
            }
            Some(Value::Object(location))
        };
        // Reserve the envelope with the largest counts and the omission notification, then
        // keep results, most important first, and notifications while they fit. Each costs
        // its bytes, an array separator and any new descriptor it needs.
        let base = base_uri(self.directory);
        let reserve = Envelope {
            disposition,
            base: base.as_deref(),
            omitted: Some((u64::MAX, u64::MAX)),
            ..Envelope::default()
        };
        let mut remaining = self.limit.saturating_sub(
            serde_json::to_string(&reserve.value())
                .expect("SARIF envelope")
                .len(),
        );
        let mut fits = |value: &Value, cost: usize| {
            let size = serde_json::to_string(value).expect("SARIF item").len() + 1 + cost;
            let fits = size <= remaining;
            if fits {
                remaining -= size;
            }
            fits
        };
        let mut rules = Descriptors::default();
        let mut results = Vec::new();
        for item in &self.results {
            let (index, cost) = rules.index(item, self.help);
            let mut result = json!({"ruleId":item.rule,"ruleIndex":index,"level":item.level,
                "message":{"text":item.message}});
            if let Some(location) = locate(&item.primary) {
                result["locations"] = json!([location]);
            }
            let related: Vec<_> = item.related.iter().filter_map(&mut locate).collect();
            if !related.is_empty() {
                result["relatedLocations"] = Value::Array(related);
            }
            if let Some(suppressed) = item.suppressed {
                result["suppressions"] = if suppressed {
                    json!([{"kind":"external","status":"accepted"}])
                } else {
                    json!([])
                };
            }
            if !fits(&result, cost) {
                break;
            }
            rules.keep(item, self.help);
            results.push(result);
        }
        let mut descriptors = Descriptors::default();
        let mut notifications = Vec::new();
        if results.len() == self.results.len() {
            for item in &self.notifications {
                let (index, cost) = descriptors.index(item, self.help);
                let mut notification = json!({"descriptor":{"id":item.rule,"index":index},
                    "level":item.level,"message":{"text":item.message}});
                if let Some(location) = locate(&item.primary) {
                    notification["locations"] = json!([location]);
                }
                if !fits(&notification, cost) {
                    break;
                }
                descriptors.keep(item, self.help);
                notifications.push(notification);
            }
        }
        let omitted = (
            findings.saturating_sub(results.len() as u64),
            incomplete.saturating_sub(notifications.len() as u64),
        );
        let log = Envelope {
            disposition,
            base: base.as_deref(),
            rules: rules.0,
            descriptors: descriptors.0,
            notifications,
            results,
            omitted: (omitted != (0, 0)).then_some(omitted),
        };
        let text = serde_json::to_string(&log.value()).expect("SARIF log");
        debug_assert!(
            text.len() <= self.limit,
            "SARIF log within the report limit"
        );
        text
    }
}

/// The whole log.
#[derive(Default)]
struct Envelope<'e> {
    disposition: &'e str,
    /// The invocation directory's `file` URI, when it is known.
    base: Option<&'e str>,
    rules: Vec<Value>,
    descriptors: Vec<Value>,
    notifications: Vec<Value>,
    results: Vec<Value>,
    /// The omitted results and notifications, which add a notification that counts them.
    omitted: Option<(u64, u64)>,
}
impl Envelope<'_> {
    fn value(self) -> Value {
        let mut notifications = self.notifications;
        let (results_omitted, notifications_omitted) = self.omitted.unwrap_or_default();
        if self.omitted.is_some() {
            notifications.push(json!({"level":"warning","message":{"text":format!(
                "{results_omitted} results and {notifications_omitted} notifications are not shown, because their details did not fit the report limits"
            )}}));
        }
        let mut run = json!({
            "tool":{"driver":{"name":"Esdieff","version":env!("CARGO_PKG_VERSION"),
                "rules":self.rules,"notifications":self.descriptors}},
            "columnKind":"unicodeCodePoints",
            "invocations":[{"executionSuccessful":true,"toolExecutionNotifications":notifications}],
            "properties":{"disposition":self.disposition,"omittedResults":results_omitted,
                "omittedNotifications":notifications_omitted},
            "results":self.results});
        if let Some(base) = self.base {
            run["originalUriBaseIds"] = json!({(BASE):{"uri":base}});
        }
        json!({"$schema":SCHEMA,"version":"2.1.0","runs":[run]})
    }
}

/// Rule or notification descriptors in order of first use, one per code.
#[derive(Default)]
struct Descriptors(Vec<Value>);
impl Descriptors {
    fn descriptor(item: &Item<'_>, help: &str) -> Value {
        json!({"id":item.rule,"shortDescription":{"text":item.rule},
            "fullDescription":{"text":item.description},"help":{"text":help}})
    }
    /// The index an item's code has or would get, and the bytes adding its descriptor costs.
    fn index(&self, item: &Item<'_>, help: &str) -> (usize, usize) {
        match self.0.iter().position(|rule| rule["id"] == item.rule) {
            Some(index) => (index, 0),
            None => (
                self.0.len(),
                serde_json::to_string(&Self::descriptor(item, help))
                    .expect("descriptor")
                    .len()
                    + 1,
            ),
        }
    }
    fn keep(&mut self, item: &Item<'_>, help: &str) {
        if !self.0.iter().any(|rule| rule["id"] == item.rule) {
            self.0.push(Self::descriptor(item, help));
        }
    }
}

fn logical_location(logical: &Logical) -> Value {
    let mut properties = json!({"view":logical.view,"document":logical.document});
    if let Some(revision) = logical.revision {
        properties["revision"] = json!(revision);
    }
    json!({"fullyQualifiedName":logical.pointer,"properties":properties})
}

/// The artifact URI of a file source and whether it is relative. A path inside `directory`
/// is relative to it, with `/` separators and dot segments resolved as RFC 3986 resolves
/// them. A path that lies outside it, whether absolute or through `..` segments, is an
/// absolute `file` URI. A source without a UTF-8 path, such as stdin, has none.
fn artifact_uri(source: &Source, directory: &Path) -> Option<(String, bool)> {
    let path = directory.join(std::str::from_utf8(source.raw_path_identity()?).ok()?);
    path_uri(&path, directory)
}

fn path_uri(path: &Path, directory: &Path) -> Option<(String, bool)> {
    let (prefix, segments) = resolve(path)?;
    if path.is_absolute() && directory.is_absolute() {
        let (base_prefix, base) = resolve(directory)?;
        if prefix == base_prefix && segments.starts_with(&base) {
            return Some((encoded(&segments[base.len()..]), true));
        }
    }
    Some(if path.is_absolute() {
        (file_uri(prefix, &segments), false)
    } else {
        // An unknown invocation directory leaves a relative path as given.
        (encoded(&segments), true)
    })
}

/// The `file` URI of the invocation directory, ending in `/`, when it is absolute.
fn base_uri(directory: &Path) -> Option<String> {
    if !directory.is_absolute() {
        return None;
    }
    let (prefix, segments) = resolve(directory)?;
    let mut uri = file_uri(prefix, &segments);
    if !uri.ends_with('/') {
        uri.push('/');
    }
    Some(uri)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FilePrefix<'a> {
    Disk(u8),
    Unc(&'a str, &'a str),
    Other(&'a str),
}

/// A path's volume prefix and normal segments, with `.` and `..` resolved lexically. Leading
/// `..` segments of a relative path remain.
fn resolve(path: &Path) -> Option<(Option<FilePrefix<'_>>, Vec<&str>)> {
    let mut prefix = None;
    let mut segments: Vec<&str> = Vec::new();
    for component in path.components() {
        // Verbatim Windows paths retain these as normal components.
        let component = match component {
            Component::Normal(name) if name == "." => Component::CurDir,
            Component::Normal(name) if name == ".." => Component::ParentDir,
            component => component,
        };
        match component {
            Component::Prefix(drive) => {
                use std::path::Prefix;
                prefix = Some(match drive.kind() {
                    Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => FilePrefix::Disk(letter),
                    Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                        FilePrefix::Unc(server.to_str()?, share.to_str()?)
                    }
                    _ => FilePrefix::Other(drive.as_os_str().to_str()?),
                });
            }
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir => match segments.last() {
                Some(&last) if last != ".." => {
                    segments.pop();
                }
                _ if path.has_root() => {}
                _ => segments.push(".."),
            },
            Component::Normal(name) => segments.push(name.to_str()?),
        }
    }
    Some((prefix, segments))
}

fn encoded(segments: &[&str]) -> String {
    segments
        .iter()
        .map(|segment| encode(segment, false))
        .collect::<Vec<_>>()
        .join("/")
}

fn file_uri(prefix: Option<FilePrefix<'_>>, segments: &[&str]) -> String {
    let mut uri = match prefix {
        Some(FilePrefix::Disk(letter)) => format!("file:///{}:/", char::from(letter)),
        Some(FilePrefix::Unc(server, share)) => {
            format!("file://{}/{}/", encode(server, false), encode(share, false))
        }
        Some(FilePrefix::Other(prefix)) => format!("file:///{}/", encode(prefix, true)),
        None => String::from("file:///"),
    };
    uri.push_str(&encoded(segments));
    uri
}

/// Percent-encode one path segment. A Windows drive prefix keeps its colon.
fn encode(segment: &str, prefix: bool) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char)
            }
            b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'=' | b'@' => {
                encoded.push(byte as char)
            }
            b':' if prefix => encoded.push(':'),
            byte => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// Lines and code-point columns of byte offsets in one source, counted in a single pass.
/// CR, LF and CR LF end lines.
struct Lines(Vec<(u32, u32, u32)>);
impl Lines {
    fn new(bytes: &[u8], mut offsets: Vec<u32>) -> Self {
        offsets.sort_unstable();
        offsets.dedup();
        let mut positions = Vec::with_capacity(offsets.len());
        let (mut line, mut column, mut index, mut after_cr) = (1, 1, 0, false);
        for offset in offsets {
            let end = (offset as usize).min(bytes.len());
            while index < end {
                let byte = bytes[index];
                match byte {
                    b'\n' if after_cr => {}
                    b'\n' | b'\r' => {
                        line += 1;
                        column = 1;
                    }
                    // Count each code point at its leading byte.
                    byte if byte & 0xC0 != 0x80 => column += 1,
                    _ => {}
                }
                after_cr = byte == b'\r';
                index += 1;
            }
            positions.push((offset, line, column));
        }
        Self(positions)
    }
    fn at(&self, offset: u32) -> Option<(u32, u32)> {
        let index = self
            .0
            .binary_search_by_key(&offset, |(offset, ..)| *offset)
            .ok()?;
        let (_, line, column) = self.0[index];
        Some((line, column))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn verbatim_windows_paths_keep_disk_and_share_identity_without_namespace_syntax() {
        let base = Path::new(r"C:\models");
        for path in [
            r"C:\models\my lock #1.sdf.json",
            r"\\?\C:\models\my lock #1.sdf.json",
            r"\\?\c:\models\.\nested\..\my lock #1.sdf.json",
        ] {
            assert_eq!(
                path_uri(Path::new(path), base),
                Some(("my%20lock%20%231.sdf.json".to_owned(), true))
            );
        }
        assert_eq!(
            path_uri(Path::new(r"\\?\C:\models\..\outside.sdf.json"), base),
            Some(("file:///C:/outside.sdf.json".to_owned(), false))
        );
        assert_eq!(
            path_uri(Path::new(r"\\?\D:\models\source.sdf.json"), base),
            Some(("file:///D:/models/source.sdf.json".to_owned(), false))
        );
        assert_eq!(
            base_uri(Path::new(r"\\?\C:\models")),
            Some("file:///C:/models/".to_owned())
        );
        let share = Path::new(r"\\server\share\models");
        assert_eq!(
            path_uri(
                Path::new(r"\\?\UNC\server\share\models\source.sdf.json"),
                share
            ),
            Some(("source.sdf.json".to_owned(), true))
        );
        for (path, uri) in [
            (
                r"\\?\UNC\server\other\models\source.sdf.json",
                "file://server/other/models/source.sdf.json",
            ),
            (
                r"\\other\share\models\source.sdf.json",
                "file://other/share/models/source.sdf.json",
            ),
        ] {
            assert_eq!(
                path_uri(Path::new(path), share),
                Some((uri.to_owned(), false))
            );
        }
        assert_eq!(
            base_uri(Path::new(r"\\?\UNC\server\share\models")),
            Some("file://server/share/models/".to_owned())
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_colons_and_backslashes_remain_filename_characters() {
        assert_eq!(
            path_uri(
                Path::new(r"/models/C:\source.sdf.json"),
                Path::new("/models")
            ),
            Some(("C%3A%5Csource.sdf.json".to_owned(), true))
        );
    }

    fn item(rule: &str, level: &'static str) -> Item<'static> {
        Item {
            rule: rule.to_owned(),
            description: format!("Rule {rule}."),
            level,
            message: "x".repeat(100),
            primary: Place::default(),
            related: Vec::new(),
            suppressed: None,
        }
    }

    /// Results the report retained are left out, and counted, when the log would exceed the
    /// limit, together with the notifications after them and the descriptors only they need.
    #[test]
    fn logs_leave_out_trailing_items_beyond_the_limit() {
        let log = |limit| {
            let mut log = Log::new(Path::new(""), limit, "Help.");
            log.results = vec![item("a", "error"), item("a", "error"), item("b", "note")];
            log.notifications = vec![item("c", "warning")];
            // The report omitted two further results and one notification itself.
            log.render("failed", 5, 2)
        };
        let everything = log(usize::MAX);
        let limit = everything.len() - 300;
        let text = log(limit);
        assert!(text.len() <= limit);
        let value: Value = serde_json::from_str(&text).unwrap();
        let run = &value["runs"][0];
        let kept = run["results"].as_array().unwrap().len();
        assert!((1..3).contains(&kept), "{kept}");
        let rules = &run["tool"]["driver"]["rules"];
        assert_eq!(rules[0]["id"], "a");
        assert_eq!(
            rules.as_array().unwrap().len(),
            1,
            "no rule for a dropped result"
        );
        assert_eq!(run["tool"]["driver"]["notifications"], json!([]));
        assert_eq!(run["properties"]["omittedResults"], 5 - kept);
        assert_eq!(run["properties"]["omittedNotifications"], 2);
    }

    #[test]
    fn lines_count_code_points_and_every_line_ending() {
        let lines = Lines::new("a\r\nü\rb\n🌡c".as_bytes(), vec![0, 3, 5, 6, 7, 8, 12, 13]);
        assert_eq!(lines.at(0), Some((1, 1)));
        assert_eq!(lines.at(3), Some((2, 1)));
        assert_eq!(lines.at(5), Some((2, 2)));
        assert_eq!(lines.at(6), Some((3, 1)));
        assert_eq!(lines.at(7), Some((3, 2)));
        assert_eq!(lines.at(8), Some((4, 1)));
        assert_eq!(lines.at(12), Some((4, 2)));
        assert_eq!(lines.at(13), Some((4, 3)));
    }
}

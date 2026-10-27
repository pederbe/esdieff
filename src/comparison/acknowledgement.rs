//! Difference acknowledgements name intended differences, so that only unacknowledged ones
//! decide a comparison's disposition. Follows ADR 0040: Acknowledged revision differences and
//! ADR 0009: Bounded processing and reporting.
//!
//! An entry's identity is its kind, subject, quality, classification and, for each revision,
//! the document's source key and the effective pointer. Matching runs after the comparison over
//! the retained differences, so it changes no classification.
use super::{
    Classification, DifferenceKind, DifferenceSubject, Record, Revision, RevisionComparison, Side,
    UndeterminedReason,
};
use crate::json::{Parsed, PointerBudget, Value};
use crate::{Class, Limits};
use std::collections::{HashMap, HashSet};

const SCHEMA: &str = "esdieff.acknowledged-differences";
const ENTRIES: usize = 4096;
/// The report reserves an unused entry's index, at most `4095` and a comma.
const INDEX_BYTES: usize = 5;

/// Whether a difference can be acknowledged: a breaking one, or an undetermined one of kind
/// removed, added or changed whose content was examined. A reason in the `Unavailable` family,
/// which covers unavailable content and the operation limit, means nobody examined it. An
/// entry names no reason, so only its kind and classification apply. This is the one
/// eligibility rule: entries and `locate` apply it, so matching, the report and
/// `Difference::acknowledgement()` follow it.
fn eligible(
    kind: DifferenceKind,
    classification: Classification,
    reason: Option<UndeterminedReason>,
) -> Result<(), &'static str> {
    if kind == DifferenceKind::Unavailable {
        return Err("unavailable content cannot be acknowledged");
    }
    match (classification, reason) {
        (Classification::Compatible, _) => Err("a compatible difference cannot be acknowledged"),
        (_, Some(UndeterminedReason::Unavailable { .. })) => {
            Err("a difference whose content was not examined cannot be acknowledged")
        }
        _ => Ok(()),
    }
}

/// A Difference acknowledgement: one intended difference, named by its kind, subject, quality,
/// classification and its location in each revision that has it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DifferenceAcknowledgement {
    kind: DifferenceKind,
    subject: Subject,
    classification: Classification,
    sides: [Option<Location>; 2],
    note: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum Subject {
    Document,
    Definition(Class),
    Entry,
    Quality(String),
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Location {
    document: String,
    pointer: String,
}
/// What matching compares: everything but the note.
#[derive(Eq, Hash, PartialEq)]
struct Identity<'a> {
    kind: &'static str,
    subject: &'static str,
    quality: Option<&'a str>,
    classification: &'static str,
    sides: [Option<(&'a str, &'a str)>; 2],
}

impl DifferenceAcknowledgement {
    /// An entry without locations, refused for a kind or classification that can never be
    /// acknowledged.
    pub fn new(
        kind: DifferenceKind,
        subject: DifferenceSubject<'_>,
        classification: Classification,
    ) -> Result<Self, &'static str> {
        eligible(kind, classification, None)?;
        let subject = match subject {
            DifferenceSubject::Document => Subject::Document,
            DifferenceSubject::Definition(Class::Data) => {
                return Err("sdfData definitions are compared where they are used");
            }
            DifferenceSubject::Definition(class) => Subject::Definition(class),
            DifferenceSubject::Entry => Subject::Entry,
            DifferenceSubject::Quality(quality) if quality.len() <= 256 => {
                Subject::Quality(quality.to_owned())
            }
            DifferenceSubject::Quality(_) => return Err("acknowledged quality exceeds 256 bytes"),
        };
        Ok(Self {
            kind,
            subject,
            classification,
            sides: [None, None],
            note: None,
        })
    }
    /// Locate the difference in the earlier revision by document source key and effective
    /// pointer.
    pub fn earlier(self, document: &str, pointer: &str) -> Result<Self, &'static str> {
        self.side(Revision::Earlier, document, pointer)
    }
    /// Locate the difference in the later revision by document source key and effective
    /// pointer.
    pub fn later(self, document: &str, pointer: &str) -> Result<Self, &'static str> {
        self.side(Revision::Later, document, pointer)
    }
    /// A comment for readers, which matching ignores.
    pub fn with_note(mut self, note: &str) -> Result<Self, &'static str> {
        if note.len() > 1024 {
            return Err("acknowledgement note exceeds 1024 bytes");
        }
        self.note = Some(note.to_owned());
        Ok(self)
    }
    fn side(
        mut self,
        revision: Revision,
        document: &str,
        pointer: &str,
    ) -> Result<Self, &'static str> {
        if document.is_empty() || document.len() > 256 {
            return Err("acknowledged document key must contain 1 through 256 bytes");
        }
        crate::json::validate_pointer(pointer)?;
        self.sides[revision.index()] = Some(Location {
            document: document.to_owned(),
            pointer: pointer.to_owned(),
        });
        Ok(self)
    }
    pub fn kind(&self) -> DifferenceKind {
        self.kind
    }
    pub fn subject(&self) -> DifferenceSubject<'_> {
        match &self.subject {
            Subject::Document => DifferenceSubject::Document,
            Subject::Definition(class) => DifferenceSubject::Definition(*class),
            Subject::Entry => DifferenceSubject::Entry,
            Subject::Quality(quality) => DifferenceSubject::Quality(quality),
        }
    }
    pub fn classification(&self) -> Classification {
        self.classification
    }
    /// The source key of the document holding the difference in one revision, absent when
    /// the entry names no location there.
    pub fn document(&self, revision: Revision) -> Option<&str> {
        self.location(revision)
            .map(|location| location.document.as_str())
    }
    /// The effective pointer of the difference in one revision.
    pub fn pointer(&self, revision: Revision) -> Option<&str> {
        self.location(revision)
            .map(|location| location.pointer.as_str())
    }
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
    fn location(&self, revision: Revision) -> Option<&Location> {
        self.sides[revision.index()].as_ref()
    }
    fn identity(&self) -> Identity<'_> {
        let subject = self.subject();
        Identity {
            kind: self.kind.as_str(),
            subject: super::report::subject(subject),
            quality: match subject {
                DifferenceSubject::Quality(quality) => Some(quality),
                _ => None,
            },
            classification: self.classification.as_str(),
            sides: [Revision::Earlier, Revision::Later].map(|revision| {
                self.location(revision)
                    .map(|location| (location.document.as_str(), location.pointer.as_str()))
            }),
        }
    }
    /// The entry as a member of an acknowledgement file, without its note.
    pub(super) fn wire(&self) -> serde_json::Value {
        use serde_json::json;
        let subject = self.subject();
        let mut value = json!({"kind":self.kind.as_str(),"subject":super::report::subject(subject),
            "classification":self.classification.as_str()});
        if let DifferenceSubject::Quality(quality) = subject {
            value["quality"] = json!(quality);
        }
        for revision in [Revision::Earlier, Revision::Later] {
            value[revision.as_str()] = match self.location(revision) {
                Some(location) => json!({"document":location.document,"pointer":location.pointer}),
                None => json!(null),
            };
        }
        value
    }
}

/// The Difference acknowledgements supplied to one comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DifferenceAcknowledgements {
    entries: Vec<DifferenceAcknowledgement>,
}
impl DifferenceAcknowledgements {
    /// The largest acknowledgement file `from_json` decodes, 1 MiB. A reader that stops one
    /// byte beyond it lets `from_json` refuse a larger file.
    pub const FILE_BYTES: usize = 1024 * 1024;
    /// At most 4096 entries, each with at least one location, and no two with the same
    /// identity. Notes are not part of an identity.
    pub fn new(entries: Vec<DifferenceAcknowledgement>) -> Result<Self, &'static str> {
        if entries.len() > ENTRIES {
            return Err("acknowledgements exceed 4096 entries");
        }
        let mut identities = HashSet::new();
        for entry in &entries {
            if entry.sides.iter().all(Option::is_none) {
                return Err("an acknowledgement requires an earlier or later location");
            }
            if !identities.insert(entry.identity()) {
                return Err("duplicate acknowledgement");
            }
        }
        Ok(Self { entries })
    }
    /// Decode an acknowledgement file of at most 1 MiB under the ordinary JSON input
    /// policies, which refuse duplicate members, within the decoding limits of `limits`,
    /// normally those of the revisions it acknowledges. Limits a Catalogue refuses are refused.
    pub fn from_json(bytes: &[u8], limits: &Limits) -> Result<Self, &'static str> {
        limits.validate()?;
        if bytes.len() > Self::FILE_BYTES {
            return Err("acknowledgement file exceeds 1 MiB");
        }
        if std::str::from_utf8(bytes).is_err() {
            return Err("acknowledgement file requires UTF-8 JSON");
        }
        let mut budget = crate::json::Budget::new(limits);
        let parsed = crate::json::parse_value(bytes, true, limits, &mut budget);
        if parsed.rejected || parsed.errors > 0 {
            return Err("acknowledgement file syntax or duplicate decoded member rejected");
        }
        let root = parsed.root.ok_or("acknowledgement file decoding limit")?;
        let file = File { parsed: &parsed };
        let [schema, version, differences] =
            file.fields(root, ["schema", "version", "differences"])?;
        if file.text(schema)? != SCHEMA || file.text(version)? != "1" {
            return Err("unknown acknowledgement file schema or version");
        }
        let differences = differences.ok_or("acknowledgement file requires differences")?;
        if !matches!(parsed.nodes[differences].value, Value::Array) {
            return Err("acknowledgement differences must be an array");
        }
        let mut entries = Vec::new();
        let mut next = parsed.nodes[differences].first;
        while let Some(node) = next {
            entries.push(file.entry(node)?);
            next = parsed.nodes[node].next;
        }
        Self::new(entries)
    }
    pub fn entries(&self) -> &[DifferenceAcknowledgement] {
        &self.entries
    }
    /// The report bytes the mandatory unused-entry list can need.
    pub(super) fn reserve(&self) -> usize {
        self.entries.len() * INDEX_BYTES
    }
}

/// A decoded acknowledgement file.
struct File<'p> {
    parsed: &'p Parsed,
}
impl<'p> File<'p> {
    fn entry(&self, node: usize) -> Result<DifferenceAcknowledgement, &'static str> {
        let [kind, subject, quality, classification, earlier, later, note] = self.fields(
            node,
            [
                "kind",
                "subject",
                "quality",
                "classification",
                "earlier",
                "later",
                "note",
            ],
        )?;
        let kind = match self.text(kind)? {
            "removed" => DifferenceKind::Removed,
            "added" => DifferenceKind::Added,
            "changed" => DifferenceKind::Changed,
            "unavailable" => DifferenceKind::Unavailable,
            _ => return Err("unknown acknowledged difference kind"),
        };
        let quality = quality.map(|node| self.string(node)).transpose()?;
        let subject = match (self.text(subject)?, quality) {
            ("quality", Some(quality)) => DifferenceSubject::Quality(quality),
            ("quality", None) | (_, Some(_)) => {
                return Err(
                    "an acknowledgement names a quality exactly when its subject is quality",
                );
            }
            ("document", None) => DifferenceSubject::Document,
            ("properties_entry", None) => DifferenceSubject::Entry,
            ("sdfThing", None) => DifferenceSubject::Definition(Class::Thing),
            ("sdfObject", None) => DifferenceSubject::Definition(Class::Object),
            ("sdfProperty", None) => DifferenceSubject::Definition(Class::Property),
            ("sdfAction", None) => DifferenceSubject::Definition(Class::Action),
            ("sdfEvent", None) => DifferenceSubject::Definition(Class::Event),
            _ => return Err("unknown acknowledged difference subject"),
        };
        let classification = match self.text(classification)? {
            "breaking" => Classification::Breaking,
            "undetermined" => Classification::Undetermined,
            "compatible" => Classification::Compatible,
            _ => return Err("unknown acknowledged classification"),
        };
        let mut entry = DifferenceAcknowledgement::new(kind, subject, classification)?;
        for (revision, side) in [(Revision::Earlier, earlier), (Revision::Later, later)] {
            let side = side.ok_or("an acknowledgement requires earlier and later members")?;
            if matches!(self.parsed.nodes[side].value, Value::Null) {
                continue;
            }
            let [document, pointer] = self.fields(side, ["document", "pointer"])?;
            entry = entry.side(revision, self.text(document)?, self.text(pointer)?)?;
        }
        match note {
            Some(note) => entry.with_note(self.string(note)?),
            None => Ok(entry),
        }
    }
    /// A map's members by name, refusing any other member.
    fn fields<const N: usize>(
        &self,
        node: usize,
        names: [&str; N],
    ) -> Result<[Option<usize>; N], &'static str> {
        if !matches!(self.parsed.nodes[node].value, Value::Map) {
            return Err("acknowledgement file and each entry and location must be maps");
        }
        let mut found = [None; N];
        let mut next = self.parsed.nodes[node].first;
        while let Some(child) = next {
            let key = self.parsed.nodes[child]
                .key
                .as_deref()
                .ok_or("acknowledgement member decoding unavailable")?;
            let index = names
                .iter()
                .position(|name| *name == key)
                .ok_or("unknown acknowledgement file member")?;
            found[index] = Some(child);
            next = self.parsed.nodes[child].next;
        }
        Ok(found)
    }
    fn text(&self, node: Option<usize>) -> Result<&'p str, &'static str> {
        self.string(node.ok_or("required acknowledgement file member absent")?)
    }
    fn string(&self, node: usize) -> Result<&'p str, &'static str> {
        match &self.parsed.nodes[node].value {
            Value::String(Some(text)) => Ok(text),
            Value::String(None) => Err("acknowledgement string decoding unavailable"),
            _ => Err("acknowledgement file member must be a string"),
        }
    }
}

/// Render the effective pointers of each retained difference that can be acknowledged, once
/// and within one materialized-pointer allowance, so that matching, the report and
/// `Difference::acknowledgement()` name the same entry. A difference whose pointer cannot be
/// rendered has no entry.
pub(super) fn locate(comparison: &mut RevisionComparison<'_>) {
    let limits = &comparison.catalogues[0].limits;
    let (cap, mut work) = (
        limits.pointer_bytes,
        PointerBudget::new(limits.pointer_total),
    );
    // A side the revision lacks has no pointer, and a pointer that does not render leaves the
    // difference without an entry.
    let mut rendered = |side: &Option<Side<'_>>| match side {
        Some(side) => super::report::render_pointer(comparison, side, cap, &mut work)
            .ok()
            .map(Some),
        None => Some(None),
    };
    let pointers: Vec<_> = comparison
        .records
        .iter()
        .map(|record| {
            let classification = record.verdict.classification();
            eligible(record.kind, classification, record.verdict.reason()).ok()?;
            let [earlier, later] = &record.sides;
            Some([rendered(earlier)?, rendered(later)?])
        })
        .collect();
    for (record, pointers) in comparison.records.iter_mut().zip(pointers) {
        record.pointers = pointers;
    }
}

/// The entry that would acknowledge a retained difference, present when it can be
/// acknowledged and `locate` rendered its pointers.
pub(super) fn entry(
    comparison: &RevisionComparison<'_>,
    record: &Record<'_>,
) -> Option<DifferenceAcknowledgement> {
    let pointers = record.pointers.as_ref()?;
    let classification = record.verdict.classification();
    let mut entry =
        DifferenceAcknowledgement::new(record.kind, record.subject, classification).ok()?;
    for revision in [Revision::Earlier, Revision::Later] {
        let index = revision.index();
        if let (Some(side), Some(pointer)) = (&record.sides[index], &pointers[index]) {
            let source = comparison.catalogue(revision).source(side.root)?;
            entry = entry.side(revision, &source.key, pointer).ok()?;
        }
    }
    Some(entry)
}

/// Mark each retained difference an entry acknowledges, and list the entries that acknowledge
/// nothing. A difference without an entry stays unacknowledged.
pub(super) fn acknowledge(
    comparison: &mut RevisionComparison<'_>,
    supplied: &DifferenceAcknowledgements,
) {
    let index: HashMap<_, _> = supplied
        .entries
        .iter()
        .enumerate()
        .map(|(position, entry)| (entry.identity(), position))
        .collect();
    let mut used = vec![false; supplied.entries.len()];
    let matched: Vec<bool> = comparison
        .records
        .iter()
        .map(|record| {
            let candidate = entry(comparison, record);
            let position =
                candidate.and_then(|candidate| index.get(&candidate.identity()).copied());
            if let Some(position) = position {
                used[position] = true;
            }
            position.is_some()
        })
        .collect();
    for (record, matched) in comparison.records.iter_mut().zip(matched) {
        record.acknowledged = matched;
        if matched {
            comparison.acknowledged[record.verdict.classification().rank()] += 1;
        }
    }
    comparison.unused = (0..used.len())
        .filter(|&position| !used[position])
        .collect();
}

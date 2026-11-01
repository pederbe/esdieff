//! Retained identities and bounded explicit acquisition make supplied catalogues dependable.
//! Follows ADR0004: Supplied catalogues and checking scope, and
//! ADR0009: Bounded processing and reporting.

use crate::Limits;
use std::{collections::HashSet, fs::File, io::Read, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SourceId(pub(crate) u32);
impl SourceId {
    pub fn get(self) -> u32 {
        self.0
    }
}

enum Content<'a> {
    Supplied(&'a [u8]),
    Owned(Vec<u8>),
    Typed(crate::JsonValue),
    Projection(&'a [u8]),
    File(&'a Path),
    Stdin,
    Failed,
}
/// Input description with borrowed metadata and borrowed, owned or explicitly acquired content.
/// Metadata is validated before copying identities or acquiring source content.
pub struct SourceInput<'a> {
    role: SourceRole,
    key: &'a str,
    label: Option<&'a str>,
    uri: Option<&'a str>,
    version: Option<&'a str>,
    digest: Option<&'a str>,
    label_omitted: bool,
    content: Content<'a>,
}
impl<'a> SourceInput<'a> {
    pub fn supplied(key: &'a str, bytes: &'a [u8]) -> Self {
        Self::new(key, Content::Supplied(bytes))
    }
    pub fn owned(key: &'a str, bytes: Vec<u8>) -> Self {
        Self::new(key, Content::Owned(bytes))
    }
    pub fn file(key: &'a str, path: &'a Path) -> Self {
        Self::new(key, Content::File(path))
    }
    pub fn failed(key: &'a str) -> Self {
        Self::new(key, Content::Failed)
    }
    pub fn stdin(key: &'a str) -> Self {
        Self::new(key, Content::Stdin)
    }
    pub fn typed(key: &'a str, value: crate::JsonValue) -> Self {
        Self::new(key, Content::Typed(value)).sample()
    }
    pub fn sample(mut self) -> Self {
        self.role = SourceRole::Sample;
        self
    }
    fn new(key: &'a str, content: Content<'a>) -> Self {
        Self {
            role: SourceRole::Model,
            key,
            label: None,
            uri: None,
            version: None,
            digest: None,
            label_omitted: false,
            content,
        }
    }
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self.label_omitted = false;
        self
    }
    pub fn omit_label(mut self) -> Self {
        self.label = None;
        self.label_omitted = true;
        self
    }
    pub fn acquisition_uri(mut self, uri: &'a str) -> Self {
        self.uri = Some(uri);
        self
    }
    pub fn version_token(mut self, version: &'a str) -> Self {
        self.version = Some(version);
        self
    }
    pub fn digest(mut self, digest: &'a str) -> Self {
        self.digest = Some(digest);
        self
    }
    fn borrowed(&self) -> SourceInput<'_> {
        SourceInput {
            role: self.role,
            key: self.key,
            label: self.label,
            uri: self.uri,
            version: self.version,
            digest: self.digest,
            label_omitted: self.label_omitted,
            content: match &self.content {
                Content::Supplied(bytes) => Content::Supplied(bytes),
                Content::Owned(bytes) => Content::Supplied(bytes),
                Content::Typed(value) => Content::Projection(value.bytes()),
                Content::Projection(bytes) => Content::Projection(bytes),
                Content::File(path) => Content::File(path),
                Content::Stdin => Content::Stdin,
                Content::Failed => Content::Failed,
            },
        }
    }
}

pub struct Catalogue {
    pub(crate) sources: Vec<Source>,
    // Substituted sources remain immutable evidence, outside active resolution.
    pub(crate) evidence: Vec<Source>,
    pub(crate) limits: Limits,
    pub(crate) reserve: usize,
    pub(crate) numeric_remaining: usize,
    pub(crate) parsed_nodes_remaining: usize,
    pub(crate) parsed_allocation_remaining: usize,
    pub(crate) decoding_work_remaining: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SourceRole {
    Model,
    Sample,
    Generated,
}
impl SourceRole {
    pub(crate) fn is_model(self) -> bool {
        matches!(self, Self::Model | Self::Generated)
    }
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Sample => "sample",
            Self::Generated => "generated",
        }
    }
}
pub struct Source {
    pub(crate) role: SourceRole,
    pub(crate) original: bool,
    pub(crate) id: SourceId,
    pub(crate) key: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) label: Option<String>,
    pub(crate) uri: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) digest: Option<String>,
    pub(crate) raw_path: Option<Vec<u8>>,
    pub(crate) acquisition: Acquisition,
    pub(crate) label_omitted: bool,
    pub(crate) complete: bool,
    pub(crate) parsed: crate::json::Parsed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Acquisition {
    Supplied,
    Acquired,
    Failed,
    Generated,
}
impl Acquisition {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Supplied => "supplied",
            Self::Acquired => "acquired",
            Self::Failed => "failed",
            Self::Generated => "generated",
        }
    }
}
impl Catalogue {
    pub fn take_inputs(inputs: Vec<SourceInput<'_>>) -> Result<Self, &'static str> {
        Self::take_inputs_with_limits(inputs, Limits::default())
    }
    pub(crate) fn take_inputs_with_limits(
        inputs: Vec<SourceInput<'_>>,
        limits: Limits,
    ) -> Result<Self, &'static str> {
        let reserve = admit(&inputs, &limits)?;
        Self::build(inputs.into_iter(), limits, reserve)
    }
    pub fn from_inputs(inputs: &[SourceInput<'_>]) -> Result<Self, &'static str> {
        Self::with_limits(inputs, Limits::default())
    }
    pub(crate) fn with_limits(
        inputs: &[SourceInput<'_>],
        limits: Limits,
    ) -> Result<Self, &'static str> {
        let reserve = admit(inputs, &limits)?;
        Self::build(inputs.iter().map(SourceInput::borrowed), limits, reserve)
    }
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn build<'a>(
        inputs: impl Iterator<Item = SourceInput<'a>>,
        limits: Limits,
        reserve: usize,
    ) -> Result<Self, &'static str> {
        let mut remaining = limits.catalogue_bytes;
        let mut budget = crate::json::Budget::new(&limits);
        let sources = inputs
            .enumerate()
            .map(|(index, input)| {
                let cap = limits.document_bytes.min(remaining);
                let raw_path = match &input.content {
                    Content::File(path) => Some(path.as_os_str().as_encoded_bytes().to_owned()),
                    _ => None,
                };
                let original = !matches!(input.content, Content::Typed(_) | Content::Projection(_));
                let (bytes, complete, acquisition) = match input.content {
                    Content::Supplied(bytes) => (
                        bytes[..bytes.len().min(cap)].to_owned(),
                        bytes.len() <= cap,
                        Acquisition::Supplied,
                    ),
                    Content::Owned(bytes) => {
                        let complete = bytes.len() <= cap;
                        let bytes = if bytes.capacity() <= cap {
                            bytes
                        } else {
                            bytes[..bytes.len().min(cap)].to_owned()
                        };
                        (bytes, complete, Acquisition::Supplied)
                    }
                    Content::Typed(value) => {
                        let bytes = value.into_bytes();
                        let complete = bytes.len() <= cap;
                        let bytes = if bytes.capacity() <= cap {
                            bytes
                        } else {
                            bytes[..bytes.len().min(cap)].to_owned()
                        };
                        (bytes, complete, Acquisition::Supplied)
                    }
                    Content::Projection(bytes) => (
                        bytes[..bytes.len().min(cap)].to_owned(),
                        bytes.len() <= cap,
                        Acquisition::Supplied,
                    ),
                    Content::Failed => (Vec::new(), false, Acquisition::Failed),
                    Content::File(path) => match File::open(path) {
                        Err(_) => (Vec::new(), false, Acquisition::Failed),
                        Ok(mut file) => {
                            let (bytes, complete) = read_bounded(&mut file, cap);
                            (bytes, complete, Acquisition::Acquired)
                        }
                    },
                    Content::Stdin => {
                        let (bytes, complete) = read_bounded(&mut std::io::stdin().lock(), cap);
                        (bytes, complete, Acquisition::Acquired)
                    }
                };
                remaining -= bytes.capacity();
                let parsed = if acquisition == Acquisition::Failed {
                    crate::json::Parsed::unavailable()
                } else {
                    if input.role == SourceRole::Sample {
                        crate::json::parse_value(&bytes, complete, &limits, &mut budget)
                    } else {
                        crate::json::parse(&bytes, complete, &limits, &mut budget)
                    }
                };
                Source {
                    role: input.role,
                    original,
                    id: SourceId((index + 1) as u32),
                    key: input.key.to_owned(),
                    bytes,
                    label: input.label.map(str::to_owned),
                    uri: input.uri.map(str::to_owned),
                    version: input.version.map(str::to_owned),
                    digest: input.digest.map(str::to_owned),
                    raw_path,
                    parsed,
                    acquisition,
                    complete,
                    label_omitted: input.label_omitted,
                }
            })
            .collect();
        Ok(Self {
            sources,
            evidence: Vec::new(),
            limits,
            reserve,
            numeric_remaining: budget.numeric,
            parsed_nodes_remaining: budget.nodes,
            parsed_allocation_remaining: budget.allocation,
            decoding_work_remaining: budget.work,
        })
    }
    pub fn sources(&self) -> impl ExactSizeIterator<Item = &Source> {
        self.sources.iter()
    }
    pub fn source(&self, id: SourceId) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }
    pub fn source_id(&self, key: &str) -> Option<SourceId> {
        self.sources.iter().find(|s| s.key == key).map(|s| s.id)
    }
    /// Every retained acquisition/generated attempt, including substituted evidence.
    pub fn retained_sources(&self) -> impl Iterator<Item = &Source> {
        self.sources.iter().chain(&self.evidence)
    }
    pub(crate) fn attempt_count(&self) -> usize {
        self.sources.len() + self.evidence.len()
    }
    /// Lend the decode allowance to one decoding step and settle its remainders afterwards.
    ///
    /// Parsed storage, decoding work and numeric work are acquisition dimensions under
    /// ADR 0025: Selected numeric samples, so the catalogue keeps their remainders. A caller
    /// borrows them only for the duration of `decode`, which may read the catalogue meanwhile.
    pub(crate) fn lend_decoding<T>(
        &mut self,
        decode: impl FnOnce(&Self, &mut crate::json::Budget) -> T,
    ) -> T {
        let mut budget = crate::json::Budget::new(&self.limits);
        budget.nodes = self.parsed_nodes_remaining;
        budget.allocation = self.parsed_allocation_remaining;
        budget.work = self.decoding_work_remaining;
        budget.numeric = self.numeric_remaining;
        let result = decode(self, &mut budget);
        self.parsed_nodes_remaining = budget.nodes;
        self.parsed_allocation_remaining = budget.allocation;
        self.decoding_work_remaining = budget.work;
        self.numeric_remaining = budget.numeric;
        result
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_sources()
            .map(|source| source.bytes.len())
            .sum()
    }
}
fn admit(inputs: &[SourceInput<'_>], limits: &Limits) -> Result<usize, &'static str> {
    limits.validate()?;
    if !common_report_supported() {
        return Err("common report owner accounting unavailable");
    }
    if inputs.iter().any(|input| input.role == SourceRole::Sample)
        && !crate::sample::mandatory_storage_supported()
    {
        return Err("sample mandatory storage accounting unavailable");
    }
    if inputs.is_empty() || inputs.len() > limits.documents {
        return Err("one through configured source count required");
    }
    if inputs
        .iter()
        .filter(|i| matches!(i.content, Content::Stdin))
        .count()
        > 1
    {
        return Err("only one standard-input document is allowed");
    }
    let mut metadata = 0usize;
    let mut keys = HashSet::new();
    for input in inputs {
        for (value, cap) in [
            (Some(input.key), 256),
            (input.label, 512),
            (input.uri, 2048),
            (input.version, 128),
            (input.digest, 256),
        ] {
            if let Some(value) = value {
                if value.len() > cap {
                    return Err("source metadata field exceeds its byte ceiling");
                }
                metadata = metadata
                    .checked_add(value.len())
                    .ok_or("metadata size overflow")?;
            }
        }
        if let Content::File(path) = input.content {
            metadata = metadata
                .checked_add(path.as_os_str().as_encoded_bytes().len())
                .ok_or("metadata size overflow")?;
        }
        if metadata > 1024 * 1024 {
            return Err("caller metadata exceeds whole-request budget");
        }
        if !keys.insert(input.key) {
            return Err("duplicate caller source key");
        }
    }
    let reserve = metadata
        .checked_mul(6)
        .and_then(|n| n.checked_add(inputs.len() * 4096 + 16384))
        .and_then(|n| {
            n.checked_add(if inputs.iter().any(|i| i.role == SourceRole::Sample) {
                crate::sample::MANDATORY_WIRE
            } else {
                0
            })
        })
        .ok_or("report reservation overflow")?;
    if reserve > limits.report_bytes {
        return Err("report cannot retain its mandatory envelope");
    }
    Ok(reserve)
}

/// The existing C partition supplies 4*16384 fixed ownership bytes plus
/// 4*4096 per source and 24 times caller metadata bytes. No reserve is added.
/// Follows ADR 0009: Bounded processing and reporting. Both qualified serde_json
/// backends fit the source-bound map-growth model before report construction.
pub(crate) fn common_report_supported() -> bool {
    use std::mem::{align_of, size_of};
    let value = size_of::<serde_json::Value>();
    let string = size_of::<String>();
    let map = size_of::<serde_json::Map<String, serde_json::Value>>();
    let word = size_of::<usize>();
    if value > 72
        || map > 72
        || string > 24
        || word > 8
        || align_of::<serde_json::Value>() > 8
        || align_of::<crate::Report<'_>>() > 8
    {
        return false;
    }
    // IndexMap 3->7 and7->14 growth includes old/new dense/index/control
    // storage, descriptor and staged bucket. A BTree map through14 members
    // needs at most two leaves and one internal node (B=6).
    let bucket = word + string + value;
    let small = (3 + 7) * bucket + (4 + 8) * word + (4 + 8) + 2 * 16 + map + bucket;
    let large = (7 + 14) * bucket + (8 + 16) * word + (8 + 16) + 2 * 16 + map + bucket;
    let leaf = 16 + 11 * (string + value);
    let btree_small = leaf + map + string + value;
    let btree_large = 3 * leaf + 12 * word + map + string + value;
    let per_source = 4096 + 3 * 1536 + 1024 + (3 + 3 + 4) * 128 + 8 * 8;
    small <= 1536
        && btree_small <= 1536
        && large <= 4096
        && btree_large <= 4096
        && per_source <= 4 * 4096
        && crate::report::common_owner_bytes() <= 4 * 16384
}
fn read_bounded(reader: &mut impl Read, cap: usize) -> (Vec<u8>, bool) {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    while bytes.len() < cap {
        let available = chunk.len().min(cap - bytes.len());
        match reader.read(&mut chunk[..available]) {
            Ok(0) => return (bytes, true),
            Ok(size) => {
                bytes.reserve_exact(size);
                bytes.extend_from_slice(&chunk[..size]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return (bytes, false),
        }
    }
    loop {
        match reader.read(&mut chunk[..1]) {
            Ok(0) => return (bytes, true),
            Ok(_) => return (bytes, false),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return (bytes, false),
        }
    }
}
impl Source {
    /// A typed Sample projection outside any Catalogue, for a value Esdieff generates.
    /// Its identity is never a supplied source's, since those start at one.
    pub(crate) fn generated_sample(value: crate::JsonValue) -> Self {
        let (bytes, parsed) = value.into_parts();
        Self {
            role: SourceRole::Sample,
            original: false,
            id: SourceId(0),
            key: String::new(),
            bytes,
            label: None,
            uri: None,
            version: None,
            digest: None,
            raw_path: None,
            acquisition: Acquisition::Generated,
            label_omitted: false,
            complete: true,
            parsed,
        }
    }
    /// One line of a Recorded trace, decoded on its own, as a Sample source with the trace
    /// file's identity. Its spans are offsets within the line.
    pub(crate) fn trace_line(id: SourceId, bytes: Vec<u8>, parsed: crate::json::Parsed) -> Self {
        Self {
            role: SourceRole::Sample,
            original: true,
            id,
            key: String::new(),
            bytes,
            label: None,
            uri: None,
            version: None,
            digest: None,
            raw_path: None,
            acquisition: Acquisition::Supplied,
            label_omitted: false,
            complete: true,
            parsed,
        }
    }
    pub fn role(&self) -> SourceRole {
        self.role
    }
    pub fn has_original_bytes(&self) -> bool {
        self.original
    }
    pub fn sample_value(&self) -> crate::Field<crate::SampleValueView<'_>> {
        if self.role != SourceRole::Sample {
            return crate::Field::Unavailable("source is not a sample");
        }
        if self.parsed.rejected {
            return crate::Field::Unavailable("sample decoding rejected");
        }
        self.parsed.root.map_or(
            crate::Field::Unavailable("sample decoding unavailable"),
            |node| crate::Field::Present(crate::SampleValueView::new(self, node)),
        )
    }
    pub fn acquisition(&self) -> Acquisition {
        self.acquisition
    }
    pub fn json(&self) -> crate::Field<crate::JsonView<'_>> {
        if !self.original {
            return crate::Field::Unavailable("typed projection has no original-source JSON view");
        }
        if self.parsed.rejected {
            crate::Field::Unavailable("source decoding rejected")
        } else {
            self.parsed.root.map_or(
                crate::Field::Unavailable("source decoding unavailable"),
                |node| crate::Field::Present(crate::JsonView { source: self, node }),
            )
        }
    }
    pub fn id(&self) -> SourceId {
        self.id
    }
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    pub fn acquisition_uri(&self) -> Option<&str> {
        self.uri.as_deref()
    }
    pub fn version_token(&self) -> Option<&str> {
        self.version.as_deref()
    }
    pub fn digest(&self) -> Option<&str> {
        self.digest.as_deref()
    }
    pub fn raw_path_identity(&self) -> Option<&[u8]> {
        self.raw_path.as_deref()
    }
    pub fn raw_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }
    pub fn content_complete(&self) -> bool {
        self.complete
    }
}

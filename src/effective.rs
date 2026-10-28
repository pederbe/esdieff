//! Compose immutable source contributions before examining surviving reference needs.
//! Follows ADR 0006: Scoped reference failures, and
//! ADR 0013: Shared workflow and report guarantees.
//!
//! # The deliberate core of the module graph
//!
//! This module, `structure`, `sample` and `equality` depend on each other in both
//! directions on purpose. They share one traversal, one resolver, one cumulative
//! allowance and one node arena, and splitting them into layers would duplicate
//! guarantees that four decisions settled. ADR 0013: Shared workflow and report
//! guarantees completes every requested area through the existing traversal,
//! composition and required-declaration resolver on the original cumulative
//! allowance, and rejects a second resolver or a replenished allowance. ADR 0020:
//! Owned effective views and nearest origins makes the composed result own one
//! compact node arena that its typed and generic views borrow. ADR 0024: Effective
//! alternatives and nullable characteristics derives alternatives and selected
//! samples in that same arena while private checking borrows the original
//! operation budget. ADR 0031: Bounded declared-value consistency reuses the
//! selected-value evaluator and contextual equality on the same counters, with no
//! phase refill or refund.
//!
//! In code, the traversal is `structure::Structure::preflight` and the `Checker`
//! visit behind it, which walk the source once and gate composition. The resolver
//! is the private `Resolver` driven by `Catalogue::compose_mode`, which fills the
//! `EffectiveModel::nodes` arena that `EffectiveJson` borrows. The allowance is
//! `allowance::OperationBudget`, created once per operation by `Catalogue::effective`
//! and the check, sample and edit workflows, then lent to composition, coverage,
//! sample checking and `equality::EqualityWork` without replenishment. The sample
//! engine enters through `sample::select` and `Catalogue::compose_sample` into the
//! same arena. Keep these four pieces single. Do not add a second resolver, give a
//! phase its own allowance, copy the arena into a parallel graph, or move
//! `civil`, `formats`, `formal`, `modified`, `numeric` or `value` back above the
//! core. Those helpers sit below it and must stay there.
//!
//! `report`, `registry`, `units`, `content_format`, `source`, `proof` and
//! `assessment` share the same cycle for a second reason. Each owner qualifies
//! its retained report rows against fixed header sizes and the mandatory report
//! reserve before any semantic work, so `report::common_owner_bytes`,
//! `source::common_report_supported`, `registry::qualify`,
//! `sample::mandatory_storage_supported` and the `fixed_headers` functions name
//! each other's types, and `assessment::Profile::admit_reporting` holds the area
//! and reserve table every owner qualifies against. This coupling is accepted.
//! Whether size qualification should stay next to each owner is a separate
//! decision that this doc does not settle.
//!
//! `allowance` and `pattern` form a separate two-module loop because
//! `OperationBudget` carries a `pattern::MatchingWork` field and pattern matching
//! charges through `allowance::charge`. Breaking it would mean moving
//! `MatchingWork` into `allowance` or handing `pattern` the separate counters
//! instead of the whole budget. Neither change is settled.
use crate::allowance::{OperationBudget, admit, charge, push};
use crate::role::Role;
use crate::{Catalogue, Class, Field, JsonKind, JsonView, SourceId};
mod alternatives;
pub use alternatives::{AlternativeId, EffectiveAlternatives};
mod required;
pub use required::*;

#[derive(Clone, Copy, Eq, PartialEq)]
enum FailureKind {
    WorkLimit,
    AllocationLimit,
    ByteLimit,
    FeatureContext,
    Cycle,
    Interpretation,
    AlternativeIncomplete,
    Reference,
    Selector,
    Capability,
    Ambiguous,
    Syntax,
    Unmapped,
}
#[derive(Clone, Copy)]
pub(crate) struct Failure {
    kind: FailureKind,
    pub(crate) reason: &'static str,
    /// Another reach of the same authored reference already reports this
    /// definite failure.
    repeat: bool,
}
impl Failure {
    fn new(kind: FailureKind, reason: &'static str) -> Self {
        Self {
            kind,
            reason,
            repeat: false,
        }
    }
    /// Whether this failure is a finding of its own. A copy that reaches an
    /// authored reference problem again stays unavailable but adds no finding.
    pub(crate) fn reported(self) -> bool {
        !self.repeat
    }
    fn limit(reason: &'static str) -> Self {
        Self::new(FailureKind::WorkLimit, reason)
    }
    pub(crate) fn scope_limit() -> Self {
        Self::limit("dependency scope work limit")
    }
    fn allocation(reason: &'static str) -> Self {
        Self::new(FailureKind::AllocationLimit, reason)
    }
    fn bytes(reason: &'static str) -> Self {
        Self::new(FailureKind::ByteLimit, reason)
    }
    fn feature(reason: &'static str) -> Self {
        Self::new(FailureKind::FeatureContext, reason)
    }
    fn cycle(reason: &'static str) -> Self {
        Self::new(FailureKind::Cycle, reason)
    }
    fn meaning(reason: &'static str) -> Self {
        Self::new(FailureKind::Interpretation, reason)
    }
    fn alternative_incomplete() -> Self {
        Self::new(
            FailureKind::AlternativeIncomplete,
            "effective alternative meaning or capability incomplete",
        )
    }
    fn reference(reason: &'static str) -> Self {
        Self::new(FailureKind::Reference, reason)
    }
    fn selector(reason: &'static str) -> Self {
        Self::new(FailureKind::Selector, reason)
    }
    fn capability(reason: &'static str) -> Self {
        Self::new(FailureKind::Capability, reason)
    }
    fn is_limit(self) -> bool {
        matches!(
            self.kind,
            FailureKind::WorkLimit | FailureKind::AllocationLimit | FailureKind::ByteLimit
        )
    }
    pub(crate) fn code(self) -> &'static str {
        match self.kind {
            FailureKind::WorkLimit | FailureKind::AllocationLimit | FailureKind::ByteLimit => {
                "operation.limit"
            }
            FailureKind::FeatureContext => "structure.feature_context_unavailable",
            FailureKind::Cycle => "reference.effective_cycle",
            FailureKind::Interpretation | FailureKind::AlternativeIncomplete => {
                "interpretation.unresolved"
            }
            FailureKind::Reference | FailureKind::Selector => "reference.unavailable",
            FailureKind::Capability => "reference.capability_unavailable",
            FailureKind::Ambiguous => "reference.ambiguous",
            FailureKind::Syntax => "reference.syntax",
            FailureKind::Unmapped => "reference.prefix_unmapped",
        }
    }
    pub(crate) fn omission_reason(self) -> &'static str {
        match self.kind {
            FailureKind::AllocationLimit | FailureKind::ByteLimit => "byte_limit",
            _ => "not_available",
        }
    }
    pub(crate) fn definite(self) -> bool {
        matches!(self.kind, FailureKind::Syntax | FailureKind::Unmapped)
    }
}
impl From<crate::allowance::AllocationLimit> for Failure {
    fn from(_: crate::allowance::AllocationLimit) -> Self {
        Self::allocation("derived allocation limit")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OriginKind {
    Source,
    Inherited,
    Override,
    Deleted,
    Default,
}

pub struct EffectiveRequest<'a> {
    pub(crate) source: SourceId,
    pub(crate) pointer: &'a str,
}
impl<'a> EffectiveRequest<'a> {
    pub fn new(source: SourceId) -> Self {
        Self {
            source,
            pointer: "",
        }
    }
    pub fn pointer(mut self, pointer: &'a str) -> Self {
        self.pointer = pointer;
        self
    }
}

const SAMPLE_CHECK_COMMON: u8 = 1;
const SAMPLE_CHECK_NUMERIC: u8 = 1 << 1;
const SAMPLE_CHECK_TEXT: u8 = 1 << 2;
const SAMPLE_CHECK_NULLABLE: u8 = 1 << 3;
const SAMPLE_CHECK_MAP: u8 = 1 << 4;
const SAMPLE_CHECK_ARRAY: u8 = 1 << 5;

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
enum SampleStage {
    #[default]
    Deferred,
    RootExpanded,
    Finalized,
    Completed,
    Excluded,
}
pub(crate) struct Node<'a> {
    pub(crate) source: JsonView<'a>,
    // Definition contribution survives content refinement and donor substitution (ADR0026).
    pub(crate) declaration_source: JsonView<'a>,
    pub(crate) parent: Option<usize>,
    pub(crate) key: Option<&'a str>,
    pub(crate) index: Option<usize>,
    pub(crate) first: Option<usize>,
    pub(crate) next: Option<usize>,
    pub(crate) kind: OriginKind,
    pub(crate) previous: Option<JsonView<'a>>,
    pub(crate) deleted: bool,
    pub(crate) unavailable: Option<Failure>,
    pub(crate) meaning_unavailable: Option<Failure>,
    pub(crate) donor_unavailable: Option<Failure>,
    pub(crate) sample_skipped: bool,
    sample_stage: SampleStage,
    // Completed common and applicable family observations include malformed conditions.
    sample_checked: u8,
    sample_active: Option<usize>,
    context: Option<usize>,
    pub(crate) role: Option<Role>,
    pub(crate) needed_ref: Option<JsonView<'a>>,
    pub(crate) changed_context: bool,
    pub(crate) inherited_context: bool,
    pub(crate) original_role: Option<Role>,
    pub(crate) complete: bool,
    pub(crate) consumer: Option<JsonView<'a>>,
    pub(crate) conflicts: Vec<JsonView<'a>>,
    pub(crate) alternative_group: Option<usize>,
    pub(crate) alternative_scanned: bool,
}
#[derive(Clone, Copy)]
struct Boundary<'a> {
    /// The copied definition.
    target: Element<'a>,
    /// The copied definition as written, or for a composed one the deepest
    /// written element on its path, which the data-only requiredness check uses.
    written: JsonView<'a>,
    /// The arena node of the importing definition, shared by every layer of its
    /// target root chain.
    importer: usize,
    parent: Option<usize>,
    consumer: JsonView<'a>,
}
#[derive(Clone, Copy)]
struct Expansion<'a> {
    target: Element<'a>,
    parent: Option<usize>,
    depth: usize,
}
/// A name reference target selected in its document's resolved model.
#[derive(Clone, Copy)]
struct Selected<'a> {
    target: Element<'a>,
    /// The written target, or for a composed target the deepest written element
    /// on its path, which stands in for it in the data-only requiredness check
    /// and as an ambiguous contributor's location.
    written: JsonView<'a>,
    position: Position,
    /// Context ownership survives selection of nested data descriptions.
    context_data: bool,
    /// The class of the definition enclosing the target's group, which classifies
    /// a composed target's declaration.
    enclosing: Option<Class>,
}
/// An element of a resolved model: written content, or an arena node of
/// composed content. Path composition copies a definition from one, and a
/// target root chain layers them.
#[derive(Clone, Copy)]
enum Element<'a> {
    Written(JsonView<'a>),
    Composed(usize),
}
impl Element<'_> {
    fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::Written(a), Self::Written(b)) => {
                a.source_id() == b.source_id() && a.node == b.node
            }
            (Self::Composed(a), Self::Composed(b)) => a == b,
            _ => false,
        }
    }
}
#[derive(Clone, Copy)]
enum PathState {
    Composing,
    Composed,
    Unavailable(Failure),
}
/// A detached copy of a definition on a name reference's path, composed once
/// per operation. Its descendants stay unexpanded, so later copies of them see
/// what an importer would see.
#[derive(Clone, Copy)]
struct PathComposition<'a> {
    origin: Element<'a>,
    node: usize,
    state: PathState,
}
/// Boundary contexts for composed content copied into another definition.
/// Content the composed definition authored moves directly beneath `below`, the
/// importer's innermost boundary, and each nested copy boundary is repeated
/// beneath it, so nested copies keep their own boundaries. Without `below`,
/// copied nodes keep their contexts.
struct CopiedBoundaries {
    below: Option<usize>,
    copies: Vec<(usize, usize)>,
}
impl CopiedBoundaries {
    fn beneath(below: Option<usize>) -> Self {
        Self {
            below,
            copies: Vec::new(),
        }
    }
    /// Fresh composed content that keeps its contexts, for a detached path
    /// composition.
    fn detached() -> Self {
        Self::beneath(None)
    }
}
pub struct EffectiveModel<'a> {
    pub(crate) catalogue: &'a Catalogue,
    pub(crate) source: SourceId,
    pub(crate) nodes: Vec<Node<'a>>,
    pub(crate) root: Option<usize>,
    pub(crate) selected: Option<usize>,
    pub(crate) generated: Option<String>,
    pub(crate) generation_failure: Option<Failure>,
    pub(crate) derived_remaining: usize,
    pub(crate) work_remaining: usize,
    pub(crate) evaluated_roles: Vec<u64>,
    pub(crate) clear_feature: bool,
    pub(crate) failure: Option<Failure>,
    pub(crate) dependencies: Vec<SourceId>,
    pub(crate) unavailable_dependencies: Vec<SourceId>,
    pub(crate) dependency_unknown: bool,
    /// Requested work relied on `copied-reference-relocation/1` from ADR 0036:
    /// Copied reference relocation.
    pub(crate) relocation_interpretation: bool,
    complete_apart_from_required: bool,
    pub(crate) reference_errors: u32,
    /// The authored references with a counted definite failure, as source and
    /// JSON node, sorted, with the arena node that reports each.
    definite_references: Vec<(u32, usize, usize)>,
    pub(crate) dependency_contexts: Vec<(SourceId, crate::structure::FeatureContext<'a>)>,
    alternative_groups: Vec<alternatives::Group>,
    alternative_rows: Vec<alternatives::Row<'a>>,
}
#[derive(Clone, Copy)]
pub struct EffectiveJson<'a> {
    pub(crate) model: &'a EffectiveModel<'a>,
    pub(crate) node: usize,
}
pub struct EffectiveChildren<'a> {
    model: &'a EffectiveModel<'a>,
    next: Option<usize>,
}
impl<'a> Iterator for EffectiveChildren<'a> {
    type Item = EffectiveJson<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.next {
            let n = &self.model.nodes[node];
            self.next = n.next;
            if !n.deleted {
                return Some(EffectiveJson {
                    model: self.model,
                    node,
                });
            }
        }
        None
    }
}
impl<'a> EffectiveJson<'a> {
    pub(crate) fn n(self) -> &'a Node<'a> {
        &self.model.nodes[self.node]
    }
    pub(crate) fn unavailable_reason(self) -> Option<&'static str> {
        self.n().unavailable.map(|failure| failure.reason)
    }
    pub fn kind(self) -> JsonKind {
        self.n().source.kind()
    }
    pub fn text(self) -> Field<&'a str> {
        if let Some(r) = self.n().unavailable {
            Field::Unavailable(r.reason)
        } else {
            self.n().source.text()
        }
    }
    pub fn boolean(self) -> Field<bool> {
        if let Some(r) = self.n().unavailable {
            Field::Unavailable(r.reason)
        } else {
            self.n().source.boolean()
        }
    }
    pub fn number(self) -> Field<crate::ExactNumber<'a>> {
        if let Some(r) = self.n().unavailable {
            Field::Unavailable(r.reason)
        } else {
            self.n().source.number()
        }
    }
    pub fn member_name(self) -> Field<&'a str> {
        self.n().key.map_or(Field::Absent, Field::Present)
    }
    pub fn children(self) -> EffectiveChildren<'a> {
        EffectiveChildren {
            model: self.model,
            next: if self.n().unavailable.is_some() {
                None
            } else {
                self.n().first
            },
        }
    }
    pub fn get(self, name: &str) -> Field<Self> {
        if let Some(r) = self.n().unavailable {
            return Field::Unavailable(r.reason);
        }
        if self.kind() != JsonKind::Map {
            return Field::Unavailable("map shape");
        }
        let mut found = None;
        let mut work = self.model.catalogue.limits.work;
        if name.len() > 8192 {
            return Field::Unavailable("effective lookup key limit");
        }
        for child in self.children() {
            if !charge(
                &mut work,
                1 + name.len() + child.n().key.map_or(0, str::len),
            ) {
                return Field::Unavailable("effective lookup work limit");
            }
            match child.n().key {
                Some(k) if k == name => found = Some(child),
                None => return Field::Unavailable("map member identity unavailable"),
                _ => {}
            }
        }
        match found {
            Some(c) if c.n().unavailable.is_some() => {
                Field::Unavailable(c.n().unavailable.unwrap().reason)
            }
            Some(c) => Field::Present(c),
            None => Field::Absent,
        }
    }
    pub fn at(self, index: usize) -> Field<Self> {
        if let Some(r) = self.n().unavailable {
            return Field::Unavailable(r.reason);
        }
        if self.kind() != JsonKind::Array {
            return Field::Unavailable("array shape");
        }
        let mut work = self.model.catalogue.limits.work;
        for child in self.children() {
            if !charge(&mut work, 1) {
                return Field::Unavailable("effective lookup work limit");
            }
            if child.n().index == Some(index) {
                return if let Some(reason) = child.n().unavailable {
                    Field::Unavailable(reason.reason)
                } else {
                    Field::Present(child)
                };
            }
        }
        Field::Absent
    }
    /// Original contributor content, explicitly separate from this effective value.
    pub fn source_json(self) -> JsonView<'a> {
        self.n().source
    }
    pub fn root_document(self) -> SourceId {
        self.model.source
    }
    pub fn capability_complete(self) -> bool {
        self.n().complete
    }
    pub fn to_json(self) -> Field<String> {
        self.render(self.model.catalogue.limits.report_bytes)
    }
    pub(crate) fn render(self, cap: usize) -> Field<String> {
        let mut work = self.model.work_remaining;
        let mut allocation = self.model.derived_remaining;
        match self.render_budgeted(cap, &mut work, &mut allocation) {
            Ok(output) => Field::Present(output),
            Err(failure) => Field::Unavailable(failure.reason),
        }
    }
    fn render_budgeted(
        self,
        cap: usize,
        work: &mut usize,
        allocation: &mut usize,
    ) -> Result<String, Failure> {
        if !self.capability_complete() {
            return Err(Failure::meaning("effective subtree unavailable"));
        }
        self.render_measured(
            cap,
            work,
            allocation,
            "effective generation allocation limit",
        )
    }
    pub(crate) fn render_for_report(
        self,
        cap: usize,
        work: &mut usize,
        allocation: &mut usize,
    ) -> Field<String> {
        // Partial alternative presentation retains opaque derived values while
        // its explicit state carries unavailable meaning. Public generation stays strict.
        let result = self.render_measured(
            cap,
            work,
            allocation,
            "alternative presentation allocation limit",
        );
        match result {
            Ok(text) => Field::Present(text),
            Err(reason) => Field::Unavailable(reason.reason),
        }
    }
    fn render_measured(
        self,
        cap: usize,
        work: &mut usize,
        allocation: &mut usize,
        allocation_reason: &'static str,
    ) -> Result<String, Failure> {
        let size = self.walk_json(work, allocation, cap, None)?;
        if !charge(allocation, size) {
            return Err(Failure::allocation(allocation_reason));
        }
        let mut output = String::with_capacity(size);
        self.walk_json(work, allocation, cap, Some(&mut output))?;
        Ok(output)
    }
    fn walk_json(
        self,
        work: &mut usize,
        allocation: &mut usize,
        cap: usize,
        mut output: Option<&mut String>,
    ) -> Result<usize, Failure> {
        enum Frame {
            Value(usize),
            Close(char),
            Member(usize, bool),
        }
        let mut frames = Vec::new();
        push(&mut frames, Frame::Value(self.node), allocation)?;
        let mut bytes = 0;
        while let Some(frame) = frames.pop() {
            if !charge(work, 1) {
                return Err(Failure::limit("effective generation work limit"));
            }
            match frame {
                Frame::Close(ch) => emit(
                    if ch == '}' { "}" } else { "]" },
                    &mut bytes,
                    cap,
                    &mut output,
                )?,
                Frame::Member(id, comma) => {
                    if comma {
                        emit(",", &mut bytes, cap, &mut output)?;
                    }
                    let n = &self.model.nodes[id];
                    if let Some(key) = n.key {
                        if !charge(work, key.len() * 8 + 1) {
                            return Err(Failure::limit("effective generation work limit"));
                        }
                        emit_key(key, &mut bytes, cap, &mut output)?;
                        emit(":", &mut bytes, cap, &mut output)?;
                    }
                    push(&mut frames, Frame::Value(id), allocation)?;
                }
                Frame::Value(id) => {
                    let json = EffectiveJson {
                        model: self.model,
                        node: id,
                    };
                    if let Some(reason) = json.n().unavailable {
                        return Err(reason);
                    }
                    match json.kind() {
                        JsonKind::Map | JsonKind::Array => {
                            let map = json.kind() == JsonKind::Map;
                            emit(if map { "{" } else { "[" }, &mut bytes, cap, &mut output)?;
                            push(
                                &mut frames,
                                Frame::Close(if map { '}' } else { ']' }),
                                allocation,
                            )?;
                            let mut children = Vec::new();
                            for child in json.children() {
                                if map && child.n().key.is_none() {
                                    return Err(Failure::meaning(
                                        "map member identity unavailable",
                                    ));
                                }
                                if !charge(work, 1) {
                                    return Err(Failure::limit("effective generation work limit"));
                                }
                                push(&mut children, child.node, allocation)?;
                            }
                            for (index, id) in children.into_iter().enumerate().rev() {
                                push(&mut frames, Frame::Member(id, index > 0), allocation)?;
                            }
                        }
                        _ => {
                            let raw = json.n().source.raw_json();
                            if !charge(work, raw.len()) {
                                return Err(Failure::limit("effective generation work limit"));
                            }
                            emit(raw, &mut bytes, cap, &mut output)?;
                        }
                    }
                }
            }
        }
        Ok(bytes)
    }
}
/// Path compositions nested inside the lookups of other path compositions. Each
/// level recurses through `expand`, so this is a supported stack bound rather
/// than an allowance, and a lower reference depth lowers it too.
const PATH_NESTING: usize = 32;

/// A JSON Pointer array index token, or `None` when it is malformed.
fn array_index(token: &str) -> Option<usize> {
    if token.is_empty()
        || token.len() > 1 && token.starts_with('0')
        || !token.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    token.parse::<usize>().ok()
}
fn emit(
    text: &str,
    bytes: &mut usize,
    cap: usize,
    output: &mut Option<&mut String>,
) -> Result<(), Failure> {
    *bytes = bytes
        .checked_add(text.len())
        .ok_or(Failure::bytes("effective generation byte limit"))?;
    if *bytes > cap {
        return Err(Failure::bytes("effective generation byte limit"));
    }
    if let Some(output) = output {
        output.push_str(text);
    }
    Ok(())
}
fn emit_key(
    key: &str,
    bytes: &mut usize,
    cap: usize,
    output: &mut Option<&mut String>,
) -> Result<(), Failure> {
    emit("\"", bytes, cap, output)?;
    for ch in key.chars() {
        match ch {
            '"' => emit("\\\"", bytes, cap, output)?,
            '\\' => emit("\\\\", bytes, cap, output)?,
            '\n' => emit("\\n", bytes, cap, output)?,
            '\r' => emit("\\r", bytes, cap, output)?,
            '\t' => emit("\\t", bytes, cap, output)?,
            '\u{08}' => emit("\\b", bytes, cap, output)?,
            '\u{0c}' => emit("\\f", bytes, cap, output)?,
            ch if ch < '\u{20}' => {
                let hex = b"0123456789abcdef";
                let value = ch as u8;
                let escape = [
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    hex[(value / 16) as usize],
                    hex[(value % 16) as usize],
                ];
                emit(
                    std::str::from_utf8(&escape).expect("ASCII escape"),
                    bytes,
                    cap,
                    output,
                )?;
            }
            ch => {
                let mut token = [0; 4];
                emit(ch.encode_utf8(&mut token), bytes, cap, output)?;
            }
        }
    }
    emit("\"", bytes, cap, output)
}

#[derive(Clone, Copy)]
pub(crate) enum Position {
    Vocabulary(Role),
    Named(Role),
    Opaque,
}
impl Position {
    pub(crate) fn child(self, key: &str, object: bool) -> Self {
        match self {
            Self::Named(role) => Self::Vocabulary(role),
            Self::Opaque => Self::Opaque,
            Self::Vocabulary(role) => {
                if let Some(class) = crate::role::class(key) {
                    return if role.allows(class) {
                        Self::Named(Role::Definition(class))
                    } else {
                        Self::Opaque
                    };
                }
                match (role, key) {
                    (Role::Document, "info") => Self::Vocabulary(Role::Information),
                    (Role::Definition(Class::Action), "sdfInputData" | "sdfOutputData")
                    | (Role::Definition(Class::Event), "sdfOutputData") => {
                        Self::Vocabulary(Role::Data)
                    }
                    (r, "items") if r.full_data() => Self::Vocabulary(Role::DirectItem),
                    (r, "sdfChoice") if r.full_data() || matches!(r, Role::DirectItem) => {
                        Self::Named(Role::Data)
                    }
                    (r, "properties")
                        if r.full_data() || matches!(r, Role::DirectItem) && object =>
                    {
                        Self::Named(Role::Data)
                    }
                    _ => Self::Opaque,
                }
            }
        }
    }
}
#[derive(Clone, Copy)]
struct Job {
    node: usize,
    position: Position,
    active: Option<usize>,
}
#[derive(Clone, Copy)]
enum SamplePhase {
    EnterRoot,
    RawBranchRoots,
    DeriveFinal,
    PresentPairs,
    Finish,
}
#[derive(Clone, Copy)]
struct SampleJob<'a> {
    node: usize,
    position: Position,
    active: Option<usize>,
    sample: Option<crate::SampleValueView<'a>>,
    phase: SamplePhase,
    cursor: Option<usize>,
    elements: Option<crate::SampleChildren<'a>>,
    root_only: bool,
}
struct Resolver<'a> {
    patterns: crate::pattern::MatchingWork,
    sample: bool,
    model: EffectiveModel<'a>,
    work: usize,
    allocation: usize,
    nodes_left: usize,
    refs: usize,
    boundaries: Vec<Boundary<'a>>,
    expansions: Vec<Expansion<'a>>,
    paths: Vec<PathComposition<'a>>,
    path_depth: usize,
    contributions: Vec<(SourceId, Result<Option<&'a str>, Failure>)>,
    pending_conflicts: Vec<JsonView<'a>>,
    sample_jobs: Vec<SampleJob<'a>>,
}

enum ComposeMode<'a, 'view> {
    Model,
    Sample {
        value: Option<crate::SampleValueView<'a>>,
        parameters: &'view mut crate::structure::Structure<'a>,
        ledger: &'view mut crate::structure::ParameterLedger,
    },
    RequiredInspection,
}
pub(crate) fn sample_staging_fixed_headers() -> usize {
    use std::mem::size_of;
    size_of::<Resolver<'_>>()
        + 2 * size_of::<crate::structure::Structure<'_>>()
        + size_of::<crate::structure::ParameterLedger>()
        + size_of::<OperationBudget>()
        + size_of::<ComposeMode<'_, '_>>()
        + 2 * size_of::<JsonView<'_>>()
        + 4 * size_of::<Vec<usize>>()
        + 2 * size_of::<String>()
        + size_of::<crate::json::PointerBudget>()
        + crate::structure::sample_pair_fixed_headers()
        + 2 * size_of::<Node<'_>>()
        + size_of::<(usize, crate::json::Children<'_>, Option<usize>)>()
        + size_of::<(usize, crate::json::Children<'_>, bool)>()
        + size_of::<(usize, Option<usize>, Option<usize>)>()
        + size_of::<(usize, Option<usize>, bool)>()
        + 2 * size_of::<SampleJob<'_>>()
}

impl<'a> Resolver<'a> {
    fn invalidate_sample_owner(&mut self, mut node: usize) -> Result<(), Failure> {
        if !self.sample {
            return Ok(());
        }
        loop {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("sample mutation work limit"));
            }
            if self.model.nodes[node]
                .role
                .is_some_and(Role::data_description_role)
            {
                let n = &mut self.model.nodes[node];
                n.complete = false;
                n.alternative_scanned = false;
                n.alternative_group = None;
                if matches!(
                    n.sample_stage,
                    SampleStage::Finalized | SampleStage::Completed
                ) {
                    let reason = Failure::meaning("finalized sample owner changed");
                    n.meaning_unavailable = Some(reason);
                    return Err(reason);
                }
                return Ok(());
            }
            let Some(parent) = self.model.nodes[node].parent else {
                return Ok(());
            };
            node = parent;
        }
    }
    fn drive_sample(
        &mut self,
        node: usize,
        position: Position,
        active: Option<usize>,
        sample: Option<crate::SampleValueView<'a>>,
        parameters: &mut crate::structure::Structure<'a>,
        ledger: &mut crate::structure::ParameterLedger,
    ) -> Result<(), Failure> {
        push(
            &mut self.sample_jobs,
            SampleJob {
                node,
                position,
                active,
                sample,
                phase: SamplePhase::EnterRoot,
                cursor: None,
                elements: None,
                root_only: false,
            },
            &mut self.allocation,
        )?;
        while let Some(mut job) = self.sample_jobs.pop() {
            if let Err(reason) = self.sample_step(&mut job, parameters, ledger) {
                self.model.nodes[job.node].meaning_unavailable = Some(reason);
                if reason.is_limit() {
                    self.sample_jobs.clear();
                    return Err(reason);
                }
            }
        }
        Ok(())
    }
    fn sample_step(
        &mut self,
        job: &mut SampleJob<'a>,
        parameters: &mut crate::structure::Structure<'a>,
        ledger: &mut crate::structure::ParameterLedger,
    ) -> Result<(), Failure> {
        if !charge(&mut self.work, 1) {
            return Err(Failure::limit("sample composition work limit"));
        }
        match job.phase {
            SamplePhase::EnterRoot => {
                if self.model.nodes[job.node].sample_stage == SampleStage::Excluded {
                    self.model.nodes[job.node].sample_stage = SampleStage::Deferred;
                }
                if self.model.nodes[job.node].sample_stage == SampleStage::Deferred {
                    if let Position::Vocabulary(role) = job.position {
                        self.model.nodes[job.node].role = Some(role);
                        if job.active.is_none() {
                            self.model.nodes[job.node].original_role = Some(role);
                        }
                        if role.reference_context()
                            && self.model.nodes[job.node].source.kind() == JsonKind::Map
                        {
                            if matches!(role, Role::DirectItem)
                                && let Some(reference) = self.find(job.node, "sdfRef")?
                                && !self.model.nodes[reference].deleted
                            {
                                self.model.nodes[job.node].needed_ref =
                                    Some(self.model.nodes[reference].source);
                                self.model.nodes[job.node].unavailable = Some(Failure::meaning(
                                    "direct-item reference import meaning unresolved",
                                ));
                            } else {
                                let context_data = EffectiveJson {
                                    model: &self.model,
                                    node: job.node,
                                }
                                .context_data();
                                match self.expand(job.node, job.active, context_data) {
                                    Ok(active) => job.active = active,
                                    Err(reason) => self.expansion_failed(job.node, reason)?,
                                }
                            }
                        }
                        self.model.nodes[job.node].role = Some(role);
                    }
                    self.model.nodes[job.node].sample_active = job.active;
                    self.model.nodes[job.node].changed_context |= job.active.is_some();
                    self.model.nodes[job.node].sample_stage = SampleStage::RootExpanded;
                } else {
                    job.active = self.model.nodes[job.node].sample_active;
                }
                if job.root_only {
                    return Ok(());
                }
                job.phase = SamplePhase::RawBranchRoots;
                job.cursor = self
                    .find(job.node, "sdfChoice")?
                    .filter(|id| !self.model.nodes[*id].deleted)
                    .and_then(|id| self.model.nodes[id].first);
                push(
                    &mut self.sample_jobs,
                    SampleJob { ..*job },
                    &mut self.allocation,
                )?;
            }
            SamplePhase::RawBranchRoots => {
                if let Some(branch) = job.cursor {
                    job.cursor = self.model.nodes[branch].next;
                    push(
                        &mut self.sample_jobs,
                        SampleJob { ..*job },
                        &mut self.allocation,
                    )?;
                    if !self.model.nodes[branch].deleted {
                        push(
                            &mut self.sample_jobs,
                            SampleJob {
                                node: branch,
                                position: Position::Vocabulary(Role::Data),
                                active: job.active,
                                sample: job.sample,
                                phase: SamplePhase::EnterRoot,
                                cursor: None,
                                elements: None,
                                root_only: true,
                            },
                            &mut self.allocation,
                        )?;
                    }
                } else {
                    job.phase = SamplePhase::DeriveFinal;
                    push(
                        &mut self.sample_jobs,
                        SampleJob { ..*job },
                        &mut self.allocation,
                    )?;
                }
            }
            SamplePhase::DeriveFinal => {
                let role = self.model.nodes[job.node].role;
                if !role.is_some_and(Role::data_description_role)
                    || self.model.nodes[job.node].unavailable.is_some()
                {
                    return Ok(());
                }
                let derived = if self.model.nodes[job.node].alternative_scanned {
                    Ok(self.model.nodes[job.node].alternative_group)
                } else {
                    self.model.nodes[job.node].alternative_scanned = true;
                    self.derive_group(job.node)
                };
                // Copies precede applicability marking so a branch may establish an object type.
                if self.model.nodes[job.node].sample_stage == SampleStage::RootExpanded {
                    let actual_array = if job
                        .sample
                        .is_some_and(|sample| sample.kind() == JsonKind::Array)
                    {
                        let mut lease = OperationBudget::lease(std::mem::take(&mut self.work));
                        let ordinary = crate::sample::ordinary_untyped_container(
                            EffectiveJson {
                                model: &self.model,
                                node: job.node,
                            },
                            &mut lease,
                        );
                        self.work = lease.work;
                        ordinary
                    } else {
                        false
                    };
                    self.exclude_sample_qualities(job.node, actual_array)?;
                }
                match derived {
                    Ok(Some(group)) => {
                        job.cursor = Some(self.model.alternative_groups[group].start)
                    }
                    Ok(None) => {
                        let role = role.expect("data role");
                        if !self.check_sample_pair(
                            job.node,
                            role,
                            crate::sample::needed_rules(job.sample),
                            parameters,
                            ledger,
                        ) {
                            return Err(Failure::limit("sample parameter work limit"));
                        }
                        if let Some(sample) =
                            job.sample.filter(|sample| sample.kind() == JsonKind::Map)
                        {
                            let mut lease = OperationBudget::lease(std::mem::take(&mut self.work));
                            let json = EffectiveJson {
                                model: &self.model,
                                node: job.node,
                            };
                            let ordinary = crate::sample::ordinary_map_context(json, &mut lease);
                            let kind = json.lookup("type", &mut lease);
                            let properties = json.lookup("properties", &mut lease);
                            let pair = ordinary
                                && kind.present().and_then(|v| v.text().present())
                                    == Some("object")
                                && crate::sample::sample_keys_known(sample, &mut lease.work);
                            job.cursor = properties
                                .present()
                                .filter(|p| {
                                    pair && crate::sample::description_members_known(
                                        *p,
                                        &mut lease.work,
                                    )
                                })
                                .and_then(|p| p.n().first);
                            self.work = lease.work;
                        } else if let Some(sample) =
                            job.sample.filter(|sample| sample.kind() == JsonKind::Array)
                        {
                            let mut lease = OperationBudget::lease(std::mem::take(&mut self.work));
                            let json = EffectiveJson {
                                model: &self.model,
                                node: job.node,
                            };
                            let pair = crate::sample::ordinary_map_context(json, &mut lease)
                                && json
                                    .lookup("type", &mut lease)
                                    .present()
                                    .and_then(|v| v.text().present())
                                    == Some("array");
                            job.cursor = json
                                .lookup("items", &mut lease)
                                .present()
                                .filter(|_| pair)
                                .map(|v| v.node);
                            job.elements = pair.then(|| sample.children());
                            self.work = lease.work;
                        }
                    }
                    Err(reason) => {
                        if let Some(group) = self.model.nodes[job.node].alternative_group {
                            self.model.alternative_groups[group].failure = Some(reason);
                        }
                        self.model.nodes[job.node].meaning_unavailable = Some(reason);
                        job.cursor = None;
                    }
                }
                self.model.nodes[job.node].sample_stage = SampleStage::Finalized;
                job.phase = SamplePhase::PresentPairs;
                push(
                    &mut self.sample_jobs,
                    SampleJob { ..*job },
                    &mut self.allocation,
                )?;
            }
            SamplePhase::PresentPairs => {
                if let Some(group) = self.model.nodes[job.node].alternative_group {
                    if let Some(row) = job
                        .cursor
                        .filter(|row| *row < self.model.alternative_groups[group].end)
                    {
                        job.cursor = Some(row + 1);
                        let child = self.model.alternative_rows[row].node;
                        push(
                            &mut self.sample_jobs,
                            SampleJob { ..*job },
                            &mut self.allocation,
                        )?;
                        push(
                            &mut self.sample_jobs,
                            SampleJob {
                                node: child,
                                position: Position::Vocabulary(Role::Data),
                                active: self.model.nodes[child].sample_active,
                                sample: job.sample,
                                phase: SamplePhase::EnterRoot,
                                cursor: None,
                                elements: None,
                                root_only: false,
                            },
                            &mut self.allocation,
                        )?;
                        return Ok(());
                    }
                } else if job.elements.is_some() {
                    if let (Some(child), Some(sample)) =
                        (job.cursor, job.elements.as_mut().and_then(Iterator::next))
                    {
                        push(
                            &mut self.sample_jobs,
                            SampleJob { ..*job },
                            &mut self.allocation,
                        )?;
                        push(
                            &mut self.sample_jobs,
                            SampleJob {
                                node: child,
                                position: Position::Vocabulary(Role::DirectItem),
                                active: job.active,
                                sample: Some(sample),
                                phase: SamplePhase::EnterRoot,
                                cursor: None,
                                elements: None,
                                root_only: false,
                            },
                            &mut self.allocation,
                        )?;
                        return Ok(());
                    }
                } else if let Some(child) = job.cursor {
                    job.cursor = self.model.nodes[child].next;
                    push(
                        &mut self.sample_jobs,
                        SampleJob { ..*job },
                        &mut self.allocation,
                    )?;
                    if !self.model.nodes[child].deleted {
                        self.model.nodes[child].role = Some(Role::Data);
                        let value = match (job.sample, self.model.nodes[child].key) {
                            (Some(sample), Some(key)) => {
                                crate::sample::sample_member(sample, key, &mut self.work)
                            }
                            _ => Field::Unavailable("sample pair identity unavailable"),
                        };
                        if let Field::Present(sample) = value {
                            push(
                                &mut self.sample_jobs,
                                SampleJob {
                                    node: child,
                                    position: Position::Vocabulary(Role::Data),
                                    active: job.active,
                                    sample: Some(sample),
                                    phase: SamplePhase::EnterRoot,
                                    cursor: None,
                                    elements: None,
                                    root_only: false,
                                },
                                &mut self.allocation,
                            )?;
                        } else if matches!(value, Field::Absent)
                            && self.model.nodes[child].sample_stage == SampleStage::Deferred
                        {
                            self.model.nodes[child].sample_stage = SampleStage::Excluded;
                        }
                    }
                    return Ok(());
                }
                job.phase = SamplePhase::Finish;
                push(
                    &mut self.sample_jobs,
                    SampleJob { ..*job },
                    &mut self.allocation,
                )?;
            }
            SamplePhase::Finish => {
                self.model.nodes[job.node].sample_stage = SampleStage::Completed;
                self.finish_root_capabilities(job.node)?;
            }
        }
        Ok(())
    }
    fn exclude_sample_qualities(&mut self, node: usize, actual_array: bool) -> Result<(), Failure> {
        let kind = self.find(node, "type")?.and_then(|id| {
            let n = &self.model.nodes[id];
            (!n.deleted
                && n.unavailable.is_none()
                && n.meaning_unavailable.is_none()
                && n.donor_unavailable.is_none())
            .then(|| n.source.text().present())
            .flatten()
        });
        let object = kind == Some("object");
        let array = kind == Some("array");
        let mut next = self.model.nodes[node].first;
        while let Some(id) = next {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("sample applicability work limit"));
            }
            let n = &mut self.model.nodes[id];
            next = n.next;
            if !n.deleted
                && (!n.key.is_some_and(crate::sample::relevant_quality)
                    || !object && matches!(n.key, Some("properties" | "required"))
                    || !array
                        && matches!(
                            n.key,
                            Some("items" | "minItems" | "maxItems" | "uniqueItems")
                        ))
                && !(actual_array && n.key == Some("uniqueItems"))
            {
                n.sample_skipped = true;
                n.unavailable
                    .get_or_insert(Failure::meaning("outside requested sample context"));
            }
        }
        Ok(())
    }
    fn check_sample_pair(
        &mut self,
        node: usize,
        role: Role,
        mut rules: crate::structure::SampleRules,
        parameters: &mut crate::structure::Structure<'a>,
        ledger: &mut crate::structure::ParameterLedger,
    ) -> bool {
        let completed = self.model.nodes[node].sample_checked;
        let requested = (u8::from(rules.common) * SAMPLE_CHECK_COMMON)
            | (u8::from(rules.numeric) * SAMPLE_CHECK_NUMERIC)
            | (u8::from(rules.text) * SAMPLE_CHECK_TEXT)
            | (u8::from(rules.nullable) * SAMPLE_CHECK_NULLABLE)
            | (u8::from(rules.map) * SAMPLE_CHECK_MAP)
            | (u8::from(rules.array) * SAMPLE_CHECK_ARRAY);
        rules.common &= completed & SAMPLE_CHECK_COMMON == 0;
        rules.numeric &= completed & SAMPLE_CHECK_NUMERIC == 0;
        rules.text &= completed & SAMPLE_CHECK_TEXT == 0;
        rules.nullable &= completed & SAMPLE_CHECK_NULLABLE == 0;
        rules.map &= completed & SAMPLE_CHECK_MAP == 0;
        rules.array &= completed & SAMPLE_CHECK_ARRAY == 0;
        if requested & !completed == 0 {
            return true;
        }
        let mut lease = OperationBudget {
            patterns: std::mem::take(&mut self.patterns),
            work: std::mem::take(&mut self.work),
            allocation: std::mem::take(&mut self.allocation),
            nodes: std::mem::take(&mut self.nodes_left),
            references: std::mem::take(&mut self.refs),
            ..Default::default()
        };
        let complete = parameters.sample_pair(
            self.model.catalogue,
            &self.model,
            node,
            role,
            rules,
            &mut lease,
            ledger,
        );
        self.work = lease.work;
        self.allocation = lease.allocation;
        self.nodes_left = lease.nodes;
        self.refs = lease.references;
        self.patterns = lease.patterns;
        if complete {
            self.model.nodes[node].sample_checked |= requested;
        }
        complete
    }
    fn new(
        catalogue: &'a Catalogue,
        source: SourceId,
        budget: &mut OperationBudget,
        clear_feature: bool,
        evaluated_roles: Vec<u64>,
        sample: bool,
    ) -> Self {
        Self {
            patterns: std::mem::take(&mut budget.patterns),
            sample,
            model: EffectiveModel {
                catalogue,
                source,
                nodes: Vec::new(),
                root: None,
                selected: None,
                generated: None,
                generation_failure: None,
                derived_remaining: budget.allocation,
                work_remaining: budget.work,
                evaluated_roles,
                clear_feature,
                failure: None,
                dependencies: Vec::new(),
                unavailable_dependencies: Vec::new(),
                dependency_unknown: false,
                relocation_interpretation: false,
                complete_apart_from_required: false,
                reference_errors: 0,
                definite_references: Vec::new(),
                dependency_contexts: Vec::new(),
                alternative_groups: Vec::new(),
                alternative_rows: Vec::new(),
            },
            work: budget.work,
            allocation: budget.allocation,
            nodes_left: budget.nodes,
            refs: budget.references,
            boundaries: Vec::new(),
            expansions: Vec::new(),
            paths: Vec::new(),
            path_depth: 0,
            contributions: Vec::new(),
            pending_conflicts: Vec::new(),
            sample_jobs: Vec::new(),
        }
    }
    /// Counts a definite failure of `reference` reached at `node` once for the
    /// authored reference, however many copies reach it, so the error is one
    /// finding at that reference. A later reach keeps the failure unreported.
    fn count_definite(
        &mut self,
        reference: Option<JsonView<'a>>,
        node: usize,
        reason: Failure,
    ) -> Result<Failure, Failure> {
        if !reason.definite() {
            return Ok(reason);
        }
        let Some(reference) = reference else {
            self.model.reference_errors += 1;
            return Ok(reason);
        };
        if !charge(&mut self.work, 1) {
            return Err(Failure::limit("reference finding work limit"));
        }
        let key = (reference.source_id().get(), reference.node);
        match self
            .model
            .definite_references
            .binary_search_by_key(&key, |&(source, json, _)| (source, json))
        {
            Ok(at) => Ok(Failure {
                repeat: self.model.definite_references[at].2 != node,
                ..reason
            }),
            Err(at) => {
                admit(&mut self.model.definite_references, &mut self.allocation)?;
                self.model
                    .definite_references
                    .insert(at, (key.0, key.1, node));
                self.model.reference_errors += 1;
                Ok(reason)
            }
        }
    }
    /// Leaves `node` unavailable after expanding its reference failed.
    fn expansion_failed(&mut self, node: usize, reason: Failure) -> Result<(), Failure> {
        let reason = self.count_definite(self.model.nodes[node].needed_ref, node, reason)?;
        self.model.nodes[node].unavailable = Some(reason);
        self.model.nodes[node].conflicts = std::mem::take(&mut self.pending_conflicts);
        Ok(())
    }
}

impl Catalogue {
    pub fn effective<'a>(
        &'a self,
        request: EffectiveRequest<'_>,
    ) -> Result<EffectiveModel<'a>, &'static str> {
        let source = self
            .source(request.source)
            .ok_or("unknown document selector")?;
        if !source.role().is_model() {
            return Err("sample source cannot be a model root");
        }
        crate::json::validate_pointer(request.pointer)?;
        let mut budget = OperationBudget::new(&self.limits);
        self.compose_gated(request, &mut budget, true)
    }
    /// Compose a model document on `budget`, within the feature context its source
    /// establishes.
    pub(crate) fn compose_gated<'a>(
        &'a self,
        request: EffectiveRequest<'_>,
        budget: &mut OperationBudget,
        generate: bool,
    ) -> Result<EffectiveModel<'a>, &'static str> {
        let gate = self.feature_gate(request.source, budget);
        self.compose(request, budget, gate, Vec::new(), generate)
    }
    /// Compose a model document on `budget` as `compose_gated` does, without generated
    /// output, and collect the Required declaration relationships of its selected
    /// definitions.
    pub(crate) fn compose_gated_required<'a>(
        &'a self,
        request: EffectiveRequest<'_>,
        budget: &mut OperationBudget,
    ) -> Result<(EffectiveModel<'a>, Option<required::Collection<'a>>), &'static str> {
        let gate = self.feature_gate(request.source, budget);
        self.compose_inspected(request, budget, gate, Vec::new(), false, true)
    }
    /// The feature context that a model document's source establishes.
    fn feature_gate<'a>(
        &'a self,
        source: SourceId,
        budget: &mut OperationBudget,
    ) -> crate::structure::FeatureContext<'a> {
        self.source(source)
            .and_then(|source| source.json().present())
            .map_or(crate::structure::FeatureContext::Limited, |root| {
                crate::structure::Structure::preflight(self, root, budget)
            })
    }
    pub(crate) fn compose<'a>(
        &'a self,
        request: EffectiveRequest<'_>,
        budget: &mut OperationBudget,
        gate: crate::structure::FeatureContext<'a>,
        evaluated_roles: Vec<u64>,
        generate: bool,
    ) -> Result<EffectiveModel<'a>, &'static str> {
        self.compose_mode(
            request,
            budget,
            gate,
            evaluated_roles,
            generate,
            ComposeMode::Model,
        )
        .map(|(model, _)| model)
    }
    pub(crate) fn compose_sample<'a>(
        &'a self,
        request: EffectiveRequest<'_>,
        budget: &mut OperationBudget,
        gate: crate::structure::FeatureContext<'a>,
        value: Option<crate::SampleValueView<'a>>,
        parameters: &mut crate::structure::Structure<'a>,
        ledger: &mut crate::structure::ParameterLedger,
    ) -> Result<EffectiveModel<'a>, &'static str> {
        self.compose_mode(
            request,
            budget,
            gate,
            Vec::new(),
            false,
            ComposeMode::Sample {
                value,
                parameters,
                ledger,
            },
        )
        .map(|(model, _)| model)
    }
    pub(crate) fn compose_inspected<'a>(
        &'a self,
        request: EffectiveRequest<'_>,
        budget: &mut OperationBudget,
        gate: crate::structure::FeatureContext<'a>,
        evaluated_roles: Vec<u64>,
        generate: bool,
        required: bool,
    ) -> Result<(EffectiveModel<'a>, Option<required::Collection<'a>>), &'static str> {
        let mode = if required {
            ComposeMode::RequiredInspection
        } else {
            ComposeMode::Model
        };
        self.compose_mode(request, budget, gate, evaluated_roles, generate, mode)
    }
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn compose_mode<'a>(
        &'a self,
        request: EffectiveRequest<'_>,
        budget: &mut OperationBudget,
        gate: crate::structure::FeatureContext<'a>,
        evaluated_roles: Vec<u64>,
        generate: bool,
        mut mode: ComposeMode<'a, '_>,
    ) -> Result<(EffectiveModel<'a>, Option<required::Collection<'a>>), &'static str> {
        let sample = matches!(&mode, ComposeMode::Sample { .. });
        let required = matches!(&mode, ComposeMode::RequiredInspection);
        let source = self
            .source(request.source)
            .ok_or("unknown document selector")?;
        let clear_feature = matches!(gate, crate::structure::FeatureContext::Clear(_));
        let mut resolver = Resolver::new(
            self,
            request.source,
            budget,
            clear_feature,
            evaluated_roles,
            sample,
        );
        if sample {
            budget.work = 0;
            budget.allocation = 0;
            budget.nodes = 0;
            budget.references = 0;
        }
        let mut selected_position = Position::Opaque;
        if let Field::Present(root) = source.json() {
            if clear_feature {
                // The initial copy visits every parsed node. Reserve its arena once,
                // at the capacity incremental growth would reach, to avoid repeatedly
                // allocating and moving the large composition records. Keep incremental
                // admission for tight budgets, including room for the traversal frames.
                let count = source.parsed.nodes.len();
                let capacity = count.max(8).next_power_of_two();
                let bytes = capacity * std::mem::size_of::<Node<'_>>();
                if count <= resolver.nodes_left && bytes <= resolver.allocation / 2 {
                    charge(&mut resolver.allocation, bytes);
                    resolver.model.nodes.reserve_exact(capacity);
                }
                match resolver.clone_source(root, None, None, None, None, OriginKind::Source) {
                    Ok(node) => {
                        resolver.model.root = Some(node);
                        let (selected, position, active) =
                            match resolver.select_composed(request.pointer) {
                                Ok(selection) => selection,
                                Err(reason) if reason.kind == FailureKind::Selector => {
                                    return Err(reason.reason);
                                }
                                Err(reason) => {
                                    resolver.model.nodes[node].unavailable = Some(reason);
                                    (Some(node), Position::Opaque, None)
                                }
                            };
                        resolver.model.selected = selected;
                        selected_position = position;
                        let mut jobs = Vec::new();
                        if let (
                            Some(node),
                            ComposeMode::Sample {
                                value,
                                parameters,
                                ledger,
                            },
                        ) = (selected, &mut mode)
                        {
                            if let Err(reason) = resolver
                                .drive_sample(node, position, active, *value, parameters, ledger)
                            {
                                resolver.model.nodes[node].meaning_unavailable = Some(reason);
                            }
                        } else if let Some(node) = selected {
                            if push(
                                &mut jobs,
                                Job {
                                    node,
                                    position,
                                    active,
                                },
                                &mut resolver.allocation,
                            )
                            .is_ok()
                            {
                                while let Some(job) = jobs.pop() {
                                    if let Err(reason) = resolver.visit(job, &mut jobs) {
                                        resolver.model.nodes[job.node].unavailable = Some(reason);
                                    }
                                }
                            } else {
                                resolver.model.nodes[node].unavailable =
                                    Some(Failure::allocation("composition allocation limit"));
                            }
                        }
                    }
                    Err(reason) => resolver.model.failure = Some(reason),
                }
            } else {
                let reason = match gate {
                    crate::structure::FeatureContext::Deferred(_) => {
                        Failure::feature("critical feature context unavailable")
                    }
                    _ => Failure::limit("source preflight work limit"),
                };
                match resolver.shallow(root, None, None, None, None, OriginKind::Source) {
                    Ok(node) => {
                        resolver.model.root = Some(node);
                        resolver.model.selected = Some(node);
                        resolver.model.nodes[node].unavailable = Some(reason);
                        if let crate::structure::FeatureContext::Deferred(json) = gate {
                            resolver.model.nodes[node].needed_ref = Some(json);
                        }
                    }
                    Err(reason) => resolver.model.failure = Some(reason),
                }
            }
        }
        if !sample && let Err(reason) = resolver.derive_alternatives() {
            resolver.model.failure = Some(reason);
            if let Some(node) = resolver.model.selected {
                resolver.model.nodes[node].meaning_unavailable = Some(reason);
            }
        }
        if !sample && let Err(reason) = resolver.finish_capabilities() {
            if let Some(node) = resolver.model.selected {
                resolver.model.nodes[node].complete = false;
            }
            resolver.model.failure = Some(reason);
        }
        if generate && let Field::Present(json) = resolver.model.json() {
            match json.render_budgeted(
                self.limits.report_bytes,
                &mut resolver.work,
                &mut resolver.allocation,
            ) {
                Ok(output) => resolver.model.generated = Some(output),
                Err(reason) if json.capability_complete() => {
                    resolver.model.generation_failure = Some(reason)
                }
                _ => {}
            }
        }
        let relationships = if required {
            let selected = resolver
                .model
                .selected
                .map(|node| (required::ViewNode::Effective(node), selected_position));
            Some(resolver.collect_required(selected))
        } else {
            None
        };
        resolver.model.derived_remaining = resolver.allocation;
        resolver.model.work_remaining = resolver.work;
        budget.work = resolver.work;
        budget.allocation = resolver.allocation;
        budget.patterns = resolver.patterns;
        budget.nodes = resolver.nodes_left;
        budget.references = resolver.refs;
        Ok((resolver.model, relationships))
    }
}

impl EffectiveModel<'_> {
    pub fn complete(&self) -> bool {
        self.json()
            .present()
            .is_some_and(EffectiveJson::capability_complete)
    }
    /// Unit and content-format scans use this. An unfinished Required declaration
    /// relationship hides no data description, following ADR 0026: Required
    /// declaration relationships and source evidence.
    pub(crate) fn complete_apart_from_required(&self) -> bool {
        self.json().present().is_some() && self.complete_apart_from_required
    }
    /// An `sdfRequired` quality of a definition or data description holds
    /// declaration references only.
    pub(crate) fn required_quality(&self, node: usize) -> bool {
        let n = &self.nodes[node];
        n.key == Some("sdfRequired")
            && n.parent.is_some_and(|parent| {
                matches!(
                    self.nodes[parent].role,
                    Some(Role::Definition(_) | Role::Data)
                )
            })
    }
    pub fn json(&self) -> Field<EffectiveJson<'_>> {
        match self.selected {
            Some(node) if self.nodes[node].unavailable.is_none() => {
                Field::Present(EffectiveJson { model: self, node })
            }
            Some(node) => Field::Unavailable(self.nodes[node].unavailable.unwrap().reason),
            None => Field::Unavailable("effective selection unavailable"),
        }
    }
}
impl<'a> Resolver<'a> {
    fn shallow(
        &mut self,
        source: JsonView<'a>,
        parent: Option<usize>,
        key: Option<&'a str>,
        index: Option<usize>,
        context: Option<usize>,
        kind: OriginKind,
    ) -> Result<usize, Failure> {
        if !charge(&mut self.nodes_left, 1)
            || !charge(
                &mut self.work,
                1 + key.map_or(0, str::len)
                    + if matches!(source.kind(), JsonKind::Map | JsonKind::Array) {
                        0
                    } else {
                        source.raw_json().len()
                    },
            )
        {
            return Err(Failure::limit("composition work or node limit"));
        }
        let id = self.model.nodes.len();
        let consumer = parent
            .and_then(|parent| self.model.nodes[parent].consumer)
            .or_else(|| context.map(|context| self.boundaries[context].consumer));
        push(
            &mut self.model.nodes,
            Node {
                source,
                declaration_source: source,
                parent,
                key,
                index,
                first: None,
                next: None,
                kind,
                previous: None,
                deleted: false,
                unavailable: None,
                sample_skipped: false,
                sample_stage: SampleStage::Deferred,
                sample_checked: 0,
                sample_active: None,
                meaning_unavailable: None,
                donor_unavailable: None,
                context,
                role: None,
                needed_ref: None,
                changed_context: false,
                inherited_context: false,
                original_role: None,
                complete: false,
                consumer,
                conflicts: Vec::new(),
                alternative_group: None,
                alternative_scanned: false,
            },
            &mut self.allocation,
        )?;
        Ok(id)
    }
    fn clone_source(
        &mut self,
        source: JsonView<'a>,
        parent: Option<usize>,
        key: Option<&'a str>,
        index: Option<usize>,
        context: Option<usize>,
        kind: OriginKind,
    ) -> Result<usize, Failure> {
        let root = self.shallow(source, parent, key, index, context, kind)?;
        let mut frames = Vec::new();
        push(
            &mut frames,
            (root, source.children(), None::<usize>),
            &mut self.allocation,
        )?;
        while let Some((parent, children, previous)) = frames.last_mut() {
            let Some(child) = children.next() else {
                frames.pop();
                continue;
            };
            let parent = *parent;
            let id = self.shallow(
                child,
                Some(parent),
                child.n().key.as_deref(),
                child.n().index,
                context,
                kind,
            )?;
            if let Some(previous) = previous {
                self.model.nodes[*previous].next = Some(id);
            } else {
                self.model.nodes[parent].first = Some(id);
            }
            *previous = Some(id);
            push(
                &mut frames,
                (id, child.children(), None),
                &mut self.allocation,
            )?;
        }
        Ok(root)
    }
    fn find(&mut self, parent: usize, key: &str) -> Result<Option<usize>, Failure> {
        let mut child = self.model.nodes[parent].first;
        let mut found = None;
        while let Some(id) = child {
            let n = &self.model.nodes[id];
            if !charge(&mut self.work, 1 + key.len() + n.key.map_or(0, str::len)) {
                return Err(Failure::limit("composition lookup work limit"));
            }
            if n.key == Some(key) {
                found = Some(id);
            } else if n.key.is_none() {
                return Err(Failure::reference("map member identity unavailable"));
            }
            child = n.next;
        }
        Ok(found)
    }
    fn append(&mut self, parent: usize, child: usize) -> Result<(), Failure> {
        self.invalidate_sample_owner(parent)?;
        let mut last = self.model.nodes[parent].first;
        while let Some(id) = last {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("composition work limit"));
            }
            if self.model.nodes[id].next.is_none() {
                self.model.nodes[id].next = Some(child);
                return Ok(());
            }
            last = self.model.nodes[id].next;
        }
        self.model.nodes[parent].first = Some(child);
        Ok(())
    }
    fn merge(
        &mut self,
        target: usize,
        patch: JsonView<'a>,
        context: Option<usize>,
        root: bool,
    ) -> Result<(), Failure> {
        self.invalidate_sample_owner(target)?;
        let mut frames = Vec::new();
        push(
            &mut frames,
            (target, patch.children(), root),
            &mut self.allocation,
        )?;
        while let Some((parent, children, skip_ref)) = frames.last_mut() {
            let Some(patch) = children.next() else {
                frames.pop();
                continue;
            };
            let parent = *parent;
            let key = patch
                .member_name()
                .require()
                .map_err(|_| Failure::reference("map member identity unavailable"))?;
            if !charge(&mut self.work, key.len() + 1) {
                return Err(Failure::limit("composition merge work limit"));
            }
            if *skip_ref && key == "sdfRef" {
                continue;
            }
            let found = self.find(parent, key)?;
            if patch.kind() == JsonKind::Null {
                self.delete_member(parent, found, patch, key, context)?;
                continue;
            }
            if patch.kind() == JsonKind::Map {
                let id =
                    self.map_member(parent, found, patch, key, context, OriginKind::Override)?;
                push(
                    &mut frames,
                    (id, patch.children(), false),
                    &mut self.allocation,
                )?;
            } else {
                let new = self.clone_source(
                    patch,
                    Some(parent),
                    Some(key),
                    None,
                    context,
                    OriginKind::Override,
                )?;
                if let Some(id) = found {
                    self.replace(id, new)?;
                } else {
                    self.append(parent, new)?;
                }
            }
        }
        Ok(())
    }
    fn delete_member(
        &mut self,
        parent: usize,
        found: Option<usize>,
        source: JsonView<'a>,
        key: &'a str,
        context: Option<usize>,
    ) -> Result<(), Failure> {
        self.invalidate_sample_owner(parent)?;
        let id = if let Some(id) = found {
            id
        } else {
            let id = self.shallow(
                source,
                Some(parent),
                Some(key),
                None,
                context,
                OriginKind::Deleted,
            )?;
            self.append(parent, id)?;
            id
        };
        let n = &mut self.model.nodes[id];
        n.previous = found.map(|_| n.source);
        n.source = source;
        n.deleted = true;
        n.kind = OriginKind::Deleted;
        Ok(())
    }
    fn map_member(
        &mut self,
        parent: usize,
        found: Option<usize>,
        source: JsonView<'a>,
        key: &'a str,
        context: Option<usize>,
        kind: OriginKind,
    ) -> Result<usize, Failure> {
        self.invalidate_sample_owner(parent)?;
        if let Some(id) = found {
            let n = &mut self.model.nodes[id];
            if n.deleted {
                n.declaration_source = source;
            }
            if n.source.kind() != JsonKind::Map || n.deleted {
                n.first = None;
            }
            n.previous = Some(n.source);
            n.source = source;
            n.deleted = false;
            n.kind = kind;
            n.context = context;
            Ok(id)
        } else {
            let id = self.shallow(source, Some(parent), Some(key), None, context, kind)?;
            self.append(parent, id)?;
            Ok(id)
        }
    }
    fn replace(&mut self, target: usize, new: usize) -> Result<(), Failure> {
        self.invalidate_sample_owner(target)?;
        let parent = self.model.nodes[target].parent;
        let key = self.model.nodes[target].key;
        let index = self.model.nodes[target].index;
        let next = self.model.nodes[target].next;
        let previous = self.model.nodes[target].source;
        let declaration_source = if self.model.nodes[target].deleted {
            self.model.nodes[new].declaration_source
        } else {
            self.model.nodes[target].declaration_source
        };
        self.model.nodes.swap(target, new);
        let n = &mut self.model.nodes[target];
        n.parent = parent;
        n.key = key;
        n.index = index;
        n.next = next;
        n.previous = Some(previous);
        n.declaration_source = declaration_source;
        let mut child = n.first;
        while let Some(id) = child {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("composition ancestry work limit"));
            }
            self.model.nodes[id].parent = Some(target);
            child = self.model.nodes[id].next;
        }
        Ok(())
    }
    /// Copies an arena subtree. With `copy`, each node is copied fresh as composed
    /// content, even while checking a sample, and its context follows `copy`.
    fn clone_derived(
        &mut self,
        source: usize,
        parent: Option<usize>,
        key: Option<&'a str>,
        index: Option<usize>,
        mut copy: Option<&mut CopiedBoundaries>,
    ) -> Result<usize, Failure> {
        let original = &self.model.nodes[source];
        let (json, context, kind) = (original.source, original.context, original.kind);
        let root = if let Some(copy) = copy.as_deref_mut() {
            let context = self.copied_context(copy, context)?;
            let root = self.shallow(json, parent, key, index, context, kind)?;
            if parent.is_none()
                && let Some(below) = copy.below
            {
                // The importer's boundary brings the whole copy into its definition.
                self.model.nodes[root].consumer = Some(self.boundaries[below].consumer);
                self.model.nodes[root].kind = OriginKind::Inherited;
            }
            root
        } else if self.sample {
            self.copy_alternative_node(source, parent, key, index, false)?
        } else {
            self.shallow(json, parent, key, index, context, kind)?
        };
        self.model.nodes[root].declaration_source = self.model.nodes[source].declaration_source;
        let mut frames = Vec::new();
        push(
            &mut frames,
            (root, self.model.nodes[source].first, None::<usize>),
            &mut self.allocation,
        )?;
        while let Some((parent, next, previous)) = frames.last_mut() {
            let Some(source) = *next else {
                frames.pop();
                continue;
            };
            let n = &self.model.nodes[source];
            *next = n.next;
            let (json, key, index, context, kind, first, deleted, old) = (
                n.source, n.key, n.index, n.context, n.kind, n.first, n.deleted, n.previous,
            );
            let parent = *parent;
            let id = if let Some(copy) = copy.as_deref_mut() {
                let context = self.copied_context(copy, context)?;
                self.shallow(json, Some(parent), key, index, context, kind)?
            } else if self.sample {
                self.copy_alternative_node(source, Some(parent), key, index, false)?
            } else {
                self.shallow(json, Some(parent), key, index, context, kind)?
            };
            self.model.nodes[id].deleted = deleted;
            self.model.nodes[id].previous = old;
            self.model.nodes[id].declaration_source = self.model.nodes[source].declaration_source;
            if let Some(previous) = previous {
                self.model.nodes[*previous].next = Some(id);
            } else {
                self.model.nodes[parent].first = Some(id);
            }
            *previous = Some(id);
            push(&mut frames, (id, first, None), &mut self.allocation)?;
        }
        Ok(root)
    }
    /// Merges an arena patch. With `copy`, the patch is composed content from a
    /// target root chain, and its contexts follow `copy`.
    fn merge_derived(
        &mut self,
        target: usize,
        patch: usize,
        mut copy: Option<&mut CopiedBoundaries>,
    ) -> Result<(), Failure> {
        self.invalidate_sample_owner(target)?;
        let mut frames = Vec::new();
        push(
            &mut frames,
            (target, self.model.nodes[patch].first, true),
            &mut self.allocation,
        )?;
        while let Some((parent, next, root)) = frames.last_mut() {
            let Some(patch) = *next else {
                frames.pop();
                continue;
            };
            let p = &self.model.nodes[patch];
            *next = p.next;
            let (source, key, context, kind, first, deleted) = (
                p.source,
                p.key
                    .ok_or(Failure::reference("map member identity unavailable"))?,
                p.context,
                if p.kind == OriginKind::Source {
                    OriginKind::Override
                } else {
                    p.kind
                },
                p.first,
                p.deleted,
            );
            let parent = *parent;
            if !charge(&mut self.work, key.len() + 1) {
                return Err(Failure::limit("composition merge work limit"));
            }
            if *root && key == "sdfRef" {
                continue;
            }
            let context = match copy.as_deref_mut() {
                Some(copy) => self.copied_context(copy, context)?,
                None => context,
            };
            let found = self.find(parent, key)?;
            if deleted || source.kind() == JsonKind::Null {
                self.delete_member(parent, found, source, key, context)?;
                continue;
            }
            if source.kind() == JsonKind::Map {
                let introduced = found.is_none_or(|id| self.model.nodes[id].deleted);
                let id = self.map_member(parent, found, source, key, context, kind)?;
                if introduced {
                    self.model.nodes[id].declaration_source =
                        self.model.nodes[patch].declaration_source;
                }
                push(&mut frames, (id, first, false), &mut self.allocation)?;
            } else {
                let new =
                    self.clone_derived(patch, Some(parent), Some(key), None, copy.as_deref_mut())?;
                self.model.nodes[new].kind = kind;
                if let Some(id) = found {
                    self.replace(id, new)?;
                } else {
                    self.append(parent, new)?;
                }
            }
        }
        Ok(())
    }
    fn copied_context(
        &mut self,
        copy: &mut CopiedBoundaries,
        context: Option<usize>,
    ) -> Result<Option<usize>, Failure> {
        let Some(below) = copy.below else {
            return Ok(context);
        };
        let Some(mut id) = context else {
            return Ok(Some(below));
        };
        let mut chain = Vec::new();
        let mut parent = loop {
            if !charge(&mut self.work, 1 + copy.copies.len()) {
                return Err(Failure::limit("composition boundary work limit"));
            }
            if let Some((_, copied)) = copy.copies.iter().find(|(original, _)| *original == id) {
                break *copied;
            }
            push(&mut chain, id, &mut self.allocation)?;
            match self.boundaries[id].parent {
                Some(parent) => id = parent,
                None => break below,
            }
        };
        for original in chain.into_iter().rev() {
            let copied = self.boundaries.len();
            let boundary = Boundary {
                parent: Some(parent),
                ..self.boundaries[original]
            };
            push(&mut self.boundaries, boundary, &mut self.allocation)?;
            push(&mut copy.copies, (original, copied), &mut self.allocation)?;
            parent = copied;
        }
        Ok(Some(parent))
    }
    fn source_field(
        &mut self,
        json: JsonView<'a>,
        name: &str,
    ) -> Result<Option<JsonView<'a>>, Failure> {
        if json.kind() != JsonKind::Map {
            return Err(Failure::reference("reference map shape unavailable"));
        }
        let mut found = None;
        for child in json.children() {
            let Some(key) = child.n().key.as_deref() else {
                return Err(Failure::reference("reference member identity unavailable"));
            };
            if !charge(&mut self.work, key.len() + name.len() + 1) {
                return Err(Failure::limit("reference lookup work limit"));
            }
            if key == name {
                found = Some(child);
            }
        }
        Ok(found)
    }
    /// Selects the target of name reference `reference`, read in the context of
    /// the document that wrote it. A `copied` reference was copied into the
    /// requested model.
    fn pointer_source(
        &mut self,
        reference: JsonView<'a>,
        copied: bool,
    ) -> Result<Selected<'a>, Failure> {
        let text = reference.text().require().map_err(|_| {
            self.model.dependency_unknown = true;
            Failure::meaning("reference meaning unavailable")
        })?;
        let prior_unknown = self.model.dependency_unknown;
        if !text.starts_with('#') {
            self.model.dependency_unknown = true;
        }
        if !charge(&mut self.work, text.len() * 8 + 1) {
            return Err(Failure::limit("reference syntax work limit"));
        }
        let assembled;
        let (namespace, fragment) = if text.starts_with('#') {
            (None, text)
        } else {
            assembled = self.assemble_global(reference, text, copied)?;
            let Some((namespace, tail)) = assembled.split_once('#') else {
                return Err(Failure::capability(
                    "global name without target fragment unavailable",
                ));
            };
            (
                Some(namespace),
                &assembled[namespace.len()..namespace.len() + tail.len() + 1],
            )
        };
        let pointer = self.decode_fragment(fragment).map_err(|reason| {
            if namespace.is_some() && reason.kind == FailureKind::Reference {
                Failure::capability("global target-fragment selection capability unavailable")
            } else {
                reason
            }
        })?;
        if let Some(namespace) = namespace {
            let catalogue = self.model.catalogue;
            let mut found: Option<Selected<'a>> = None;
            let mut unknown = None;
            let mut conflicts = Vec::new();
            for source in catalogue.sources() {
                if !source.role().is_model() {
                    continue;
                }
                let contribution = self.contribution(source.id());
                let name = match contribution {
                    Ok(Some(name)) => name,
                    Ok(None) => continue,
                    Err(reason) => {
                        self.unavailable_source(source.id())?;
                        unknown = Some(reason);
                        continue;
                    }
                };
                if !charge(&mut self.work, namespace.len() + name.len() + 1) {
                    return Err(Failure::limit("namespace comparison work limit"));
                }
                if namespace != name {
                    continue;
                }
                // Each contributor addresses the element at the pointer in its
                // resolved model, following ADR 0036: Copied reference relocation.
                match self.select_resolved(
                    source
                        .json()
                        .require()
                        .map_err(|_| Failure::reference("contributor unavailable"))?,
                    &pointer,
                ) {
                    Ok(Some(target))
                        if matches!(
                            target.position,
                            Position::Vocabulary(Role::Definition(_) | Role::Data)
                        ) =>
                    {
                        if let Some(previous) = found {
                            if conflicts.is_empty() {
                                push(&mut conflicts, previous.written, &mut self.allocation)?;
                            }
                            push(&mut conflicts, target.written, &mut self.allocation)?;
                        } else {
                            found = Some(target);
                        }
                    }
                    Ok(Some(_)) => {
                        self.unavailable_source(source.id())?;
                        unknown = Some(Failure::capability(
                            "addressed global definition role capability unavailable",
                        ));
                    }
                    Ok(None) => {}
                    Err(reason) => {
                        self.unavailable_source(source.id())?;
                        unknown = Some(reason);
                    }
                }
            }
            if !conflicts.is_empty() {
                self.pending_conflicts = conflicts;
                self.model.dependency_unknown = true;
                return Err(Failure::new(
                    FailureKind::Ambiguous,
                    "multiple supplied contributors address this global definition",
                ));
            }
            if let Some(reason) = unknown {
                self.model.dependency_unknown = true;
                return Err(reason);
            }
            let target = found.ok_or_else(|| {
                self.model.dependency_unknown = true;
                Failure::reference("supplied global target unavailable")
            })?;
            self.need_source(target.written.source_id())?;
            self.model.dependency_unknown = prior_unknown;
            Ok(target)
        } else {
            let root = reference
                .source
                .json()
                .require()
                .map_err(|_| Failure::reference("reference target unavailable"))?;
            self.select_resolved(root, &pointer)?
                .ok_or(Failure::reference("reference target unavailable"))
        }
    }
    /// Selects `pointer` in the resolved model of the document rooted at `root`.
    /// Each `sdfRef` on the path is composed before descending, in a detached copy
    /// that the requested traversal never visits. The target's own `sdfRef` stays
    /// for the caller's target root chain. Follows ADR 0036: Copied reference
    /// relocation.
    fn select_resolved(
        &mut self,
        root: JsonView<'a>,
        pointer: &str,
    ) -> Result<Option<Selected<'a>>, Failure> {
        let malformed = Failure::reference("reference target unavailable");
        let mut position = Position::Vocabulary(Role::Document);
        let mut enclosing = None;
        let mut context_data = false;
        // The deepest written element on the path, which is the target until a
        // composed element is entered.
        let mut written = Some(root);
        let mut view = root;
        let mut composed: Option<usize> = None;
        let mut tokens = pointer
            .strip_prefix('/')
            .into_iter()
            .flat_map(|tail| tail.split('/'))
            .peekable();
        while let Some(token) = tokens.next() {
            if !charge(&mut self.work, token.len() * 4 + 1)
                || !charge(&mut self.allocation, token.len() * 2)
            {
                return Err(Failure::limit(
                    "reference selector work or allocation limit",
                ));
            }
            if let Position::Vocabulary(role) = position
                && role.reference_context()
            {
                let origin = match composed {
                    None if view.kind() == JsonKind::Map
                        && self.source_field(view, "sdfRef")?.is_some() =>
                    {
                        Some(Element::Written(view))
                    }
                    Some(node)
                        if self.model.nodes[node].source.kind() == JsonKind::Map
                            && self
                                .find(node, "sdfRef")?
                                .is_some_and(|id| !self.model.nodes[id].deleted) =>
                    {
                        Some(Element::Composed(node))
                    }
                    _ => None,
                };
                if let Some(origin) = origin {
                    if matches!(role, Role::DirectItem) {
                        return Err(Failure::meaning(
                            "direct-item reference import meaning unresolved",
                        ));
                    }
                    composed = Some(self.compose_path(
                        origin,
                        context_data || matches!(role, Role::Definition(Class::Context)),
                    )?);
                }
            }
            if let Position::Vocabulary(Role::Definition(class)) = position {
                enclosing = Some(class);
                context_data = class == Class::Context;
            } else if !matches!(position, Position::Named(_)) {
                enclosing = None;
            }
            let name = token.replace("~1", "/").replace("~0", "~");
            let Some(node) = composed else {
                let Some((next, next_position)) =
                    self.written_child(view, &name, position, malformed)?
                else {
                    return Ok(None);
                };
                position = next_position;
                view = next;
                written = Some(next);
                continue;
            };
            // Follow the written document beside the composed path for as long
            // as it has the same element.
            if let Some(current) = written {
                written = self
                    .written_child(current, &name, position, malformed)?
                    .map(|(next, _)| next);
                if let Some(next) = written {
                    view = next;
                }
            }
            let object = matches!(position, Position::Vocabulary(Role::DirectItem))
                && self.find(node, "type")?.is_some_and(|id| {
                    matches!(self.model.nodes[id].source.text(), Field::Present("object"))
                });
            position = position.child(&name, object);
            let next = match self.model.nodes[node].source.kind() {
                JsonKind::Map => self.find(node, &name)?,
                JsonKind::Array => {
                    position = Position::Opaque;
                    let index = array_index(&name).ok_or(malformed)?;
                    self.indexed_child(node, index, "reference lookup work limit")?
                }
                _ => None,
            };
            match next {
                Some(id) if !self.model.nodes[id].deleted => composed = Some(id),
                _ => {
                    // The target is missing. When the written document has an
                    // element at the target's pointer, composition removed it.
                    if tokens.peek().is_none() && written.is_some() {
                        self.model.relocation_interpretation = true;
                    }
                    return Ok(None);
                }
            }
        }
        let target = match composed {
            Some(node) => {
                if self.composed_only(node, written)? {
                    self.model.relocation_interpretation = true;
                }
                Element::Composed(node)
            }
            None => Element::Written(view),
        };
        Ok(Some(Selected {
            target,
            written: view,
            position,
            context_data: (context_data
                || matches!(
                    position,
                    Position::Vocabulary(Role::Definition(Class::Context))
                ))
                && matches!(position, Position::Vocabulary(role) if role.full_data()),
            enclosing,
        }))
    }
    /// Whether a composed target was found only through composition: the written
    /// document has no element at its coordinate, or the target holds content
    /// that the written element does not, such as a copied quality or a deletion.
    fn composed_only(
        &mut self,
        node: usize,
        written: Option<JsonView<'a>>,
    ) -> Result<bool, Failure> {
        let Some(written) = written else {
            return Ok(true);
        };
        let mut pending = Vec::new();
        push(&mut pending, node, &mut self.allocation)?;
        while let Some(id) = pending.pop() {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("reference selector work limit"));
            }
            let n = &self.model.nodes[id];
            if n.deleted || !self.ancestor(n.source, written)? {
                return Ok(true);
            }
            let mut child = self.model.nodes[id].first;
            while let Some(id) = child {
                push(&mut pending, id, &mut self.allocation)?;
                child = self.model.nodes[id].next;
            }
        }
        Ok(false)
    }
    /// One written step of a pointer's path, with the child's position. A
    /// malformed array index fails with `malformed`.
    fn written_child(
        &mut self,
        current: JsonView<'a>,
        name: &str,
        position: Position,
        malformed: Failure,
    ) -> Result<Option<(JsonView<'a>, Position)>, Failure> {
        match current.kind() {
            JsonKind::Map => {
                let object = if matches!(position, Position::Vocabulary(Role::DirectItem)) {
                    self.source_field(current, "type")?
                        .is_some_and(|json| matches!(json.text(), Field::Present("object")))
                } else {
                    false
                };
                Ok(self
                    .source_field(current, name)?
                    .map(|next| (next, position.child(name, object))))
            }
            JsonKind::Array => {
                let index = array_index(name).ok_or(malformed)?;
                let mut found = None;
                for child in current.children() {
                    if !charge(&mut self.work, 1) {
                        return Err(Failure::limit("reference lookup work limit"));
                    }
                    if child.n().index == Some(index) {
                        found = Some(child);
                    }
                }
                Ok(found.map(|next| (next, Position::Opaque)))
            }
            _ => Ok(None),
        }
    }
    /// The child of arena array `node` at `index`. Each child scanned costs one
    /// unit of work, and exhaustion fails with `limit`.
    fn indexed_child(
        &mut self,
        node: usize,
        index: usize,
        limit: &'static str,
    ) -> Result<Option<usize>, Failure> {
        let mut child = self.model.nodes[node].first;
        let mut found = None;
        while let Some(id) = child {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit(limit));
            }
            if self.model.nodes[id].index == Some(index) {
                found = Some(id);
            }
            child = self.model.nodes[id].next;
        }
        Ok(found)
    }
    /// Composes a definition on a name reference's path once per operation, in a
    /// detached copy with its own expansion chain. Reaching it again while it
    /// composes is a cycle. Nested path compositions recurse, so their nesting
    /// has a fixed stack bound.
    fn compose_path(&mut self, origin: Element<'a>, context_data: bool) -> Result<usize, Failure> {
        for path in &self.paths {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("path composition work limit"));
            }
            if path.origin.same(origin) {
                return match path.state {
                    PathState::Composing => Err(Failure::cycle("effective reference cycle")),
                    PathState::Composed => Ok(path.node),
                    PathState::Unavailable(reason) => Err(reason),
                };
            }
        }
        if self.path_depth >= PATH_NESTING.min(self.model.catalogue.limits.reference_depth) {
            return Err(Failure::limit("reference path nesting limit"));
        }
        let node = match origin {
            Element::Written(json) => self.clone_source(
                json,
                None,
                json.member_name().present(),
                json.n().index,
                None,
                OriginKind::Source,
            )?,
            Element::Composed(node) => {
                let (key, index) = (self.model.nodes[node].key, self.model.nodes[node].index);
                self.clone_derived(
                    node,
                    None,
                    key,
                    index,
                    Some(&mut CopiedBoundaries::detached()),
                )?
            }
        };
        let entry = self.paths.len();
        push(
            &mut self.paths,
            PathComposition {
                origin,
                node,
                state: PathState::Composing,
            },
            &mut self.allocation,
        )?;
        self.path_depth += 1;
        let result = self.expand(node, None, context_data);
        self.path_depth -= 1;
        match result {
            Ok(_) => {
                self.paths[entry].state = PathState::Composed;
                Ok(node)
            }
            Err(reason) => {
                // The definition's own reference reports its cause where it is
                // requested. The path through it is only unavailable.
                self.pending_conflicts.clear();
                let reason = if reason.is_limit() || reason.kind == FailureKind::Cycle {
                    reason
                } else {
                    Failure::reference("reference path composition unavailable")
                };
                self.paths[entry].state = PathState::Unavailable(reason);
                Err(reason)
            }
        }
    }
    /// Assembles a CURIE with the namespace mappings of the document that wrote
    /// `reference`. Reading a `copied` reference this way relies on ADR 0036:
    /// Copied reference relocation, even when its prefix has no mapping there.
    fn assemble_global(
        &mut self,
        reference: JsonView<'a>,
        text: &str,
        copied: bool,
    ) -> Result<String, Failure> {
        let (prefix, tail) = self.curie_parts(text)?;
        if copied {
            self.model.relocation_interpretation = true;
        }
        let root = reference
            .source
            .json()
            .require()
            .map_err(|_| Failure::reference("binding context unavailable"))?;
        let map = self.source_field(root, "namespace")?.ok_or(Failure::new(
            FailureKind::Unmapped,
            "reference prefix has no namespace mapping",
        ))?;
        let binding = self.source_field(map, prefix)?.ok_or(Failure::new(
            FailureKind::Unmapped,
            "reference prefix has no namespace mapping",
        ))?;
        let binding = binding
            .text()
            .require()
            .map_err(|_| Failure::reference("namespace binding unavailable"))?;
        let size = binding
            .len()
            .checked_add(tail.len())
            .ok_or(Failure::limit("reference assembly limit"))?;
        if !charge(&mut self.work, size * 8 + 1) || !charge(&mut self.allocation, size) {
            return Err(Failure::limit("reference assembly limit"));
        }
        let mut assembled = String::with_capacity(size);
        assembled.push_str(binding);
        assembled.push_str(tail);
        if !crate::uri::valid_uri(&assembled) {
            return Err(Failure::capability(
                "assembled global name capability unavailable",
            ));
        }
        Ok(assembled)
    }
    fn curie_parts<'text>(
        &mut self,
        text: &'text str,
    ) -> Result<(&'text str, &'text str), Failure> {
        if !charge(&mut self.work, text.len() * 8 + 1) {
            return Err(Failure::limit("reference syntax work limit"));
        }
        let Some((prefix, tail)) = text.split_once(':') else {
            return Err(Failure::meaning("reference meaning unavailable"));
        };
        if prefix.is_empty() {
            return Err(Failure::meaning(
                "default-prefix reference meaning unresolved",
            ));
        }
        if !prefix.is_ascii() {
            return Err(Failure::capability(
                "non-ASCII NCName prefix capability unavailable",
            ));
        }
        if !prefix
            .as_bytes()
            .first()
            .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
            || !prefix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            || !crate::uri::valid_relative_reference(tail)
        {
            return Err(Failure::new(
                FailureKind::Syntax,
                "malformed established CURIE syntax",
            ));
        }
        Ok((prefix, tail))
    }
    fn decode_fragment(&mut self, text: &str) -> Result<String, Failure> {
        if !charge(&mut self.work, text.len() * 4 + 1)
            || !charge(&mut self.allocation, text.len() * 2)
        {
            return Err(Failure::limit("reference decoding limit"));
        }
        let mut bytes = Vec::with_capacity(text.len());
        let raw = &text.as_bytes()[1..];
        let mut i = 0;
        while i < raw.len() {
            if raw[i] == b'%' {
                let a = *raw
                    .get(i + 1)
                    .ok_or(Failure::reference("reference fragment unavailable"))?;
                let b = *raw
                    .get(i + 2)
                    .ok_or(Failure::reference("reference fragment unavailable"))?;
                let hex = |v: u8| (v as char).to_digit(16).map(|n| n as u8);
                bytes.push(
                    hex(a)
                        .zip(hex(b))
                        .map(|(a, b)| a * 16 + b)
                        .ok_or(Failure::reference("reference fragment unavailable"))?,
                );
                i += 3;
            } else {
                bytes.push(raw[i]);
                i += 1;
            }
        }
        let pointer = std::str::from_utf8(&bytes)
            .map_err(|_| Failure::reference("reference fragment unavailable"))?;
        crate::json::validate_pointer(pointer)
            .map_err(|_| Failure::reference("reference fragment unavailable"))?;
        Ok(pointer.to_owned())
    }
    /// Selects an inspection selector in the document as written.
    fn select_inspected(
        &mut self,
        mut current: JsonView<'a>,
        pointer: &str,
    ) -> Result<Option<(JsonView<'a>, Position)>, Failure> {
        let malformed = Failure::selector("unknown inspection selector");
        let mut position = Position::Vocabulary(Role::Document);
        for token in pointer
            .strip_prefix('/')
            .into_iter()
            .flat_map(|tail| tail.split('/'))
        {
            if !charge(&mut self.work, token.len() * 4 + 1)
                || !charge(&mut self.allocation, token.len() * 2)
            {
                return Err(Failure::limit(
                    "reference selector work or allocation limit",
                ));
            }
            let name = token.replace("~1", "/").replace("~0", "~");
            let Some(next) = self.written_child(current, &name, position, malformed)? else {
                return Ok(None);
            };
            (current, position) = next;
        }
        Ok(Some((current, position)))
    }
    fn contribution(&mut self, id: SourceId) -> Result<Option<&'a str>, Failure> {
        if !charge(&mut self.work, 1) {
            return Err(Failure::limit("namespace index work limit"));
        }
        for (source, result) in &self.contributions {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("namespace index work limit"));
            }
            if *source == id {
                return *result;
            }
        }
        let result = self.contribution_uncached(id);
        push(&mut self.contributions, (id, result), &mut self.allocation)?;
        result
    }
    fn gate(&mut self, root: JsonView<'a>) -> Result<(), Failure> {
        let mut budget = OperationBudget {
            patterns: Default::default(),
            work: self.work,
            allocation: self.allocation,
            nodes: self.nodes_left,
            references: self.refs,
            ..Default::default()
        };
        let gate = crate::structure::Structure::preflight(self.model.catalogue, root, &mut budget);
        self.work = budget.work;
        self.allocation = budget.allocation;
        push(
            &mut self.model.dependency_contexts,
            (root.source_id(), gate),
            &mut self.allocation,
        )?;
        match gate {
            crate::structure::FeatureContext::Clear(_) => Ok(()),
            crate::structure::FeatureContext::Deferred(_) => Err(Failure::feature(
                "contributor critical feature context unavailable",
            )),
            _ => Err(Failure::limit("contributor preflight work limit")),
        }
    }
    fn contribution_uncached(&mut self, id: SourceId) -> Result<Option<&'a str>, Failure> {
        let source = self.model.catalogue.source(id).expect("catalogue source");
        let root = source
            .json()
            .require()
            .map_err(|_| Failure::reference("possible namespace contribution unavailable"))?;
        self.gate(root)?;
        let Some(default) = self.source_field(root, "defaultNamespace")? else {
            return Ok(None);
        };
        let prefix = default
            .text()
            .require()
            .map_err(|_| Failure::reference("namespace contribution identity unavailable"))?;
        let map = self
            .source_field(root, "namespace")?
            .ok_or(Failure::reference(
                "namespace contribution identity unavailable",
            ))?;
        let name = self.source_field(map, prefix)?.ok_or(Failure::reference(
            "namespace contribution identity unavailable",
        ))?;
        let name = name
            .text()
            .require()
            .map_err(|_| Failure::reference("namespace contribution identity unavailable"))?;
        if !charge(&mut self.work, name.len() * 8 + 1) {
            return Err(Failure::limit("namespace identity work limit"));
        }
        if !crate::uri::valid_uri(name) {
            return Err(Failure::reference(
                "namespace contribution identity unavailable",
            ));
        }
        Ok(Some(name))
    }
    fn need_source(&mut self, id: SourceId) -> Result<(), Failure> {
        for existing in &self.model.dependencies {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("dependency closure work limit"));
            }
            if *existing == id {
                return Ok(());
            }
        }
        if id != self.model.source {
            push(&mut self.model.dependencies, id, &mut self.allocation)?;
        }
        Ok(())
    }
    fn unavailable_source(&mut self, id: SourceId) -> Result<(), Failure> {
        for existing in &self.model.unavailable_dependencies {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("dependency uncertainty work limit"));
            }
            if *existing == id {
                return Ok(());
            }
        }
        push(
            &mut self.model.unavailable_dependencies,
            id,
            &mut self.allocation,
        )?;
        Ok(())
    }
    fn expand(
        &mut self,
        node: usize,
        active: Option<usize>,
        context_data: bool,
    ) -> Result<Option<usize>, Failure> {
        let source = self.model.nodes[node].source;
        // The `sdfRef` member's own context decides whether it is copied, so a patch
        // that touches only sibling qualities leaves it copied, following ADR 0036:
        // Copied reference relocation. Its source JSON belongs to the document that
        // wrote it, whose namespace mappings assemble a copied CURIE.
        let (first_ref, mut context) = match self.find(node, "sdfRef")? {
            Some(id) if !self.model.nodes[id].deleted => {
                (self.model.nodes[id].source, self.model.nodes[id].context)
            }
            _ => return Ok(active),
        };
        let mut reference = first_ref;
        // The boundary the current reference was copied through, if any.
        let mut copied_through = context;
        // The current layer of the target root chain. `None` is the expanding node
        // itself, which merges last as the consumer's patch.
        let mut current: Option<Element<'a>> = None;
        let mut active = active;
        let mut patches = Vec::new();
        loop {
            self.model.nodes[node].needed_ref = Some(reference);
            if !charge(&mut self.refs, 1) || !charge(&mut self.work, 1) {
                if reference
                    .text()
                    .present()
                    .is_none_or(|text| !text.starts_with('#'))
                {
                    self.model.dependency_unknown = true;
                }
                return Err(Failure::limit("reference application limit"));
            }
            let selected = self.pointer_source(reference, copied_through.is_some())?;
            let (selected, _) = self.relocate(selected, copied_through)?;
            // Context-to-Context reuse keeps the same absent-default boundary.
            // Exporting that content into a base-SDF role still needs a policy
            // for defaults, so retain its scoped limitation under ADR 0047.
            if selected.context_data && !context_data {
                return Err(Failure::capability(
                    "reference reuse of Context data outside Context is not yet supported",
                ));
            }
            if !matches!(
                selected.position,
                Position::Vocabulary(Role::Definition(_) | Role::Data)
            ) {
                return Err(Failure::meaning("reference target role unavailable"));
            }
            let kind = match selected.target {
                Element::Written(json) => json.kind(),
                Element::Composed(target) => self.model.nodes[target].source.kind(),
            };
            if kind != JsonKind::Map {
                return Err(Failure::meaning("reference target role unavailable"));
            }
            let mut previous = active;
            let mut depth = 1;
            while let Some(id) = previous {
                if !charge(&mut self.work, 1) {
                    return Err(Failure::limit("reference cycle work limit"));
                }
                let e = self.expansions[id];
                depth = depth.max(e.depth + 1);
                if selected.target.same(e.target) {
                    return Err(Failure::cycle("effective reference cycle"));
                }
                previous = e.parent;
            }
            if selected.target.same(Element::Written(source)) {
                return Err(Failure::cycle("effective reference cycle"));
            }
            if depth > self.model.catalogue.limits.reference_depth {
                return Err(Failure::limit("reference chain depth limit"));
            }
            let expansion = self.expansions.len();
            push(
                &mut self.expansions,
                Expansion {
                    target: selected.target,
                    parent: active,
                    depth,
                },
                &mut self.allocation,
            )?;
            active = Some(expansion);
            let boundary = self.boundaries.len();
            push(
                &mut self.boundaries,
                Boundary {
                    target: selected.target,
                    written: selected.written,
                    importer: node,
                    parent: context,
                    consumer: first_ref,
                },
                &mut self.allocation,
            )?;
            if let Some(layer) = current {
                push(&mut patches, (layer, context), &mut self.allocation)?;
            }
            context = Some(boundary);
            current = Some(selected.target);
            // A written target's own reference is original in its document. A
            // composed target's reference may itself have been copied.
            let next = match selected.target {
                Element::Composed(target) => self
                    .find(target, "sdfRef")?
                    .filter(|id| !self.model.nodes[*id].deleted)
                    .map(|id| (self.model.nodes[id].source, self.model.nodes[id].context)),
                Element::Written(target) => self
                    .source_field(target, "sdfRef")?
                    .map(|json| (json, None)),
            };
            (reference, copied_through) = match next {
                None => break,
                Some(next) => next,
            };
        }
        let new = match current.expect("an expanded reference has a target") {
            Element::Written(json) => {
                self.clone_source(json, None, None, None, context, OriginKind::Inherited)?
            }
            Element::Composed(target) => self.clone_derived(
                target,
                None,
                None,
                None,
                Some(&mut CopiedBoundaries::beneath(context)),
            )?,
        };
        for (patch, patch_context) in patches.into_iter().rev() {
            match patch {
                Element::Written(json) => self.merge(new, json, patch_context, true)?,
                Element::Composed(patch) => self.merge_derived(
                    new,
                    patch,
                    Some(&mut CopiedBoundaries::beneath(patch_context)),
                )?,
            }
        }
        self.merge_derived(new, node, None)?;
        self.replace(node, new)?;
        Ok(active)
    }
    fn visit(&mut self, job: Job, jobs: &mut Vec<Job>) -> Result<(), Failure> {
        if !charge(&mut self.work, 1) {
            return Err(Failure::limit("composition work limit"));
        }
        let mut active = job.active;
        if let Position::Vocabulary(role) = job.position {
            self.model.nodes[job.node].role = Some(role);
            if job.active.is_none() {
                self.model.nodes[job.node].original_role = Some(role);
            }
            if role.reference_context() && self.model.nodes[job.node].source.kind() == JsonKind::Map
            {
                if matches!(role, Role::DirectItem)
                    && let Some(id) = self.find(job.node, "sdfRef")?
                    && !self.model.nodes[id].deleted
                {
                    self.model.nodes[job.node].needed_ref = Some(self.model.nodes[id].source);
                    self.model.nodes[job.node].unavailable = Some(Failure::meaning(
                        "direct-item reference import meaning unresolved",
                    ));
                    return Ok(());
                }
                let context_data = EffectiveJson {
                    model: &self.model,
                    node: job.node,
                }
                .context_data();
                match self.expand(job.node, active, context_data) {
                    Ok(next) => active = next,
                    Err(reason) => return self.expansion_failed(job.node, reason),
                }
            }
        }
        self.model.nodes[job.node].changed_context = active.is_some();
        if let Position::Vocabulary(role) = job.position {
            self.model.nodes[job.node].role = Some(role);
        }
        if let Position::Vocabulary(role @ (Role::Definition(_) | Role::Data)) = job.position
            && !self.sample
        {
            self.required_identity(job.node, role)?;
        }
        let object = self
            .find(job.node, "type")
            .ok()
            .flatten()
            .is_some_and(|id| {
                matches!(self.model.nodes[id].source.text(), Field::Present("object"))
            });
        let mut child = self.model.nodes[job.node].first;
        while let Some(id) = child {
            let n = &self.model.nodes[id];
            child = n.next;
            if n.deleted {
                continue;
            }
            if self.sample
                && matches!(job.position, Position::Vocabulary(role) if role.full_data() || matches!(role, Role::DirectItem))
                && !n.key.is_some_and(crate::sample::relevant_quality)
            {
                self.model.nodes[id].sample_skipped = true;
                self.model.nodes[id].unavailable =
                    Some(Failure::meaning("outside requested sample context"));
                continue;
            }
            let position = match n.key {
                Some(key) => job.position.child(key, object),
                None if matches!(job.position, Position::Named(_)) => Position::Opaque,
                None => Position::Opaque,
            };
            push(
                jobs,
                Job {
                    node: id,
                    position,
                    active,
                },
                &mut self.allocation,
            )?;
        }
        Ok(())
    }
    fn select_composed(
        &mut self,
        pointer: &str,
    ) -> Result<(Option<usize>, Position, Option<usize>), Failure> {
        let Some(mut node) = self.model.root else {
            return Ok((None, Position::Opaque, None));
        };
        let mut position = Position::Vocabulary(Role::Document);
        let mut active = None;
        for token in pointer
            .strip_prefix('/')
            .into_iter()
            .flat_map(|tail| tail.split('/'))
        {
            if let Position::Vocabulary(role) = position {
                self.model.nodes[node].role = Some(role);
                if role.reference_context() && self.model.nodes[node].source.kind() == JsonKind::Map
                {
                    if matches!(role, Role::DirectItem)
                        && let Some(id) = self.find(node, "sdfRef")?
                        && !self.model.nodes[id].deleted
                    {
                        self.model.nodes[node].needed_ref = Some(self.model.nodes[id].source);
                        self.model.nodes[node].unavailable = Some(Failure::meaning(
                            "direct-item reference import meaning unresolved",
                        ));
                        return Ok((Some(node), position, active));
                    }
                    let context_data = EffectiveJson {
                        model: &self.model,
                        node,
                    }
                    .context_data();
                    match self.expand(node, active, context_data) {
                        Ok(next) => active = next,
                        Err(reason) => {
                            self.expansion_failed(node, reason)?;
                            return Ok((Some(node), position, active));
                        }
                    }
                }
                self.model.nodes[node].role = Some(role);
                self.model.nodes[node].changed_context = active.is_some();
            }
            if !charge(&mut self.work, token.len() * 4 + 1)
                || !charge(&mut self.allocation, token.len() * 2)
            {
                self.model.nodes[node].unavailable = Some(Failure::limit(
                    "effective selection work or allocation limit",
                ));
                return Ok((Some(node), position, active));
            }
            let name = token.replace("~1", "/").replace("~0", "~");
            let object = if self.model.nodes[node].source.kind() == JsonKind::Map {
                self.find(node, "type")?.is_some_and(|id| {
                    matches!(self.model.nodes[id].source.text(), Field::Present("object"))
                })
            } else {
                false
            };
            position = position.child(&name, object);
            node = if self.model.nodes[node].source.kind() == JsonKind::Array {
                position = Position::Opaque;
                let index =
                    array_index(&name).ok_or(Failure::selector("unknown inspection selector"))?;
                match self.indexed_child(node, index, "effective selection work limit") {
                    Ok(found) => found.ok_or(Failure::selector("unknown inspection selector"))?,
                    Err(reason) => {
                        self.model.nodes[node].unavailable = Some(reason);
                        return Ok((Some(node), position, active));
                    }
                }
            } else {
                match self.find(node, &name)? {
                    Some(id) if !self.model.nodes[id].deleted => id,
                    _ => return Err(Failure::selector("unknown inspection selector")),
                }
            };
        }
        Ok((Some(node), position, active))
    }

    /// Relocates a copied reference's target through the copy boundaries it was
    /// copied through, from the innermost outwards. At each boundary, a target
    /// that is the copied definition or lies beneath it by pointer tokens moves to
    /// the corresponding position in the importing definition. The final position
    /// is selected in the resolved model and returned with its document and
    /// pointer. A target that no boundary relocates stays as it is, with no
    /// position. Follows ADR 0036: Copied reference relocation.
    fn relocate(
        &mut self,
        target: Selected<'a>,
        copied_through: Option<usize>,
    ) -> Result<(Selected<'a>, Option<(SourceId, String)>), Failure> {
        let Some((document, pointer)) = self.relocated_position(target.target, copied_through)?
        else {
            return Ok((target, None));
        };
        self.model.relocation_interpretation = true;
        let root = self
            .model
            .catalogue
            .source(document)
            .and_then(|source| source.json().present())
            .ok_or(Failure::reference("reference target unavailable"))?;
        // A relocated target that a patch deleted is missing, with no fallback.
        let relocated = self
            .select_resolved(root, &pointer)?
            .ok_or(Failure::reference("reference target unavailable"))?;
        Ok((relocated, Some((document, pointer))))
    }
    /// The document and pointer that a copied reference's target relocates to,
    /// if any boundary relocates it.
    fn relocated_position(
        &mut self,
        target: Element<'a>,
        copied_through: Option<usize>,
    ) -> Result<Option<(SourceId, String)>, Failure> {
        let Some(mut id) = copied_through else {
            return Ok(None);
        };
        let (mut document, mut pointer) = self.coordinate(target)?;
        let mut relocated = false;
        loop {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("reference relocation work limit"));
            }
            let b = self.boundaries[id];
            let (copied_document, copied) = self.coordinate(b.target)?;
            if !charge(&mut self.work, copied.len() + 1) {
                return Err(Failure::limit("reference relocation work limit"));
            }
            let within = pointer
                .strip_prefix(copied.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'));
            if copied_document == document && within {
                let (importer_document, mut importer) = self.node_coordinate(b.importer)?;
                let rest = &pointer[copied.len()..];
                if !charge(&mut self.allocation, rest.len()) {
                    return Err(Failure::allocation("reference relocation allocation limit"));
                }
                importer.push_str(rest);
                (document, pointer) = (importer_document, importer);
                relocated = true;
            }
            match b.parent {
                Some(parent) => id = parent,
                None => break,
            }
        }
        Ok(relocated.then_some((document, pointer)))
    }
    /// The document and pointer of an element.
    fn coordinate(&mut self, element: Element<'a>) -> Result<(SourceId, String), Failure> {
        match element {
            Element::Composed(node) => self.node_coordinate(node),
            Element::Written(json) => self.written_coordinate(json),
        }
    }
    /// The document and pointer of an arena node in its document's resolved
    /// model. The requested document's composition starts at its root, and a
    /// path composition at the position of the definition it composes.
    fn node_coordinate(&mut self, node: usize) -> Result<(SourceId, String), Failure> {
        let mut path = Vec::new();
        let mut current = node;
        let (document, mut pointer) = loop {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("reference relocation work limit"));
            }
            if let Some(parent) = self.model.nodes[current].parent {
                push(&mut path, current, &mut self.allocation)?;
                current = parent;
                continue;
            }
            if self.model.root == Some(current) {
                break (self.model.source, String::new());
            }
            let mut origin = None;
            for composition in &self.paths {
                if !charge(&mut self.work, 1) {
                    return Err(Failure::limit("reference relocation work limit"));
                }
                if composition.node == current {
                    origin = Some(composition.origin);
                }
            }
            match origin {
                Some(Element::Written(json)) => break self.written_coordinate(json)?,
                // The detached copy stands at the composed element's position.
                Some(Element::Composed(composed)) => current = composed,
                None => {
                    return Err(Failure::meaning("copied reference relocation unavailable"));
                }
            }
        };
        for id in path.into_iter().rev() {
            let n = &self.model.nodes[id];
            let (key, index) = (n.key, n.index);
            self.push_token(&mut pointer, key, index)?;
        }
        Ok((document, pointer))
    }
    /// The document and pointer of a written element.
    fn written_coordinate(&mut self, json: JsonView<'a>) -> Result<(SourceId, String), Failure> {
        let mut path = Vec::new();
        let mut current = json;
        while let Some(parent) = current.n().parent {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("reference relocation work limit"));
            }
            push(&mut path, current, &mut self.allocation)?;
            current.node = parent;
        }
        let mut pointer = String::new();
        for json in path.into_iter().rev() {
            let n = json.n();
            self.push_token(&mut pointer, n.key.as_deref(), n.index)?;
        }
        Ok((json.source_id(), pointer))
    }
    /// Appends one escaped JSON Pointer token.
    fn push_token(
        &mut self,
        pointer: &mut String,
        key: Option<&str>,
        index: Option<usize>,
    ) -> Result<(), Failure> {
        let size = key.map_or(20, |key| key.len() * 2) + 1;
        if !charge(&mut self.work, size) || !charge(&mut self.allocation, size) {
            return Err(Failure::limit("reference relocation work limit"));
        }
        pointer.push('/');
        match (key, index) {
            (Some(key), _) => {
                for ch in key.chars() {
                    match ch {
                        '~' => pointer.push_str("~0"),
                        '/' => pointer.push_str("~1"),
                        ch => pointer.push(ch),
                    }
                }
            }
            (None, Some(index)) => pointer.push_str(&index.to_string()),
            (None, None) => return Err(Failure::reference("map member identity unavailable")),
        }
        Ok(())
    }
    fn ancestor(&mut self, mut child: JsonView<'a>, parent: JsonView<'a>) -> Result<bool, Failure> {
        if child.source_id() != parent.source_id() {
            return Ok(false);
        }
        loop {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("element containment work limit"));
            }
            if child.node == parent.node {
                return Ok(true);
            }
            let Some(id) = child.n().parent else {
                return Ok(false);
            };
            child.node = id;
        }
    }

    fn finish_capabilities(&mut self) -> Result<(), Failure> {
        let Some(root) = self.model.selected else {
            return Ok(());
        };
        self.model.complete_apart_from_required = self.finish_root_capabilities(root)?;
        Ok(())
    }
    /// Marks capability completeness below `root`. Also returns whether the root is
    /// complete apart from `sdfRequired` qualities, which never hold data descriptions.
    fn finish_root_capabilities(&mut self, root: usize) -> Result<bool, Failure> {
        let mut frames = Vec::new();
        let mut root_apart = false;
        push(
            &mut frames,
            (root, self.model.nodes[root].first, true, true),
            &mut self.allocation,
        )?;
        while let Some((id, next, children_complete, children_apart)) = frames.last_mut() {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("effective capability work limit"));
            }
            if let Some(child) = *next {
                *next = self.model.nodes[child].next;
                if self.model.nodes[child].deleted {
                    continue;
                }
                let next = if self.model.nodes[child].unavailable.is_some() {
                    None
                } else {
                    self.model.nodes[child].first
                };
                push(&mut frames, (child, next, true, true), &mut self.allocation)?;
            } else {
                let n = &self.model.nodes[*id];
                let sample_description = n.role.is_some_and(Role::data_description_role)
                    || n.parent.is_some_and(|parent| {
                        let p = &self.model.nodes[parent];
                        n.key == Some("items") && p.role.is_some_and(Role::data_description_role)
                            || matches!(p.key, Some("properties" | "sdfChoice"))
                                && p.parent.is_some_and(|owner| {
                                    self.model.nodes[owner]
                                        .role
                                        .is_some_and(Role::data_description_role)
                                })
                    });
                let own = n.unavailable.is_none()
                    && n.meaning_unavailable.is_none()
                    && (n.source.n().capable
                        || matches!(n.source.kind(), JsonKind::Map | JsonKind::Array))
                    && (n.parent.is_none() || n.key.is_some() || n.index.is_some())
                    && (!self.sample
                        || !sample_description
                        || n.sample_stage == SampleStage::Completed);
                let complete = *children_complete && own;
                let apart = *children_apart && own || self.model.required_quality(*id);
                let id = *id;
                frames.pop();
                self.model.nodes[id].complete = complete;
                if let Some((_, _, parent_complete, parent_apart)) = frames.last_mut() {
                    *parent_complete &= complete;
                    *parent_apart &= apart;
                } else {
                    root_apart = apart;
                }
            }
        }
        Ok(root_apart)
    }

    fn required_identity(&mut self, parent: usize, role: Role) -> Result<(), Failure> {
        let Some(node) = self.find(parent, "sdfRequired")? else {
            return Ok(());
        };
        if self.model.nodes[node].deleted {
            return Ok(());
        }
        let copied = self.model.nodes[node].context.is_some();
        let mut child = self.model.nodes[node].first;
        while let Some(id) = child {
            let n = &self.model.nodes[id];
            child = n.next;
            let Field::Present(text) = n.source.text() else {
                continue;
            };
            if !charge(&mut self.work, text.len() + 1) {
                return Err(Failure::limit("required identity work limit"));
            }
            let reference = n.source;
            let curie = !text.starts_with('#');
            if curie {
                if !text.contains(':') {
                    continue;
                }
                // Copied or not, a CURIE is assembled with the namespace mappings of
                // the document that wrote it, following ADR 0036: Copied reference
                // relocation.
                if let Err(reason) = self.assemble_global(reference, text, copied) {
                    if reason.is_limit() {
                        self.model.nodes[parent].needed_ref = Some(reference);
                        return Err(reason);
                    }
                    let affected = if self.model.nodes[node].meaning_unavailable.is_some() {
                        id
                    } else {
                        node
                    };
                    let reason = self.count_definite(Some(reference), affected, reason)?;
                    self.model.nodes[affected].meaning_unavailable = Some(reason);
                    self.model.nodes[affected].needed_ref = Some(reference);
                    continue;
                }
            }
            // A copied pointer in a definition relocates by coordinate when required
            // declaration inspection resolves it, following ADR 0036: Copied
            // reference relocation. One in a full data description keeps the
            // separate data-only limitation of ADR 0023: Supplied global
            // composition and dependency scope.
            if !copied || matches!(role, Role::Definition(_)) {
                continue;
            }
            let target = match self.pointer_source(reference, copied) {
                Ok(selected) => Some(selected.written),
                Err(reason) if reason.is_limit() => {
                    self.model.nodes[parent].needed_ref = Some(reference);
                    return Err(reason);
                }
                // As for an authored CURIE, required declaration inspection reports a
                // global target that is missing, ambiguous or unavailable.
                Err(_) if curie => {
                    self.pending_conflicts.clear();
                    continue;
                }
                Err(_) => None,
            };
            let mut copied = target.is_none();
            let mut boundary = self.model.nodes[node].context;
            while let Some(id) = boundary {
                if !charge(&mut self.work, 1) {
                    return Err(Failure::limit("required identity work limit"));
                }
                let b = self.boundaries[id];
                if let Some(target) = target {
                    copied |= self.ancestor(target, b.written)?;
                }
                boundary = b.parent;
            }
            if copied {
                let affected = if self.model.nodes[node].meaning_unavailable.is_some() {
                    id
                } else {
                    node
                };
                self.model.nodes[affected].meaning_unavailable = Some(Failure::meaning(
                    "data-only required reference meaning unresolved",
                ));
                self.model.nodes[affected].needed_ref = Some(reference);
                continue;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub struct EffectiveLocation<'a> {
    json: EffectiveJson<'a>,
    trailing: Option<&'static str>,
}
impl EffectiveLocation<'_> {
    pub fn root_document(self) -> SourceId {
        self.json.root_document()
    }
    pub fn pointer(self, limit: u32) -> Field<String> {
        if limit == 0 || limit > 8192 {
            return Field::Unavailable("unsupported pointer output budget");
        }
        let mut work =
            crate::json::PointerBudget::new(self.json.model.catalogue.limits.pointer_total);
        let size = match self.pointer_size(limit as usize, &mut work) {
            Ok(size) => size,
            Err(_) => return Field::Unavailable("effective pointer unavailable or limited"),
        };
        if work.debit(size.render_work()).is_err() {
            return Field::Unavailable("effective pointer work limit");
        }
        if size.bytes + size.depth * std::mem::size_of::<usize>()
            > self.json.model.derived_remaining
        {
            return Field::Unavailable("effective pointer allocation limit");
        }
        Field::Present(self.render_pointer(size))
    }
    pub(crate) fn pointer_size(
        self,
        cap: usize,
        work: &mut crate::json::PointerBudget,
    ) -> Result<EffectivePointerSize, crate::json::PointerError> {
        let mut bytes = self.trailing.map_or(0, |key| key.len() + 1);
        let mut depth = 0;
        let mut current = self.json.node;
        while let Some(parent) = self.json.model.nodes[current].parent {
            work.debit(1)?;
            let node = &self.json.model.nodes[current];
            let token = match (node.key, node.index) {
                (Some(key), _) => {
                    let remaining = cap
                        .checked_sub(bytes + 1)
                        .ok_or(crate::json::PointerError::Limit)?;
                    if key.len() > remaining {
                        return Err(crate::json::PointerError::Limit);
                    }
                    work.debit(key.len())?;
                    let mut token = key.len();
                    for byte in key.bytes() {
                        if matches!(byte, b'~' | b'/') {
                            token = token
                                .checked_add(1)
                                .filter(|n| *n <= remaining)
                                .ok_or(crate::json::PointerError::Limit)?;
                        }
                    }
                    token
                }
                (None, Some(index)) => {
                    if index == 0 {
                        1
                    } else {
                        index.ilog10() as usize + 1
                    }
                }
                _ => return Err(crate::json::PointerError::NotAvailable),
            };
            bytes = bytes
                .checked_add(token + 1)
                .filter(|bytes| *bytes <= cap)
                .ok_or(crate::json::PointerError::Limit)?;
            depth += 1;
            current = parent;
        }
        if bytes > cap {
            return Err(crate::json::PointerError::Limit);
        }
        Ok(EffectivePointerSize { bytes, depth })
    }
    pub(crate) fn render_pointer(self, size: EffectivePointerSize) -> String {
        let mut ancestry = Vec::with_capacity(size.depth);
        let mut current = self.json.node;
        while let Some(parent) = self.json.model.nodes[current].parent {
            ancestry.push(current);
            current = parent;
        }
        let mut pointer = String::with_capacity(size.bytes);
        for id in ancestry.into_iter().rev() {
            let node = &self.json.model.nodes[id];
            pointer.push('/');
            if let Some(key) = node.key {
                for ch in key.chars() {
                    match ch {
                        '~' => pointer.push_str("~0"),
                        '/' => pointer.push_str("~1"),
                        other => pointer.push(other),
                    }
                }
            } else {
                use std::fmt::Write;
                write!(pointer, "{}", node.index.expect("array index")).expect("String write");
            }
        }
        if let Some(key) = self.trailing {
            pointer.push('/');
            pointer.push_str(key);
        }
        pointer
    }
}
pub(crate) struct EffectivePointerSize {
    pub(crate) bytes: usize,
    depth: usize,
}
impl EffectivePointerSize {
    pub(crate) fn storage_bytes(&self) -> Option<usize> {
        self.depth
            .checked_mul(std::mem::size_of::<usize>())?
            .checked_add(self.bytes)
    }
    pub(crate) fn render_work(&self) -> usize {
        2 * self.depth + 1 + 2 * self.bytes
    }
}
#[derive(Clone, Copy)]
pub struct Explanation<'a> {
    json: EffectiveJson<'a>,
    default: bool,
    trailing: Option<&'static str>,
}
impl<'a> Explanation<'a> {
    pub(crate) fn selected(json: EffectiveJson<'a>) -> Self {
        Self {
            json,
            default: false,
            trailing: None,
        }
    }
    /// Original reference establishing the applicable importing boundary.
    pub fn consumer(self) -> Option<JsonView<'a>> {
        self.json.n().consumer
    }
    pub fn kind(self) -> OriginKind {
        if self.default {
            OriginKind::Default
        } else {
            self.json.n().kind
        }
    }
    pub fn effective(self) -> EffectiveLocation<'a> {
        EffectiveLocation {
            json: self.json,
            trailing: self.trailing,
        }
    }
    pub fn sources(self) -> impl Iterator<Item = JsonView<'a>> {
        let node = self.json.n();
        let values = if self.default {
            [None, None]
        } else {
            [Some(node.source), node.previous]
        };
        values.into_iter().flatten()
    }
}
impl OriginKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Inherited => "inherited",
            Self::Override => "override",
            Self::Deleted => "deleted",
            Self::Default => "default",
        }
    }
}
impl<'a> EffectiveJson<'a> {
    pub fn location(self) -> EffectiveLocation<'a> {
        EffectiveLocation {
            json: self,
            trailing: None,
        }
    }
    pub fn pointer(self, limit: u32) -> Field<String> {
        self.location().pointer(limit)
    }
}
impl EffectiveModel<'_> {
    pub(crate) fn explanation_node(&self, pointer: &str) -> Field<usize> {
        if crate::json::validate_pointer(pointer).is_err() {
            return Field::Unavailable("invalid or oversized effective pointer");
        }
        let Some(mut current) = self.root else {
            return Field::Unavailable("effective root unavailable");
        };
        let mut work = self.catalogue.limits.work;
        let mut allocation = self.derived_remaining;
        for token in pointer
            .strip_prefix('/')
            .into_iter()
            .flat_map(|tail| tail.split('/'))
        {
            if let Some(reason) = self.nodes[current].unavailable {
                return Field::Unavailable(reason.reason);
            }
            if !charge(&mut work, token.len() * 4 + 1) || !charge(&mut allocation, token.len() * 2)
            {
                return Field::Unavailable("effective explanation work or allocation limit");
            }
            let name = token.replace("~1", "/").replace("~0", "~");
            let mut child = self.nodes[current].first;
            let mut found = None;
            let mut unknown = false;
            while let Some(id) = child {
                let node = &self.nodes[id];
                if !charge(&mut work, name.len() + node.key.map_or(0, str::len) + 1) {
                    return Field::Unavailable("effective explanation work limit");
                }
                match (node.key, node.index) {
                    (Some(key), _) if key == name => found = Some(id),
                    (None, Some(index))
                        if !name.is_empty()
                            && !(name.len() > 1 && name.starts_with('0'))
                            && name.parse::<usize>() == Ok(index) =>
                    {
                        found = Some(id)
                    }
                    (None, None) => unknown = true,
                    _ => {}
                }
                child = node.next;
            }
            if unknown {
                return Field::Unavailable("map member identity unavailable");
            }
            current = match found {
                Some(id) => id,
                None => return Field::Absent,
            };
        }
        if let Some(reason) = self.nodes[current].unavailable {
            Field::Unavailable(reason.reason)
        } else {
            Field::Present(current)
        }
    }
    pub fn explanation(&self, pointer: &str) -> Field<Explanation<'_>> {
        match self.explanation_node(pointer) {
            Field::Present(node) => Field::Present(Explanation {
                json: EffectiveJson { model: self, node },
                default: false,
                trailing: None,
            }),
            Field::Absent => Field::Absent,
            Field::Unavailable(reason) => Field::Unavailable(reason),
        }
    }
    pub fn default_explanation(&self, pointer: &str) -> Field<Explanation<'_>> {
        let Some((parent, key)) = pointer.rsplit_once('/') else {
            return Field::Absent;
        };
        let key = match key {
            "readable" => "readable",
            "writable" => "writable",
            "observable" => "observable",
            "nullable" => "nullable",
            _ => return Field::Absent,
        };
        let parent = match self.explanation_node(parent) {
            Field::Present(node) => node,
            Field::Absent => return Field::Absent,
            Field::Unavailable(reason) => return Field::Unavailable(reason),
        };
        let role = self.nodes[parent].role;
        if key == "nullable"
            && (EffectiveJson {
                model: self,
                node: parent,
            })
            .context_data()
        {
            return match (EffectiveJson {
                model: self,
                node: parent,
            })
            .get(key)
            {
                Field::Present(_) => Field::Absent,
                _ => Field::Unavailable(crate::context::DEFAULT_UNAVAILABLE),
            };
        }
        let supported = match key {
            "readable" | "writable" | "observable" => {
                matches!(role, Some(Role::Definition(Class::Property)))
            }
            "nullable" => role.is_some_and(Role::full_data),
            _ => false,
        };
        if !supported {
            return if matches!(role, Some(Role::DirectItem)) && key == "nullable" {
                Field::Unavailable("direct-item nullable default meaning unresolved")
            } else {
                Field::Absent
            };
        }
        match self.explanation_node(pointer) {
            Field::Present(node) if !self.nodes[node].deleted => Field::Absent,
            Field::Present(node) => Field::Present(Explanation {
                json: EffectiveJson { model: self, node },
                default: true,
                trailing: None,
            }),
            Field::Absent => Field::Present(Explanation {
                json: EffectiveJson {
                    model: self,
                    node: parent,
                },
                default: true,
                trailing: Some(key),
            }),
            Field::Unavailable(reason) => Field::Unavailable(reason),
        }
    }
}
pub(crate) struct ExplanationRows<'a> {
    model: &'a EffectiveModel<'a>,
    current: Option<usize>,
    phase: u8,
    pub(crate) work: usize,
    pub(crate) limited: bool,
    root: usize,
}
impl<'a> Iterator for ExplanationRows<'a> {
    type Item = Explanation<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.current?;
            if !charge(&mut self.work, 1) {
                self.limited = true;
                self.current = None;
                return None;
            }
            if self.phase == 0 {
                self.phase = 1;
                if self.model.nodes[node].unavailable.is_none() {
                    return Some(Explanation {
                        json: EffectiveJson {
                            model: self.model,
                            node,
                        },
                        default: false,
                        trailing: None,
                    });
                }
            }
            if self.phase <= 4 {
                let key = match self.phase {
                    1 => "readable",
                    2 => "writable",
                    3 => "observable",
                    _ => "nullable",
                };
                self.phase += 1;
                match self.model.default_at(node, key, &mut self.work) {
                    Field::Present(value) => return Some(value),
                    Field::Unavailable("effective explanation work limit") => {
                        self.limited = true;
                        self.current = None;
                        return None;
                    }
                    _ => {}
                }
                continue;
            }
            self.phase = 0;
            let n = &self.model.nodes[node];
            if !n.deleted
                && n.unavailable.is_none()
                && let Some(first) = n.first
            {
                self.current = Some(first);
                continue;
            }
            let mut current = node;
            loop {
                if !charge(&mut self.work, 1) {
                    self.limited = true;
                    self.current = None;
                    return None;
                }
                if current == self.root {
                    self.current = None;
                    break;
                }
                let n = &self.model.nodes[current];
                if let Some(next) = n.next {
                    self.current = Some(next);
                    break;
                }
                current = n.parent.expect("effective ancestry");
            }
        }
    }
}
impl EffectiveModel<'_> {
    pub(crate) fn explanations(&self) -> ExplanationRows<'_> {
        ExplanationRows {
            model: self,
            current: self.selected,
            phase: 0,
            work: self.catalogue.limits.work,
            limited: false,
            root: self.selected.unwrap_or(0),
        }
    }
    pub(crate) fn explanations_from(&self, root: usize, work: usize) -> ExplanationRows<'_> {
        ExplanationRows {
            model: self,
            current: Some(root),
            phase: 0,
            work,
            limited: false,
            root,
        }
    }
    pub(crate) fn default_at<'a>(
        &'a self,
        parent: usize,
        key: &'static str,
        work: &mut usize,
    ) -> Field<Explanation<'a>> {
        if let Some(reason) = self.nodes[parent].unavailable {
            return Field::Unavailable(reason.reason);
        }
        let role = self.nodes[parent].role;
        if key == "nullable"
            && (EffectiveJson {
                model: self,
                node: parent,
            })
            .context_data()
        {
            return match (EffectiveJson {
                model: self,
                node: parent,
            })
            .get(key)
            {
                Field::Present(_) => Field::Absent,
                _ => Field::Unavailable(crate::context::DEFAULT_UNAVAILABLE),
            };
        }
        let supported = match key {
            "readable" | "writable" | "observable" => {
                matches!(role, Some(Role::Definition(Class::Property)))
            }
            "nullable" => role.is_some_and(Role::full_data),
            _ => false,
        };
        if !supported {
            return if matches!(role, Some(Role::DirectItem)) && key == "nullable" {
                Field::Unavailable("direct-item nullable default meaning unresolved")
            } else {
                Field::Absent
            };
        }
        let mut child = self.nodes[parent].first;
        let mut found = None;
        while let Some(id) = child {
            let n = &self.nodes[id];
            if !charge(work, n.key.map_or(0, str::len) + key.len() + 1) {
                return Field::Unavailable("effective explanation work limit");
            }
            match n.key {
                Some(name) if name == key => found = Some(id),
                None => return Field::Unavailable("map member identity unavailable"),
                _ => {}
            }
            child = n.next;
        }
        match found {
            Some(node) if !self.nodes[node].deleted => Field::Absent,
            Some(node) => Field::Present(Explanation {
                json: EffectiveJson { model: self, node },
                default: true,
                trailing: None,
            }),
            None => Field::Present(Explanation {
                json: EffectiveJson {
                    model: self,
                    node: parent,
                },
                default: true,
                trailing: Some(key),
            }),
        }
    }
}

pub(crate) struct EffectiveConditions<'a> {
    model: &'a EffectiveModel<'a>,
    next: Option<usize>,
    work: usize,
}
impl<'a> Iterator for EffectiveConditions<'a> {
    type Item = (EffectiveJson<'a>, Failure);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(id) = self.next {
            if !charge(&mut self.work, 1) {
                self.next = None;
                return None;
            }
            let n = &self.model.nodes[id];
            if !n.deleted
                && n.unavailable.is_none()
                && let Some(first) = n.first
            {
                self.next = Some(first);
            } else {
                let mut current = id;
                loop {
                    if !charge(&mut self.work, 1) {
                        self.next = None;
                        return None;
                    }
                    if Some(current) == self.model.selected {
                        self.next = None;
                        break;
                    }
                    let n = &self.model.nodes[current];
                    if let Some(next) = n.next {
                        self.next = Some(next);
                        break;
                    }
                    current = n.parent.expect("effective ancestry");
                }
            }
            if !n.deleted
                && !n.sample_skipped
                && let Some(reason) = n.unavailable.or(n.meaning_unavailable)
                && reason.reported()
            {
                return Some((
                    EffectiveJson {
                        model: self.model,
                        node: id,
                    },
                    reason,
                ));
            }
        }
        None
    }
}
impl EffectiveModel<'_> {
    pub(crate) fn conditions(&self) -> EffectiveConditions<'_> {
        EffectiveConditions {
            model: self,
            next: self.selected,
            work: self.catalogue.limits.work,
        }
    }
    /// Leaves each definite reference failure that an `earlier` model already
    /// reports to that model, so a check over several documents reports each
    /// authored reference problem once. Failures it lacks the work to compare
    /// stay reported.
    pub(crate) fn leave_reported_references(
        &mut self,
        earlier: &[EffectiveModel<'_>],
        work: &mut usize,
    ) {
        for &(source, json, node) in &self.definite_references {
            if !charge(work, earlier.len()) {
                return;
            }
            let reported = earlier.iter().any(|model| {
                model
                    .definite_references
                    .binary_search_by_key(&(source, json), |&(source, json, _)| (source, json))
                    .is_ok()
            });
            if reported {
                // The condition this node reports is the one to leave out.
                let n = &mut self.nodes[node];
                if let Some(failure) = n.unavailable.as_mut().or(n.meaning_unavailable.as_mut())
                    && failure.definite()
                {
                    failure.repeat = true;
                }
                self.reference_errors -= 1;
            }
        }
    }
}

impl<'a> EffectiveJson<'a> {
    pub(crate) fn requiredness(self) -> Field<Self> {
        match self.get("sdfRequired") {
            Field::Present(json) if json.n().meaning_unavailable.is_some() => {
                Field::Unavailable(json.n().meaning_unavailable.unwrap().reason)
            }
            value => value,
        }
    }
}

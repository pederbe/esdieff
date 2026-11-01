//! Revision comparison classifies the differences between two Model revisions from the
//! selected perspective. Follows ADR 0053: Implementer revision comparison,
//! ADR 0039: Client-side revision comparison, ADR 0009: Bounded
//! processing and reporting, ADR 0013: Shared workflow and report guarantees, and
//! ADR 0020: Owned effective views and nearest origins.
//!
//! The operation composes every model document of both Catalogues through the existing
//! effective view on one operation allowance, pairs Corresponding definitions in `walk`,
//! and classifies each difference in `classify`. A change to admitted values is breaking
//! only with a Breaking witness and compatible only with a Compatibility proof, which
//! `witness` settles. Supplied Difference acknowledgements are matched afterwards in
//! `acknowledgement`, following ADR 0040: Acknowledged revision differences. A breaking
//! document pair whose `info` `version` does not compare higher earns a warning in `version`,
//! following ADR 0044: Versions of breaking revisions.
//! Records keep arena indices into the owned effective models, so typed views and both
//! reports borrow the same result.
use crate::allowance::{OperationBudget, admit};
use crate::sample::ValueAllowance;
use crate::{Catalogue, Class, Disposition, EffectiveJson, EffectiveModel, Field, SourceId};

mod acknowledgement;
mod changelog;
mod classify;
mod read;
mod report;
mod requiredness;
pub(crate) mod version;
mod walk;
mod witness;
pub(crate) mod wording;
pub use acknowledgement::{DifferenceAcknowledgement, DifferenceAcknowledgements};
pub use version::VersionWarning;
pub use witness::Witness;

/// The identity of the comparison rules, which binds the classified differences. A
/// release that classifies more differences gets a new identity.
pub const COMPARISON_CONTRACT: &str = "esdieff.revision-comparison/8";

pub(crate) const SCOPE_NOTICE: &str =
    "Client interactions only. Context metadata compatibility is not assessed.";

/// Whose existing behavior must remain valid when adopting the later Model revision.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum Perspective {
    /// Clients built for the earlier model can still interact with the later model.
    #[default]
    Client,
    /// Implementations satisfying the earlier model still satisfy the later model.
    Implementer,
}
impl Perspective {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Implementer => "implementer",
        }
    }
    pub(crate) fn scope_notice(self) -> &'static str {
        match self {
            Self::Client => SCOPE_NOTICE,
            Self::Implementer => {
                "Existing implementation obligations only. Context metadata compatibility is not assessed."
            }
        }
    }
    /// Value-set obligations are dual to client obligations. Keep chronology at the
    /// public boundary while reusing the same proof and witness machinery internally.
    fn ordered<T>(self, mut pair: [T; 2]) -> [T; 2] {
        if self == Self::Implementer {
            pair.swap(0, 1);
        }
        pair
    }
}

/// One of the two Model revisions under comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Revision {
    Earlier,
    Later,
}
impl Revision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Earlier => "earlier",
            Self::Later => "later",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Earlier => 0,
            Self::Later => 1,
        }
    }
    fn other(self) -> Self {
        match self {
            Self::Earlier => Self::Later,
            Self::Later => Self::Earlier,
        }
    }
}

/// How a difference, or the comparison as a whole, affects the selected perspective.
/// Compatibility for consumers of Context metadata is not assessed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Classification {
    Breaking,
    Compatible,
    Undetermined,
}
impl Classification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Breaking => "breaking",
            Self::Compatible => "compatible",
            Self::Undetermined => "undetermined",
        }
    }
    /// The `check` disposition: compatible passes and breaking fails.
    pub fn disposition(self) -> Disposition {
        match self {
            Self::Breaking => Disposition::Failed,
            Self::Compatible => Disposition::Passed,
            Self::Undetermined => Disposition::Undetermined,
        }
    }
    /// Report order: breaking first, then undetermined, then compatible.
    fn rank(self) -> usize {
        match self {
            Self::Breaking => 0,
            Self::Undetermined => 1,
            Self::Compatible => 2,
        }
    }
}

/// A classification, with the reason a difference has exactly when it is undetermined.
#[derive(Clone, Copy, PartialEq)]
enum Verdict {
    Breaking,
    Compatible,
    Undetermined(UndeterminedReason),
}
impl Verdict {
    const NOT_CLASSIFIED: Self = Self::Undetermined(UndeterminedReason::NotClassified);
    fn classification(self) -> Classification {
        match self {
            Self::Breaking => Classification::Breaking,
            Self::Compatible => Classification::Compatible,
            Self::Undetermined(_) => Classification::Undetermined,
        }
    }
    fn reason(self) -> Option<UndeterminedReason> {
        match self {
            Self::Undetermined(reason) => Some(reason),
            _ => None,
        }
    }
}

/// What differs between the corresponding content of the two revisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DifferenceKind {
    /// Present only in the earlier revision.
    Removed,
    /// Present only in the later revision.
    Added,
    /// A quality whose effective value differs, including one present in only one revision.
    Changed,
    /// Content that an effective view could not establish, so whether it changed is unknown.
    Unavailable,
}
impl DifferenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Unavailable => "unavailable",
        }
    }
}

/// The content a difference is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DifferenceSubject<'a> {
    /// A whole document.
    Document,
    /// A grouping, affordance or Context of this class.
    Definition(Class),
    /// A named entry of map data's `properties`.
    Entry,
    /// A quality of a grouping, affordance or data description.
    Quality(&'a str),
}

/// A direction of interaction data between Client and Thing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Direction {
    /// Written Property values and Action input.
    ClientSupplied,
    /// Read or observed Property values, Action output and Event output.
    ThingSupplied,
}
impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClientSupplied => "client_supplied",
            Self::ThingSupplied => "thing_supplied",
        }
    }
    fn bit(self) -> u8 {
        match self {
            Self::ClientSupplied => CLIENT,
            Self::ThingSupplied => THING,
        }
    }
    /// The revision that admits a Breaking witness in this direction.
    fn admitting(self) -> Revision {
        match self {
            Self::ClientSupplied => Revision::Earlier,
            Self::ThingSupplied => Revision::Later,
        }
    }
}
const CLIENT: u8 = 1;
const THING: u8 = 2;
const DIRECTIONS: [Direction; 2] = [Direction::ClientSupplied, Direction::ThingSupplied];

/// The state of a difference's content in one revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Presence {
    Present,
    Absent,
    Unavailable,
}
impl Presence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Why a difference is undetermined.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum UndeterminedReason {
    /// The comparison does not classify this quality yet.
    NotClassified,
    /// Neither a Breaking witness nor a Compatibility proof was established.
    Unproven,
    /// Sample-value checking could not settle whether a witness candidate is admitted.
    WitnessUnsettled,
    /// An effective view could not establish the content, or the operation allowance ran out.
    Unavailable {
        code: &'static str,
        message: &'static str,
    },
    /// The recorded SenML evidence cannot settle a unit change.
    Unit(UnitReason),
    /// The established meanings of a `contentFormat` change cannot be compared.
    ContentFormat(ContentFormatReason),
}
impl UndeterminedReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotClassified => "comparison.not_classified",
            Self::Unproven => "comparison.unproven",
            Self::WitnessUnsettled => "comparison.witness_unsettled",
            Self::Unavailable { code, .. } => code,
            Self::Unit(reason) => reason.code(),
            Self::ContentFormat(reason) => reason.code(),
        }
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::NotClassified => "quality is not classified yet",
            Self::Unproven => "no breaking witness or compatibility proof",
            Self::WitnessUnsettled => "witness candidate admission is undetermined",
            Self::Unavailable { message, .. } => message,
            Self::Unit(reason) => reason.message(),
            Self::ContentFormat(reason) => reason.message(),
        }
    }
    fn limit(message: &'static str) -> Self {
        Self::Unavailable {
            code: "operation.limit",
            message,
        }
    }
}

/// Why a changed unit is undetermined. Only a change between two different current names
/// in the recorded SenML evidence is breaking, following ADR 0039: Client-side revision
/// comparison and ADR 0027: Unit names and recorded registry guidance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum UnitReason {
    /// Only the later revision states a unit.
    Added,
    /// Only the earlier revision states a unit.
    Removed,
    /// A URI unit name, whose unit the recorded evidence does not identify.
    UriName,
    /// A spelling without an entry in the recorded evidence.
    UnrecordedSpelling,
    /// A legacy name, which can be one of several names for one unit.
    LegacyName,
    /// The request withholds the recorded evidence that the change needs.
    EvidenceWithheld,
}
impl UnitReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::Added => "comparison.unit_added",
            Self::Removed => "comparison.unit_removed",
            Self::UriName => "comparison.unit_uri_name",
            Self::UnrecordedSpelling => "comparison.unit_unrecorded",
            Self::LegacyName => "comparison.unit_legacy",
            Self::EvidenceWithheld => "registry.caller_withheld",
        }
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::Added => "unit is added, which changes the stated meaning of values",
            Self::Removed => "unit is removed, which changes the stated meaning of values",
            Self::UriName => "a URI unit name is involved",
            Self::UnrecordedSpelling => "a unit spelling has no recorded SenML entry",
            Self::LegacyName => {
                "a legacy unit name is involved, and legacy names can give one unit several names"
            }
            Self::EvidenceWithheld => "the request withholds recorded unit evidence",
        }
    }
}

/// Why a changed `contentFormat` is undetermined. Only a change between two different
/// established meanings is breaking, following ADR 0039: Client-side revision comparison and
/// ADR 0033: Content-format model meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContentFormatReason {
    /// Only the later revision declares a content format.
    Added,
    /// Only the earlier revision declares a content format.
    Removed,
    /// A declaration has no established meaning, such as a media type outside the supported
    /// rules or an unrecorded identifier or coding.
    Unestablished,
    /// The request withholds the recorded evidence that the change needs.
    EvidenceWithheld,
}
impl ContentFormatReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::Added => "comparison.content_format_added",
            Self::Removed => "comparison.content_format_removed",
            Self::Unestablished => "comparison.content_format_unestablished",
            Self::EvidenceWithheld => "registry.caller_withheld",
        }
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::Added => "content format is added, which changes the stated meaning of values",
            Self::Removed => {
                "content format is removed, which changes the stated meaning of values"
            }
            Self::Unestablished => "a content format has no established meaning",
            Self::EvidenceWithheld => "the request withholds recorded content-format evidence",
        }
    }
}

/// One revision's side of a recorded difference.
#[derive(Clone, Copy)]
struct Side<'a> {
    root: SourceId,
    /// Index of the revision's effective model, absent when the document was not composed.
    model: Option<usize>,
    /// The content's node, or its owner when `trailing` names an absent member.
    node: Option<usize>,
    trailing: Option<&'a str>,
    presence: Presence,
    /// The node whose explanation is the nearest content origin.
    origin: Option<usize>,
}
struct Record<'a> {
    kind: DifferenceKind,
    subject: DifferenceSubject<'a>,
    directions: u8,
    verdict: Verdict,
    sides: [Option<Side<'a>>; 2],
    witness: Option<Witness>,
    /// A supplied Difference acknowledgement matches the difference.
    acknowledged: bool,
    /// The effective pointer of each side the difference has, rendered once for the entry that
    /// would acknowledge it, and absent when it cannot be acknowledged.
    pointers: Option<[Option<String>; 2]>,
}
impl Record<'_> {
    /// Report order: unacknowledged breaking, unacknowledged undetermined, acknowledged
    /// breaking, acknowledged undetermined, then compatible.
    fn order(&self) -> usize {
        match self.verdict.classification() {
            Classification::Compatible => 4,
            classification => classification.rank() + 2 * usize::from(self.acknowledged),
        }
    }
}

/// Restore chronological labels on reasons produced by the dual value comparison.
fn reverse_reason(verdict: Verdict) -> Verdict {
    use UndeterminedReason::{ContentFormat, Unit};
    match verdict {
        Verdict::Undetermined(reason) => Verdict::Undetermined(match reason {
            Unit(UnitReason::Added) => Unit(UnitReason::Removed),
            Unit(UnitReason::Removed) => Unit(UnitReason::Added),
            ContentFormat(ContentFormatReason::Added) => {
                ContentFormat(ContentFormatReason::Removed)
            }
            ContentFormat(ContentFormatReason::Removed) => {
                ContentFormat(ContentFormatReason::Added)
            }
            reason => reason,
        }),
        verdict => verdict,
    }
}

use crate::budgeted::failure;

/// Compares an earlier and a later Catalogue. Both use the fixed internal safeguards,
/// which sets the single allowance covering both revisions.
pub struct ComparisonRequest<'a> {
    perspective: Perspective,
    catalogues: [&'a Catalogue; 2],
    registry_policy: crate::RecordedRegistryPolicy,
    acknowledgements: Option<&'a DifferenceAcknowledgements>,
}
impl<'a> ComparisonRequest<'a> {
    pub fn new(earlier: &'a Catalogue, later: &'a Catalogue) -> Self {
        Self {
            perspective: Perspective::default(),
            catalogues: [earlier, later],
            registry_policy: crate::RecordedRegistryPolicy::default(),
            acknowledgements: None,
        }
    }
    /// Select whose existing behavior the later model must continue to accommodate.
    pub fn perspective(mut self, value: Perspective) -> Self {
        self.perspective = value;
        self
    }
    /// Control whether unit and content-format comparison may use the bundled dated registry
    /// evidence. As for `check`, the default uses it.
    pub fn recorded_registry_policy(mut self, value: crate::RecordedRegistryPolicy) -> Self {
        self.registry_policy = value;
        self
    }
    /// Supply Difference acknowledgements. Acknowledged differences keep their
    /// classifications, and only unacknowledged ones decide the disposition.
    pub fn acknowledgements(mut self, acknowledgements: &'a DifferenceAcknowledgements) -> Self {
        self.acknowledgements = Some(acknowledgements);
        self
    }
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn compare(self) -> Result<RevisionComparison<'a>, &'static str> {
        let [earlier, later] = self.catalogues;
        let limits = &earlier.limits;
        if *limits != later.limits {
            return Err("revisions must share one limits configuration");
        }
        if reserve(self.catalogues, self.acknowledgements) > limits.report_bytes {
            return Err("report cannot retain its mandatory envelope");
        }
        let (acquired, numeric) = acquisition(earlier, later);
        let budget = OperationBudget::new(limits);
        let mut allowance = ValueAllowance::lend(budget, numeric, limits.equality_pairs);
        let key = |catalogue: &'a Catalogue, id| &catalogue.source(id).expect("model source").key;
        let [documents, others] = self.catalogues.map(|catalogue| {
            catalogue
                .sources()
                .filter(|source| source.role.is_model())
                .map(|source| source.id)
                .collect::<Vec<_>>()
        });
        let mut compared = Vec::new();
        let mut removed = Vec::new();
        for &id in &documents {
            match others
                .iter()
                .find(|&&other| key(later, other) == key(earlier, id))
            {
                Some(&other) => compared.push((id, other)),
                None => removed.push(id),
            }
        }
        let added: Vec<_> = others
            .iter()
            .filter(|&&id| !compared.iter().any(|pair| pair.1 == id))
            .copied()
            .collect();
        // Each document is composed just before its comparison, so the classifications
        // already made stand when a later document exhausts the allowance.
        let mut models = Vec::new();
        let mut required = Vec::new();
        let mut outcome = walk::Outcome::new(self.registry_policy, self.perspective);
        let mut warnings = Vec::new();
        for &(earlier_id, later_id) in &compared {
            let budget = &mut allowance.budget;
            let mut composed = |catalogue, id, acquired| {
                compose(catalogue, id, acquired, budget, &mut models, &mut required)
            };
            let earlier_root = composed(earlier, earlier_id, true)?;
            let later_root = composed(later, later_id, acquired)?;
            let found = outcome.counts[Classification::Breaking.rank()];
            let mut walk = walk::Walk::new(&models, &required, &mut allowance, outcome);
            let [first, second] = self.perspective.ordered([earlier_root, later_root]);
            walk.document(first, second);
            outcome = walk.finish();
            // Every difference of this pair's walk, retained or not, lies in its documents.
            let breaking = outcome.counts[Classification::Breaking.rank()] - found;
            warnings.extend(version::find(
                self.catalogues,
                (earlier_id, later_id),
                breaking,
            ));
        }
        for (revision, ids) in [(Revision::Earlier, &removed), (Revision::Later, &added)] {
            let catalogue = self.catalogues[revision.index()];
            let acquired = revision == Revision::Earlier || acquired;
            for &id in ids {
                let budget = &mut allowance.budget;
                let root = compose(catalogue, id, acquired, budget, &mut models, &mut required)?;
                let mut walk = walk::Walk::new(&models, &required, &mut allowance, outcome);
                let revision = if self.perspective == Perspective::Implementer {
                    revision.other()
                } else {
                    revision
                };
                walk.unmatched(revision, root);
                outcome = walk.finish();
            }
        }
        let walk::Outcome {
            mut records,
            counts,
            unretained,
            registry,
            interpretations,
            ..
        } = outcome;
        if self.perspective == Perspective::Implementer {
            for record in &mut records {
                record.sides.swap(0, 1);
                record.kind = match record.kind {
                    DifferenceKind::Added => DifferenceKind::Removed,
                    DifferenceKind::Removed => DifferenceKind::Added,
                    kind => kind,
                };
                record.verdict = reverse_reason(record.verdict);
                if let Some(witness) = &mut record.witness {
                    witness.reverse_revisions();
                }
            }
        }
        let relocation = models.iter().any(|model| model.relocation_interpretation);
        // The comparison consults no recorded extension evidence.
        let undeclared = allowance.budget.set_aside.relies_on_reading(false);
        let mut comparison = RevisionComparison {
            perspective: self.perspective,
            catalogues: self.catalogues,
            models,
            records,
            counts,
            unretained,
            acknowledgements: self.acknowledgements,
            acknowledged: [0; 3],
            unused: Vec::new(),
            compared,
            removed,
            added,
            warnings,
            relocation,
            undeclared,
            registry,
            interpretations,
        };
        acknowledgement::locate(&mut comparison);
        if let Some(acknowledgements) = self.acknowledgements {
            acknowledgement::acknowledge(&mut comparison, acknowledgements);
        }
        comparison.records.sort_by_key(Record::order);
        Ok(comparison)
    }
}

/// The report bytes both revisions' mandatory envelopes and the unused acknowledgements need.
fn reserve(
    catalogues: [&Catalogue; 2],
    acknowledgements: Option<&DifferenceAcknowledgements>,
) -> usize {
    catalogues[0].reserve
        + catalogues[1].reserve
        + acknowledgements.map_or(0, DifferenceAcknowledgements::reserve)
}

/// Whether the later revision's acquisition fits what the earlier revision left of the one
/// allowance, and the numeric allowance both leave for comparison.
fn acquisition(earlier: &Catalogue, later: &Catalogue) -> (bool, usize) {
    let limits = &earlier.limits;
    let spent = |limit: usize, remaining: usize| limit.saturating_sub(remaining);
    let fits = |limit: usize, a: usize, b: usize| spent(limit, a) + spent(limit, b) <= limit;
    let acquired = fits(
        limits.nodes,
        earlier.parsed_nodes_remaining,
        later.parsed_nodes_remaining,
    ) && fits(
        limits.allocation,
        earlier.parsed_allocation_remaining,
        later.parsed_allocation_remaining,
    ) && fits(
        limits.work,
        earlier.decoding_work_remaining,
        later.decoding_work_remaining,
    ) && fits(
        limits.number_work,
        earlier.numeric_remaining,
        later.numeric_remaining,
    ) && earlier.retained_bytes() + later.retained_bytes() <= limits.catalogue_bytes;
    let numeric = limits.number_work.saturating_sub(
        spent(limits.number_work, earlier.numeric_remaining)
            + spent(limits.number_work, later.numeric_remaining),
    );
    (acquired, numeric)
}

/// Compose a model document on the shared allowance, unless its revision's acquisition
/// already exceeded that allowance, with the requiredness of its groupings beside it.
fn compose<'a>(
    catalogue: &'a Catalogue,
    id: SourceId,
    acquired: bool,
    budget: &mut OperationBudget,
    models: &mut Vec<EffectiveModel<'a>>,
    required: &mut Vec<requiredness::Requiredness>,
) -> Result<walk::Root, &'static str> {
    let unavailable = |message| {
        Ok(walk::Root {
            id,
            model: Err(UndeterminedReason::limit(message)),
        })
    };
    if !acquired {
        return unavailable("revision acquisition exceeds the shared operation allowance");
    }
    if admit(models, &mut budget.allocation).is_err()
        || admit(required, &mut budget.allocation).is_err()
    {
        return unavailable("comparison allocation limit");
    }
    let request = crate::EffectiveRequest::new(id);
    let (model, collection) = catalogue.compose_gated_required(request, budget)?;
    required.push(requiredness::Requiredness::new(
        &model,
        collection.as_ref(),
        budget,
    ));
    models.push(model);
    Ok(walk::Root {
        id,
        model: Ok(models.len() - 1),
    })
}

/// The result of comparing two Model revisions. It borrows both Catalogues and owns
/// their effective models, which its differences and reports borrow.
pub struct RevisionComparison<'a> {
    perspective: Perspective,
    catalogues: [&'a Catalogue; 2],
    models: Vec<EffectiveModel<'a>>,
    records: Vec<Record<'a>>,
    counts: [u32; 3],
    unretained: u32,
    acknowledgements: Option<&'a DifferenceAcknowledgements>,
    /// Acknowledged differences by classification.
    acknowledged: [u32; 3],
    /// The positions of supplied entries that acknowledge no difference.
    unused: Vec<usize>,
    compared: Vec<(SourceId, SourceId)>,
    removed: Vec<SourceId>,
    added: Vec<SourceId>,
    /// The version warnings of compared document pairs, in the earlier revision's source order.
    warnings: Vec<VersionWarning<'a>>,
    relocation: bool,
    /// A classification relied on `undeclared-extensions-ignorable/1`.
    undeclared: bool,
    /// The recorded evidence policy, the registry sections unit and content-format comparison
    /// consulted, and the content-format meaning interpretation.
    registry: crate::registry::State,
    /// The value interpretations that Breaking witnesses relied on.
    interpretations: crate::sample::ValueInterpretations,
}
impl<'a> RevisionComparison<'a> {
    pub fn perspective(&self) -> Perspective {
        self.perspective
    }
    pub fn catalogue(&self, revision: Revision) -> &'a Catalogue {
        self.catalogues[revision.index()]
    }
    pub fn contract(&self) -> &'static str {
        COMPARISON_CONTRACT
    }
    /// Breaking when any difference is breaking, otherwise undetermined when any is
    /// undetermined, otherwise compatible. Acknowledgements leave it unchanged.
    pub fn outcome(&self) -> Classification {
        if self.count(Classification::Breaking) > 0 {
            Classification::Breaking
        } else if self.count(Classification::Undetermined) > 0 {
            Classification::Undetermined
        } else {
            Classification::Compatible
        }
    }
    /// Failed when any unacknowledged difference is breaking, otherwise undetermined when any
    /// unacknowledged difference is undetermined, otherwise passed. Without acknowledgements it
    /// is the outcome's disposition.
    pub fn disposition(&self) -> Disposition {
        let open =
            |classification| self.count(classification) - self.acknowledged_count(classification);
        if open(Classification::Breaking) > 0 {
            Disposition::Failed
        } else if open(Classification::Undetermined) > 0 {
            Disposition::Undetermined
        } else {
            Disposition::Passed
        }
    }
    /// Whether no unacknowledged difference is undetermined, so the one requested comparison
    /// is assessed.
    pub fn complete(&self) -> bool {
        self.count(Classification::Undetermined)
            == self.acknowledged_count(Classification::Undetermined)
    }
    /// Every difference of this classification, including any whose details could not be
    /// retained.
    pub fn count(&self, classification: Classification) -> u32 {
        self.counts[classification.rank()]
    }
    /// Every difference of every classification, including any whose details could not be
    /// retained.
    pub(crate) fn total(&self) -> u64 {
        self.counts.iter().copied().map(u64::from).sum()
    }
    /// The acknowledged differences of this classification. Only retained differences can be
    /// acknowledged.
    pub fn acknowledged_count(&self, classification: Classification) -> u32 {
        self.acknowledged[classification.rank()]
    }
    /// The zero-based positions, among the supplied acknowledgements, of the entries that
    /// acknowledge no difference.
    pub fn unused_acknowledgements(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        self.unused.iter().copied()
    }
    /// Retained differences: unacknowledged breaking ones first, then unacknowledged
    /// undetermined, acknowledged breaking, acknowledged undetermined and compatible. Fewer
    /// than the counts are retained only when the operation allowance ran out of storage.
    pub fn differences(&self) -> impl ExactSizeIterator<Item = Difference<'_>> {
        self.records.iter().map(|record| Difference {
            comparison: self,
            record,
        })
    }
    /// The compared document pairs with a breaking difference whose later `info` `version`
    /// does not compare higher, one warning each. Warnings never change the disposition.
    pub fn version_warnings(&self) -> impl ExactSizeIterator<Item = &VersionWarning<'a>> {
        self.warnings.iter()
    }
    /// Documents corresponding by source key, as earlier and later source IDs.
    pub fn compared_documents(&self) -> impl ExactSizeIterator<Item = (SourceId, SourceId)> {
        self.compared.iter().copied()
    }
    /// Earlier model documents without a later document of the same key.
    pub fn removed_documents(&self) -> &[SourceId] {
        &self.removed
    }
    /// Later model documents without an earlier document of the same key.
    pub fn added_documents(&self) -> &[SourceId] {
        &self.added
    }
    /// The recorded registry evidence that unit and content-format comparison consulted or
    /// found withheld, Absent when no change needed it.
    pub fn registry_knowledge(&self, id: &str) -> Field<crate::RegistryKnowledgeView> {
        self.registry.view(id)
    }
    pub fn to_json(&self) -> String {
        report::to_json(self)
    }
    pub fn to_human(&self) -> String {
        self.to_human_with_style(crate::HumanStyle::Plain)
    }
    pub fn to_human_with_style(&self, style: crate::HumanStyle) -> String {
        report::to_human(self, style)
    }
    /// Render the comparison as a SARIF 2.1.0 log within the report limit, following ADR 0043:
    /// SARIF and Markdown report projections. Artifact URIs of file sources are their paths
    /// relative to `directory`, the directory relative paths were resolved from.
    pub fn to_sarif(&self, directory: &std::path::Path) -> String {
        crate::sarif::diff(self, directory)
    }
    /// Render the comparison as a Markdown changelog within the report limit, following
    /// ADR 0043: SARIF and Markdown report projections.
    pub fn to_markdown(&self) -> String {
        changelog::render(self)
    }
    fn json(&self, side: &Side<'_>, node: usize) -> EffectiveJson<'_> {
        EffectiveJson {
            model: &self.models[side.model.expect("composed side")],
            node,
        }
    }
}

/// A classified difference between corresponding content of the two revisions.
#[derive(Clone, Copy)]
pub struct Difference<'r> {
    comparison: &'r RevisionComparison<'r>,
    record: &'r Record<'r>,
}
impl<'r> Difference<'r> {
    pub fn kind(&self) -> DifferenceKind {
        self.record.kind
    }
    pub fn subject(&self) -> DifferenceSubject<'r> {
        self.record.subject
    }
    pub fn classification(&self) -> Classification {
        self.record.verdict.classification()
    }
    /// A supplied Difference acknowledgement matches this difference.
    pub fn acknowledged(&self) -> bool {
        self.record.acknowledged
    }
    /// The entry that would acknowledge this difference, present when it can be acknowledged
    /// and its effective pointers can be rendered. Matching compares exactly this entry.
    pub fn acknowledgement(&self) -> Option<DifferenceAcknowledgement> {
        acknowledgement::entry(self.comparison, self.record)
    }
    /// Present exactly when the difference is undetermined.
    pub fn reason(&self) -> Option<UndeterminedReason> {
        self.record.verdict.reason()
    }
    /// The Breaking witness of a data difference that is breaking because of one.
    pub fn witness(&self) -> Option<&'r Witness> {
        self.record.witness.as_ref()
    }
    /// The Client directions of the data this difference affects, among those both
    /// revisions offer. A changed Property characteristic affects the direction it allows:
    /// Client-supplied data when writable, Thing-supplied data when readable or observable.
    /// Other differences outside data descriptions affect none.
    pub fn directions(&self) -> impl Iterator<Item = Direction> + use<> {
        let bits = self.record.directions;
        DIRECTIONS
            .into_iter()
            .filter(move |direction| bits & direction.bit() != 0)
    }
    pub fn affects(&self, direction: Direction) -> bool {
        self.record.directions & direction.bit() != 0
    }
    /// The difference's location and origin in one revision, absent when that revision
    /// lacks the corresponding definition, `properties` entry or document.
    pub fn side(&self, revision: Revision) -> Option<DifferenceSide<'r>> {
        self.record.sides[revision.index()]
            .as_ref()
            .map(|side| DifferenceSide {
                comparison: self.comparison,
                side,
            })
    }
}

/// One revision's side of a difference.
#[derive(Clone, Copy)]
pub struct DifferenceSide<'r> {
    comparison: &'r RevisionComparison<'r>,
    side: &'r Side<'r>,
}
impl<'r> DifferenceSide<'r> {
    pub fn presence(&self) -> Presence {
        self.side.presence
    }
    /// The document whose effective view holds the location, numbered within its revision.
    pub fn root_document(&self) -> SourceId {
        self.side.root
    }
    /// The effective location. An absent quality's location names where it would be.
    pub fn pointer(&self) -> Field<String> {
        self.pointer_with_limit(8192)
    }
    pub(crate) fn pointer_with_limit(&self, limit: u32) -> Field<String> {
        if limit == 0 || limit > 8192 {
            return Field::Unavailable("unsupported pointer output budget");
        }
        let limits = &self.comparison.catalogues[0].limits;
        let mut work = crate::json::PointerBudget::new(limits.pointer_total);
        match report::render_pointer(self.comparison, self.side, limit as usize, &mut work) {
            Ok(pointer) => Field::Present(pointer),
            Err(_) => Field::Unavailable("effective pointer unavailable or limited"),
        }
    }
    /// The nearest content origin, or the deletion that removed an absent quality.
    pub fn origin(&self) -> Option<crate::Explanation<'r>> {
        self.side
            .origin
            .map(|node| crate::Explanation::selected(self.comparison.json(self.side, node)))
    }
}

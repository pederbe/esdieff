//! Borrowed final support is separate from request-wide convention consultation.
use super::{AREAS, Admission, ContextFindings, ContextState, Evaluation, State, Status};
use crate::BasisKind;
use crate::Field;
use crate::finding::Basis;

#[derive(Clone, Copy)]
/// One immutable reference supporting a conclusion or explaining a limitation.
/// Identity is the complete kind, optional ID and optional citation triple.
pub struct SampleBasisRef {
    kind: BasisKind,
    id: Option<&'static str>,
    citation: Option<&'static str>,
}
impl SampleBasisRef {
    pub fn kind(self) -> BasisKind {
        self.kind
    }
    pub fn id(self) -> Option<&'static str> {
        self.id
    }
    pub fn citation(self) -> Option<&'static str> {
        self.citation
    }
    fn from_basis(basis: Basis) -> Self {
        Self {
            kind: match basis.kind() {
                "interpretation" => BasisKind::Interpretation,
                "operational" => BasisKind::Operational,
                "rfc" => BasisKind::Rfc,
                _ => unreachable!("fixed sample basis kind"),
            },
            // Knowledge editions belong to temporal metadata, not RFC references.
            id: (basis.kind() != "rfc").then(|| basis.id()).flatten(),
            citation: (!basis.citation().is_empty()).then_some(basis.citation()),
        }
    }
}

// Keep the precise producing references, including compound and format rules.
pub(super) const SUPPORT: [Basis; 24] = [
    Basis::Numeric,
    Basis::Text,
    Basis::Boolean,
    Basis::Nullable,
    Basis::TypeNull,
    Basis::Const,
    Basis::Alternatives,
    Basis::Map,
    Basis::Array,
    Basis::Temporal,
    Basis::TemporalRepresentation,
    Basis::TemporalCalendar,
    Basis::TemporalClock,
    Basis::TemporalSlot,
    Basis::TemporalLeap,
    Basis::Resource,
    Basis::AbstractMixed,
    Basis::UriSyntax,
    Basis::HttpIdentity,
    Basis::UuidText,
    Basis::UuidValue,
    Basis::Abstract,
    Basis::Byte,
    Basis::Unix,
];
// Only abstract-value producers insert into this area. Ordinary type, const,
// choice and compound rules retain their distinct admission/const references.
pub(super) const ABSTRACT_SUPPORT: u32 = ((1 << 24) - 1) & !((1 << 9) - 1);
// Definite contextual comparators establish these denotations. Representation
// rejection references belong to abstract/admission producers, not a successful
// comparison of two established values.
pub(super) const CONST_SUPPORT: u32 = (1 << 0)
    | (1 << 1)
    | (1 << 2)
    | (1 << 4)
    | (1 << 5)
    | (1 << 6)
    | (1 << 7)
    | (1 << 8)
    | (1 << 9)
    | (1 << 15)
    | (1 << 20)
    | (1 << 21)
    | (1 << 22)
    | (1 << 23);
const LIMIT_IDS: [&str; 6] = [
    "sample-establishment",
    "sample-implementation",
    "sample-abstract-denotation-unavailable",
    "sample-work-limit",
    "sample-number-work-limit",
    "sample-owner-limit",
];
// The existing cited operational references remain distinct from the new causes.
const CITED_LIMITS: [Basis; 3] = [
    Basis::TemporalKnowledge,
    Basis::TemporalMeaning,
    Basis::ResourceMeaning,
];

#[derive(Clone, Copy, Default)]
pub(super) struct MeaningRecord([u8; 15]);
impl MeaningRecord {
    fn insert(&mut self, slot: usize, basis: Basis) {
        self.insert_mask(slot, basis.support_mask());
    }
    fn insert_mask(&mut self, slot: usize, mask: u32) {
        if slot == 0 {
            assert_eq!(mask & !ABSTRACT_SUPPORT, 0);
        } else if slot == 1 {
            assert_eq!(mask & !CONST_SUPPORT, 0);
        }
        self.0[slot * 5] |= mask as u8;
        self.0[slot * 5 + 1] |= (mask >> 8) as u8;
        self.0[slot * 5 + 2] |= (mask >> 16) as u8;
    }
    fn clear_support(&mut self, slot: usize) {
        self.0[slot * 5..slot * 5 + 3].fill(0);
    }
    fn limit(&mut self, slot: usize, index: u8) {
        assert!(index < 9);
        self.0[slot * 5 + 3 + usize::from(index / 8)] |= 1 << (index % 8);
    }
    fn merge_limits(&mut self, slot: usize, mask: u16) {
        assert_eq!(mask & !511, 0);
        self.0[slot * 5 + 3] |= mask as u8;
        self.0[slot * 5 + 4] |= (mask >> 8) as u8;
    }
    fn replace_limits(&mut self, slot: usize, mask: u16) {
        self.0[slot * 5 + 3] = mask as u8;
        self.0[slot * 5 + 4] = (self.0[slot * 5 + 4] & 128) | (mask >> 8) as u8;
    }
    fn has_limits(&self, slot: usize) -> bool {
        self.limitations(slot) != 0
    }
    fn has_limit(&self, slot: usize, index: u8) -> bool {
        self.limitations(slot) & (1 << index) != 0
    }
    fn copy_const_support_to_conclusion(&mut self) {
        self.0[10] |= self.0[5];
        self.0[11] |= self.0[6];
        self.0[12] |= self.0[7];
    }
    fn const_primary(&self) -> Basis {
        match self.support(1) & ABSTRACT_SUPPORT {
            mask if mask == Basis::Byte.support_mask() => Basis::Byte,
            mask if mask == Basis::Unix.support_mask() => Basis::Unix,
            _ => Basis::Const,
        }
    }
    fn merge_areas(&mut self, other: &Self) {
        for index in 0..10 {
            self.0[index] |= other.0[index];
        }
        self.0[14] |= other.0[14] & 128;
    }
    fn merge_conclusion(&mut self, other: &Self) {
        for index in 10..15 {
            self.0[index] |= other.0[index];
        }
    }
    fn merge_conclusion_areas(&mut self) {
        self.0[10] |= self.0[0] | self.0[5];
        self.0[11] |= self.0[1] | self.0[6];
        self.0[12] |= self.0[2] | self.0[7];
    }
    fn merge_conclusion_limits(&mut self) {
        self.0[13] |= self.0[3] | self.0[8];
        self.0[14] |= self.0[4] | self.0[9];
    }
    fn normalize(&mut self, areas: &[Status; 9], admission: super::Admission) {
        for (slot, area) in [(0, 8), (1, 4)] {
            if areas[area] == Status::NotApplicable {
                self.0[slot * 5..slot * 5 + 5].fill(0);
            }
        }
        if admission == super::Admission::Undetermined {
            self.clear_support(2);
        } else {
            self.0[13] = 0;
            self.0[14] &= 128;
        }
    }
    fn witness_failed(&self) -> bool {
        self.0[14] & 128 != 0
    }
    fn mark_witness_failed(&mut self) {
        self.0[14] |= 128;
        self.0[3] |= 1 << 5;
        self.0[8] |= 1 << 5;
        self.0[13] |= 1 << 5;
    }
    fn support(&self, slot: usize) -> u32 {
        let offset = slot * 5;
        u32::from(self.0[offset])
            | (u32::from(self.0[offset + 1]) << 8)
            | (u32::from(self.0[offset + 2]) << 16)
    }
    fn limitations(&self, slot: usize) -> u32 {
        let offset = slot * 5 + 3;
        (u32::from(self.0[offset]) | (u32::from(self.0[offset + 1]) << 8)) & 511
    }
}
impl Evaluation {
    pub(super) fn begin_abstract_meaning(&mut self, basis: Basis) {
        self.areas[8] = Status::Assessed;
        self.abstract_basis = basis;
    }
    pub(super) fn abstract_established(&mut self) {
        self.areas[8] = Status::Assessed;
        self.meaning.insert(0, self.abstract_basis);
    }
    pub(super) fn abstract_excluded(&mut self, basis: Basis) {
        self.areas[8] = Status::Assessed;
        self.meaning.insert(0, basis);
        self.exclude(basis, 0);
    }
    pub(super) fn abstract_unfinished(&mut self, cause: u8) {
        self.areas[8] = Status::Undetermined;
        self.abstract_unknown |= 2;
        self.context = ContextState::Partial;
        self.meaning.limit(0, cause);
        self.total_findings += 1;
    }
    pub(super) fn const_comparison(&mut self, supporting: u32) {
        self.areas[4] = Status::Assessed;
        self.meaning.insert_mask(1, supporting);
    }
    pub(super) fn const_established(&mut self) {
        self.areas[4] = Status::Assessed;
        self.meaning.insert(1, Basis::Const);
    }
    pub(super) fn const_unfinished(&mut self, limitations: u16) {
        self.meaning.merge_limits(1, limitations);
        self.areas[4] = Status::Undetermined;
        self.context = ContextState::Partial;
    }
    pub(super) fn admission_unfinished(&mut self, area: usize, status: Status, cause: u8) {
        debug_assert!(matches!(status, Status::Undetermined | Status::Unassessed));
        self.areas[area] = status;
        self.meaning.limit(2, cause);
    }
    pub(super) fn items_comparison_unfinished(&mut self, limitations: u16) {
        // Pair-specific evidence remains with the contextual equality handoff.
        // A later equal pair can still settle the active uniqueness condition.
        self.meaning.merge_limits(2, limitations);
    }
    pub(super) fn exclude_constraint(&mut self, basis: Basis) {
        self.exclude(basis, 0);
        if basis == Basis::Const {
            self.meaning.copy_const_support_to_conclusion();
        }
    }
    pub(super) fn exclude_equal_items(&mut self, supporting: u32) {
        self.exclude(Basis::Array, supporting);
    }
    fn exclude(&mut self, basis: Basis, supporting: u32) {
        // Replace tentative success only at the first established rejection.
        // These authoritative updates precede every optional finding retention.
        self.begin_rejection();
        self.meaning.insert(2, basis);
        self.meaning.insert_mask(2, supporting);
        self.errors += 1;
        self.total_findings += 1;
    }
    fn begin_rejection(&mut self) {
        if self.errors == 0 {
            self.meaning.clear_support(2);
        }
    }
    pub(super) fn finish_choice(&mut self, unknown: bool) {
        if unknown {
            self.choice_unfinished();
        } else {
            self.areas[5] = Status::Assessed;
            if self.errors > 0 {
                self.meaning.insert(2, Basis::Alternatives);
            }
        }
        self.finish_value();
    }
    pub(super) fn accept_choice_witness(&mut self, mut witness: Self) {
        witness.areas[5] = Status::Assessed;
        witness.meaning.insert(2, Basis::Alternatives);
        // One complete alternative replaces tentative branch proof. Actual
        // request consultation remains with the shared equality work owner.
        *self = witness;
    }
    pub(crate) fn const_basis(&self) -> Basis {
        self.meaning.const_primary()
    }
    pub(crate) fn abstract_unavailable(&self, area: &str) -> bool {
        let slot = if area == AREAS[8] {
            0
        } else if area == AREAS[4] {
            1
        } else {
            return false;
        };
        self.meaning.has_limit(slot, 2)
    }
    pub(crate) fn witness_node(&self) -> Field<usize> {
        if self.meaning.witness_failed() {
            Field::Unavailable("sample witness index unavailable")
        } else {
            self.witness
                .map_or(Field::Absent, |node| Field::Present(node.get() - 1))
        }
    }
    pub(super) fn assign_witness(&mut self, node: usize) {
        self.witness = node.checked_add(1).and_then(std::num::NonZeroUsize::new);
        if self.witness.is_none() {
            self.meaning.mark_witness_failed();
            self.areas[0] = Status::Undetermined;
            self.context = ContextState::Partial;
            self.finish_value();
        }
    }
    pub(super) fn empty() -> Self {
        let mut areas = [Status::NotApplicable; 9];
        areas[0] = Status::Assessed;
        Self {
            meaning: MeaningRecord::default(),
            sample_node: 0,
            admission: Admission::Undetermined,
            context: ContextState::Available,
            context_findings: ContextFindings::default(),
            areas,
            type_null_basis: Basis::Numeric,
            text_boolean_basis: Basis::Numeric,
            abstract_basis: Basis::Numeric,
            abstract_unknown: 0,
            temporal_use: 0,
            findings: Vec::new(),
            total_findings: 0,
            errors: 0,
            storage_omitted: false,
            witness: None,
        }
    }
    pub(super) fn unavailable() -> Self {
        let mut result = Self::empty();
        result.abstract_unknown = 1;
        result.areas = [Status::Undetermined; 9];
        result.context = ContextState::Partial;
        result.meaning.limit(0, 0);
        result.meaning.limit(1, 0);
        result.meaning.limit(2, 0);
        result
    }
    pub(super) fn unavailable_cause(cause: u8) -> Self {
        let mut result = Self::unavailable();
        for slot in 0..3 {
            result.meaning.replace_limits(slot, 1 << cause);
        }
        result
    }
    pub(super) fn finish_value(&mut self) {
        if matches!(self.areas[8], Status::Unassessed | Status::Undetermined)
            && self.abstract_unknown == 0
        {
            self.abstract_unknown = 1;
        }
        self.admission = if self.errors > 0 {
            Admission::Rejected
        } else if self
            .areas
            .iter()
            .all(|area| matches!(area, Status::Assessed | Status::NotApplicable))
        {
            Admission::Admitted
        } else {
            Admission::Undetermined
        };
        if self.admission == Admission::Admitted {
            self.meaning.insert(2, Basis::Const);
            for (area, basis) in [
                (1, self.type_null_basis),
                (2, Basis::Numeric),
                (3, self.text_boolean_basis),
                (5, Basis::Alternatives),
                (6, Basis::Map),
                (7, Basis::Array),
            ] {
                if self.areas[area] == Status::Assessed {
                    self.meaning.insert(2, basis);
                }
            }
            self.meaning.merge_conclusion_areas();
        } else if self.admission == Admission::Undetermined {
            self.meaning.merge_conclusion_limits();
            if !self.meaning.has_limits(2) {
                self.meaning.limit(
                    2,
                    if self.areas.contains(&Status::Unassessed) {
                        1
                    } else {
                        0
                    },
                );
            }
        }
        self.finish_area_limits();
        self.meaning.normalize(&self.areas, self.admission);
    }
    fn finish_area_limits(&mut self) {
        for (slot, area) in [(0, 8), (1, 4)] {
            if matches!(self.areas[area], Status::Unassessed | Status::Undetermined)
                && !self.meaning.has_limits(slot)
            {
                self.meaning.limit(
                    slot,
                    if self.areas[area] == Status::Unassessed {
                        1
                    } else {
                        0
                    },
                );
            }
        }
    }
    pub(super) fn seal_request(&mut self, context_cause: Option<u8>) {
        if let Some(cause) = context_cause {
            self.areas[0] = Status::Undetermined;
            if self.admission == Admission::Admitted {
                self.admission = Admission::Undetermined;
            }
            self.meaning.limit(2, cause);
        }
        self.finish_area_limits();
        if self.admission == Admission::Undetermined {
            self.meaning.merge_conclusion_limits();
            if !self.meaning.has_limits(2) {
                self.meaning.limit(2, 0);
            }
        }
        let areas = self.areas;
        let admission = self.admission;
        self.meaning.normalize(&areas, admission);
    }
    pub(super) fn choice_unfinished(&mut self) {
        self.total_findings -= self.errors as usize;
        self.errors = 0;
        self.findings.retain(|finding| finding.basis.incomplete());
        self.areas[5] = Status::Undetermined;
        self.context = ContextState::Partial;
        self.meaning.clear_support(2);
        self.meaning.limit(2, 0);
    }
    pub(super) fn conjoin_child(&mut self, source: &Self) {
        self.meaning.merge_areas(&source.meaning);
        if source.errors > 0 {
            self.begin_rejection();
            self.meaning.merge_conclusion(&source.meaning);
        } else if self.errors == 0 {
            self.meaning.merge_conclusion(&source.meaning);
        }
        self.temporal_use |= source.temporal_use;
        self.abstract_unknown |= source.abstract_unknown;
        if source.areas[8] != Status::NotApplicable {
            if self.areas[8] == Status::NotApplicable {
                self.abstract_basis = source.abstract_basis;
            } else if self.abstract_basis != source.abstract_basis {
                self.abstract_basis = if matches!(
                    self.abstract_basis,
                    Basis::Abstract | Basis::Byte | Basis::Unix
                ) || matches!(
                    source.abstract_basis,
                    Basis::Abstract | Basis::Byte | Basis::Unix
                ) {
                    Basis::Abstract
                } else {
                    Basis::AbstractMixed
                };
            }
        }
        self.context =
            if self.context == ContextState::Partial || source.context == ContextState::Partial {
                ContextState::Partial
            } else {
                ContextState::Available
            };
        for (area, target_basis, source_basis) in [
            (1, &mut self.type_null_basis, source.type_null_basis),
            (3, &mut self.text_boolean_basis, source.text_boolean_basis),
        ] {
            if source.areas[area] != Status::NotApplicable {
                if self.areas[area] == Status::NotApplicable {
                    *target_basis = source_basis;
                } else if *target_basis != source_basis {
                    *target_basis = Basis::TypeNull;
                }
            }
        }
        for (target, source) in self.areas.iter_mut().zip(source.areas) {
            let rank = |status| match status {
                Status::Undetermined => 3,
                Status::Unassessed => 2,
                Status::Assessed => 1,
                Status::NotApplicable => 0,
            };
            if rank(source) > rank(*target) {
                *target = source
            }
        }
        self.errors += source.errors;
        self.total_findings += source.total_findings;
        self.storage_omitted |= source.storage_omitted;
    }
}

struct BasisIter {
    mask: u32,
    limitations: bool,
}
impl BasisIter {
    fn supporting(mask: u32) -> Self {
        // Slot and leap rules share one RFC reference after knowledge metadata
        // is kept outside this view. Retain both producing tags in the owner.
        let mask = if mask & (1 << 13) != 0 {
            mask & !(1 << 14)
        } else {
            mask
        };
        Self {
            mask,
            limitations: false,
        }
    }
}
impl Iterator for BasisIter {
    type Item = SampleBasisRef;
    fn next(&mut self) -> Option<Self::Item> {
        if self.mask == 0 {
            return None;
        }
        let index = self.mask.trailing_zeros() as usize;
        self.mask &= self.mask - 1;
        Some(if !self.limitations {
            SampleBasisRef::from_basis(SUPPORT[index])
        } else if index < LIMIT_IDS.len() {
            SampleBasisRef {
                kind: BasisKind::Operational,
                id: Some(LIMIT_IDS[index]),
                citation: None,
            }
        } else {
            SampleBasisRef::from_basis(CITED_LIMITS[index - LIMIT_IDS.len()])
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}
impl ExactSizeIterator for BasisIter {
    fn len(&self) -> usize {
        self.mask.count_ones() as usize
    }
}
/// Final Sample proof and limitations, independent of consultation history.
pub struct SampleMeaning<'a>(&'a State);
impl<'a> SampleMeaning<'a> {
    pub fn abstract_values(&self) -> SampleMeaningArea<'a> {
        SampleMeaningArea {
            record: &self.0.meaning,
            slot: 0,
            status: self.0.areas[8],
        }
    }
    pub fn const_values(&self) -> SampleMeaningArea<'a> {
        SampleMeaningArea {
            record: &self.0.meaning,
            slot: 1,
            status: self.0.areas[4],
        }
    }
    pub fn admission(&self) -> SampleMeaningConclusion<'a> {
        SampleMeaningConclusion {
            record: &self.0.meaning,
        }
    }
}
/// One area's authoritative status and unique supporting/limiting references.
pub struct SampleMeaningArea<'a> {
    record: &'a MeaningRecord,
    slot: usize,
    status: Status,
}
impl SampleMeaningArea<'_> {
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn basis(&self) -> impl ExactSizeIterator<Item = SampleBasisRef> {
        BasisIter::supporting(self.record.support(self.slot))
    }
    pub fn limitations(&self) -> impl ExactSizeIterator<Item = SampleBasisRef> {
        BasisIter {
            mask: self.record.limitations(self.slot),
            limitations: true,
        }
    }
}
/// The final admission proof and causes of unfinished admission work.
/// Undetermined admission has no supporting proof.
pub struct SampleMeaningConclusion<'a> {
    record: &'a MeaningRecord,
}
impl SampleMeaningConclusion<'_> {
    pub fn basis(&self) -> impl ExactSizeIterator<Item = SampleBasisRef> {
        BasisIter::supporting(self.record.support(2))
    }
    pub fn limitations(&self) -> impl ExactSizeIterator<Item = SampleBasisRef> {
        BasisIter {
            mask: self.record.limitations(2),
            limitations: true,
        }
    }
}
impl<'a> super::SampleResult<'a> {
    /// Borrow the final abstract-value, const and admission reference sets.
    /// These mandatory facts survive omission of optional presentation detail.
    pub fn meaning(&self) -> SampleMeaning<'a> {
        SampleMeaning(self.state)
    }
    /// Conventional byte/POSIX interpretations actually attempted in this request.
    /// Discarding a witness or exhausting work does not erase consultation.
    pub fn consulted_interpretations(&self) -> impl ExactSizeIterator<Item = &'static str> {
        ConsultationIter(self.state.abstract_use & 3)
    }
}
struct ConsultationIter(u8);
impl Iterator for ConsultationIter {
    type Item = &'static str;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let bit = self.0.trailing_zeros();
        self.0 &= self.0 - 1;
        Some(if bit == 0 {
            "byte-string-json/1"
        } else {
            "unix-time-posix-coordinate/1"
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}
impl ExactSizeIterator for ConsultationIter {
    fn len(&self) -> usize {
        self.0.count_ones() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triple(
        reference: SampleBasisRef,
    ) -> (BasisKind, Option<&'static str>, Option<&'static str>) {
        (reference.kind(), reference.id(), reference.citation())
    }

    #[test]
    fn reference_projection_is_unique_ordered_and_exact_sized() {
        let slot = 1 << 13;
        let leap = 1 << 14;
        let mut complete = Vec::new();
        for basis in SUPPORT {
            let reference = SampleBasisRef::from_basis(basis);
            if reference.kind() == BasisKind::Rfc {
                assert!(reference.id().is_none());
            }
            if !complete.contains(&triple(reference)) {
                complete.push(triple(reference));
            }
        }
        assert_eq!(SUPPORT.len(), 24);
        assert_eq!(complete.len(), 23);
        for mask in [
            0,
            slot,
            leap,
            slot | leap,
            (1 << 12) | slot | leap | (1 << 15),
            (1 << 24) - 1,
        ] {
            let mut expected = Vec::new();
            for (index, basis) in SUPPORT.iter().enumerate() {
                if mask & (1 << index) != 0 {
                    let reference = triple(SampleBasisRef::from_basis(*basis));
                    if !expected.contains(&reference) {
                        expected.push(reference);
                    }
                }
            }
            let mut iterator = BasisIter::supporting(mask);
            let expected_count = expected.len();
            for (index, expected) in expected.iter().enumerate() {
                let remaining = iterator.len();
                assert_eq!(remaining, expected_count - index);
                assert_eq!(iterator.size_hint(), (remaining, Some(remaining)));
                assert_eq!(triple(iterator.next().unwrap()), *expected);
                assert_eq!(iterator.len(), remaining - 1);
                assert_eq!(iterator.size_hint(), (remaining - 1, Some(remaining - 1)));
            }
            assert!(iterator.next().is_none());
            assert_eq!(iterator.len(), 0);
            assert_eq!(iterator.size_hint(), (0, Some(0)));
        }
    }
    #[test]
    fn nine_limitation_triples_preserve_existing_citations() {
        let mut iterator = BasisIter {
            mask: 511,
            limitations: true,
        };
        let mut actual = Vec::new();
        for remaining in (1..=9).rev() {
            assert_eq!(iterator.len(), remaining);
            assert_eq!(iterator.size_hint(), (remaining, Some(remaining)));
            let reference = iterator.next().unwrap();
            assert_eq!(reference.kind(), BasisKind::Operational);
            assert!(reference.id().is_some());
            assert_eq!(reference.citation().is_some(), actual.len() >= 6);
            assert!(!actual.contains(&triple(reference)));
            actual.push(triple(reference));
        }
        assert_eq!(
            actual
                .iter()
                .filter(|reference| reference.1 == Some("sample-establishment"))
                .count(),
            3
        );
        assert!(iterator.next().is_none());
        assert_eq!(iterator.size_hint(), (0, Some(0)));
    }
}

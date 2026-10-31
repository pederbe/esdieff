//! Explain retained candidates using their construction aim and settled admission.
//! This is evidence about values, not an inventory of constraints or Choice branches.
use crate::Admission;

/// What one retained Generated value establishes for the generated corpus.
/// Counts of these observations do not measure RFC coverage or operation completeness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GenerationEvidence {
    /// An admitted candidate for an example target, or an admitted outside candidate.
    AdmittedExample,
    /// A boundary candidate admitted by the whole selected description.
    AdmittedBoundary,
    /// An outside candidate rejected by the whole selected description. Other
    /// constraints may also reject it, so this does not establish an isolated cause.
    RejectedOutside,
    /// A rejected example or boundary candidate. It establishes no admitted example.
    RejectedCandidate,
    /// Checking could not settle this candidate's admission.
    Undetermined,
}

impl GenerationEvidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AdmittedExample => "admitted_example",
            Self::AdmittedBoundary => "admitted_boundary",
            Self::RejectedOutside => "rejected_outside",
            Self::RejectedCandidate => "rejected_candidate",
            Self::Undetermined => "undetermined",
        }
    }
}

/// The construction aim of a bare target, recorded before nested names are attached.
/// Entry names can contain dots or even be named after another target.
#[derive(Clone, Copy)]
pub(super) enum Aim {
    Example,
    Boundary,
    Outside,
}

impl Aim {
    pub(super) fn of(target: &str) -> Self {
        match target {
            "minimum" | "maximum" | "min_length" | "max_length" | "min_items" | "max_items" => {
                Self::Boundary
            }
            "below_minimum" | "above_maximum" | "not_multiple" | "too_short" | "too_long"
            | "too_few_items" | "too_many_items" | "duplicate_items" | "missing_entry"
            | "wrong_type" => Self::Outside,
            _ => Self::Example,
        }
    }

    pub(super) fn evidence(self, admission: Admission) -> GenerationEvidence {
        match (self, admission) {
            (Self::Boundary, Admission::Admitted) => GenerationEvidence::AdmittedBoundary,
            (_, Admission::Admitted) => GenerationEvidence::AdmittedExample,
            (Self::Outside, Admission::Rejected) => GenerationEvidence::RejectedOutside,
            (_, Admission::Rejected) => GenerationEvidence::RejectedCandidate,
            (_, Admission::Undetermined) => GenerationEvidence::Undetermined,
        }
    }
}

//! Expected-result matching for native Model and Sample-value checking results.
//! Every expectation and every observed result must be accounted for exactly once.
use crate::expected_native::{
    DecisiveDiagnostic, ModelExpectation, SAMPLE_AREAS, SAMPLE_INVENTORY, SampleExpectation,
};
use crate::native::{Coverage, ModelCheck, NativeResult, SampleCheck};
use crate::wording::mismatch;
use esdieff::{Admission, Disposition};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
};

/// Matched expectations and every mismatch, each naming its case and what differed.
#[derive(Debug, Default)]
pub struct Summary {
    pub expected: usize,
    pub matched: usize,
    pub mismatches: Vec<String>,
}
impl Summary {
    pub fn passed(&self) -> bool {
        self.expected > 0 && self.matched == self.expected && self.mismatches.is_empty()
    }
}

/// Native sample expectations with the observed admission counts kept visible.
#[derive(Debug, Default)]
pub struct NativeSummary {
    pub summary: Summary,
    pub admitted: usize,
    pub rejected: usize,
    pub undetermined: usize,
    pub unavailable: usize,
}

fn differ<T: Display + PartialEq>(
    problems: &mut Vec<String>,
    case: &dyn Display,
    field: &str,
    expected: T,
    observed: T,
) {
    problems
        .extend(mismatch(field, expected, observed).map(|problem| format!("{case}: {problem}")));
}

fn shown<T: Display>(value: Option<T>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.to_string())
}

/// Every expected result in this demonstration completes its requested inventory.
fn coverage(
    problems: &mut Vec<String>,
    case: &dyn Display,
    observed: &Coverage,
    inventory: &str,
    areas: u32,
) {
    differ(
        problems,
        case,
        "requested inventory",
        inventory,
        observed.inventory.as_deref().unwrap_or("none"),
    );
    differ(
        problems,
        case,
        "requested total",
        areas.to_string(),
        shown(observed.total),
    );
    let counted = [
        observed.assessed,
        observed.not_applicable,
        observed.unassessed,
        observed.undetermined,
    ]
    .iter()
    .try_fold(0u32, |sum, count| sum.checked_add(*count));
    differ(
        problems,
        case,
        "coverage accounting",
        areas.to_string(),
        shown(counted),
    );
    differ(
        problems,
        case,
        "coverage completion",
        true,
        observed.unassessed == 0 && observed.undetermined == 0,
    );
}

pub fn match_models(expected: &[ModelExpectation], observed: &[ModelCheck]) -> Summary {
    let mut summary = Summary {
        expected: expected.len(),
        ..Summary::default()
    };
    let mut seen = BTreeSet::new();
    for check in observed {
        let case = check.case;
        if !seen.insert(case) {
            summary
                .mismatches
                .push(format!("{case}: duplicate model result"));
            continue;
        }
        let Some(expectation) = expected.iter().find(|e| e.case == check.case) else {
            summary
                .mismatches
                .push(format!("{case}: unexpected model result"));
            continue;
        };
        let native = match &check.native {
            Ok(native) => native,
            Err(reason) => {
                summary
                    .mismatches
                    .push(format!("{case}: native result unavailable ({reason})"));
                continue;
            }
        };
        let mut problems = Vec::new();
        differ(
            &mut problems,
            &case,
            "assessment",
            format!("{:?}", expectation.assessment),
            shown(native.assessment.map(|a| format!("{a:?}"))),
        );
        differ(
            &mut problems,
            &case,
            "disposition",
            Disposition::Passed.as_str(),
            native.disposition.as_str(),
        );
        differ(&mut problems, &case, "completion", true, native.complete);
        differ(&mut problems, &case, "errors", 0, native.errors);
        differ(&mut problems, &case, "warnings", 0, native.warnings);
        coverage(
            &mut problems,
            &case,
            &native.coverage,
            expectation.inventory,
            expectation.areas,
        );
        if problems.is_empty() {
            summary.matched += 1;
        }
        summary.mismatches.extend(problems);
    }
    for expectation in expected {
        let case = expectation.case;
        if !seen.contains(&case) {
            summary
                .mismatches
                .push(format!("{case}: missing model result"));
        }
    }
    summary
}

pub fn match_samples(expected: &[SampleExpectation], observed: &[SampleCheck]) -> NativeSummary {
    let mut native = NativeSummary {
        summary: Summary {
            expected: expected.len(),
            ..Summary::default()
        },
        ..NativeSummary::default()
    };
    let mismatches = &mut native.summary.mismatches;
    let mut expectations = BTreeMap::new();
    for expectation in expected {
        if expectations.insert(expectation.id, expectation).is_some() {
            mismatches.push(format!("{}: duplicate expectation", expectation.id));
        }
    }
    let mut seen = BTreeSet::new();
    for check in observed {
        let id = check.id;
        if !seen.insert(id) {
            mismatches.push(format!("{id}: duplicate sample result"));
            continue;
        }
        match &check.native {
            Ok(result) => match result.admission {
                Admission::Admitted => native.admitted += 1,
                Admission::Rejected => native.rejected += 1,
                _ => native.undetermined += 1,
            },
            Err(_) => native.unavailable += 1,
        }
        let Some(expectation) = expectations.get(&id) else {
            mismatches.push(format!("{id}: extra sample result without an expectation"));
            continue;
        };
        let problems = match &check.native {
            Ok(result) => sample_problems(expectation, result),
            Err(reason) => vec![format!("{id}: native result unavailable ({reason})")],
        };
        if problems.is_empty() {
            native.summary.matched += 1;
        }
        mismatches.extend(problems);
    }
    for id in expectations.keys() {
        if !seen.contains(id) {
            mismatches.push(format!("{id}: missing sample result"));
        }
    }
    native
}

fn sample_problems(expected: &SampleExpectation, observed: &NativeResult) -> Vec<String> {
    let id = &expected.id;
    let mut problems = Vec::new();
    if !observed.complete {
        problems.push(format!("{id}: unexpected incomplete result"));
    }
    differ(
        &mut problems,
        id,
        "selector kind",
        expected.kind.as_str(),
        observed.kind.as_str(),
    );
    differ(
        &mut problems,
        id,
        "admission",
        expected.admission.as_str(),
        observed.admission.as_str(),
    );
    let disposition = match expected.admission {
        Admission::Admitted => Disposition::Passed,
        Admission::Rejected => Disposition::Failed,
        _ => Disposition::Undetermined,
    };
    differ(
        &mut problems,
        id,
        "disposition",
        disposition.as_str(),
        observed.disposition.as_str(),
    );
    coverage(
        &mut problems,
        id,
        &observed.coverage,
        SAMPLE_INVENTORY,
        SAMPLE_AREAS,
    );
    if let Some(decisive) = &expected.decisive
        && !observed.errors.iter().any(|error| decides(decisive, error))
    {
        problems.push(format!(
            "{id}: missing decisive {} at sample pointer {:?} related to {}",
            decisive.code, decisive.sample_pointer, decisive.source_pointer
        ));
    }
    problems
}

fn decides(expected: &DecisiveDiagnostic, observed: &crate::native::Diagnostic) -> bool {
    observed.code == expected.code
        && observed.sample_pointer.as_deref() == Some(expected.sample_pointer)
        && observed
            .source_pointers
            .iter()
            .any(|pointer| pointer == expected.source_pointer)
}

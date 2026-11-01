//! The words the SARIF log and the Markdown changelog of a Revision comparison share for one
//! difference or version warning, following ADR 0043: SARIF and Markdown report projections.
use super::{Difference, DifferenceSubject, Revision, RevisionComparison, VersionWarning};
use crate::Field;

/// How a projection renders model text, such as Given Names, quality names, source keys and
/// witness values, and Esdieff's own explanatory text.
pub(crate) struct Style {
    pub(crate) code: fn(&str) -> String,
    pub(crate) text: fn(&str) -> String,
}

/// Plain text, as SARIF messages carry it.
pub(crate) const PLAIN: Style = Style {
    code: str::to_owned,
    text: str::to_owned,
};

/// What a difference changed and where, with its undetermined reason or Breaking witness:
/// for example ``changed quality `minimum` in `meter.sdf.json` at `/sdfObject/Meter/...` ``
/// in Markdown. The later revision's side names the place, or the earlier one's when only it
/// exists.
pub(crate) fn describe(
    comparison: &RevisionComparison<'_>,
    difference: &Difference<'_>,
    style: &Style,
) -> String {
    let code = style.code;
    let mut words = format!("{} ", difference.kind().as_str());
    match difference.subject() {
        DifferenceSubject::Quality(quality) => {
            words.push_str(&format!("quality {}", code(quality)))
        }
        DifferenceSubject::Entry => words.push_str(&format!("{} entry", code("properties"))),
        DifferenceSubject::Definition(class) => words.push_str(&code(class.keyword())),
        DifferenceSubject::Document => words.push_str(&code("document")),
    }
    let side = [Revision::Later, Revision::Earlier]
        .into_iter()
        .find_map(|revision| Some((revision, difference.side(revision)?)));
    if let Some((revision, side)) = side {
        let catalogue = comparison.catalogue(revision);
        let document = catalogue
            .source(side.root_document())
            .map_or("", |source| source.key());
        words.push_str(&format!(" in {}", code(document)));
        match side.pointer_with_limit(catalogue.limits.pointer_bytes as u32) {
            Field::Present(pointer) if pointer.is_empty() => {
                words.push_str(" at the document root")
            }
            Field::Present(pointer) => words.push_str(&format!(" at {}", code(&pointer))),
            _ => words.push_str(" at an omitted pointer"),
        }
    }
    if let Some(reason) = difference.reason() {
        words.push_str(&format!(
            ": {} ({})",
            (style.text)(reason.message()),
            code(reason.code())
        ));
    }
    if let Some(witness) = difference.witness() {
        words.push_str(&format!(
            " (witness {} for {} data, earlier {}, later {})",
            code(witness.value()),
            witness.direction().as_str().replace('_', "-"),
            witness.admission(Revision::Earlier).as_str(),
            witness.admission(Revision::Later).as_str(),
        ));
    }
    words
}

/// What a version warning found and where: for example ``version `1` in `lock.sdf.json` at
/// `/info/version` does not compare higher than earlier version `1`, after 2 breaking
/// differences (`comparison.version_not_increased`)`` in Markdown.
pub(crate) fn describe_warning(
    comparison: &RevisionComparison<'_>,
    warning: &VersionWarning<'_>,
    style: &Style,
) -> String {
    let code = style.code;
    let document = comparison
        .catalogue(Revision::Later)
        .source(warning.documents().1)
        .map_or("", |source| source.key());
    let place = match warning.pointer(Revision::Later) {
        "" => format!("in {} at the document root", code(document)),
        pointer => format!("in {} at {}", code(document), code(pointer)),
    };
    let earlier = code(warning.earlier_version());
    let breaking = breaking_differences(warning.breaking().into());
    let words = match warning.later_version() {
        Some(later) => format!(
            "version {} {place} does not compare higher than earlier version {earlier}, after {breaking}",
            code(later)
        ),
        None => {
            format!("version is absent {place}, after earlier version {earlier} and {breaking}")
        }
    };
    format!("{words} ({})", code(warning.code()))
}

/// A count of breaking differences, such as `1 breaking difference`.
pub(crate) fn breaking_differences(count: u64) -> String {
    match count {
        1 => "1 breaking difference".to_owned(),
        _ => format!("{count} breaking differences"),
    }
}

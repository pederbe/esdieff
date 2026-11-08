//! A changed `sdfType` follows the meaning-quality rule of ADR 0039: Client-side revision
//! comparison. `byte-string`, `unix-time` and the `link` of ADR 0060: Link sdfType draft
//! support tell a Client how to read a value, so a change between them is breaking in every
//! direction the data carries. A convention that is added or removed changes how values read
//! too, so its change has no Compatibility proof and is breaking only with a Breaking
//! witness: a value outside the convention's JSON representation, which Sample-value checking
//! rejects under it. Any other value is not classified.
use super::{Candidate, Descriptions, Relation, Rule};
use crate::EffectiveJson;
use crate::comparison::{CLIENT, Direction, Revision, THING, UndeterminedReason, Verdict};

/// Semantic types from RFC 9880 Section 4.7.1 with Esdieff's recorded JSON representations,
/// and the recorded link draft.
const CONVENTIONS: [&str; 3] = ["byte-string", "unix-time", "link"];

/// A value outside one convention's JSON representation.
#[derive(Clone, Copy)]
enum Outside {
    /// A string, or for `unix-time` a non-number.
    Text(&'static str),
    /// The map `{"X":""}`, outside a link's lowercase member names.
    UppercaseMember,
}

/// A change between two conventions is breaking. Any other change awaits `rule`.
pub(super) fn changed(members: [Option<EffectiveJson<'_>>; 2]) -> Verdict {
    match members.map(|json| json.and_then(|json| json.text().present())) {
        [Some(earlier), Some(later)]
            if earlier != later
                && [earlier, later]
                    .iter()
                    .all(|name| CONVENTIONS.contains(name)) =>
        {
            Verdict::Breaking
        }
        _ => Verdict::NOT_CLASSIFIED,
    }
}

/// A convention that is added or removed has a candidate the revision with it rejects. When
/// either revision has another value, the change has no candidates and is not classified.
pub(super) fn rule(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let mut rule = Rule::of(Relation::Unknown);
    // The revision with the convention rejects the candidate, and the other must admit it.
    let mut candidates = [None, None];
    for (candidate, (revision, direction)) in candidates.iter_mut().zip([
        (Revision::Earlier, Direction::ThingSupplied),
        (Revision::Later, Direction::ClientSupplied),
    ]) {
        let Some(json) = descriptions.member(revision, "sdfType")? else {
            continue;
        };
        let outside = match json.text().present() {
            // `!` is outside the base64url alphabet of a byte string.
            Some("byte-string") => Outside::Text("!"),
            // A Unix time is a number.
            Some("unix-time") => Outside::Text(""),
            // A link member name has its lowercase form.
            Some("link") => Outside::UppercaseMember,
            _ => {
                rule.unclassified = CLIENT | THING;
                return Ok(rule);
            }
        };
        *candidate = Some((outside, direction));
    }
    for (outside, direction) in candidates.into_iter().flatten() {
        let value = match outside {
            Outside::Text(text) => descriptions.text(text)?,
            Outside::UppercaseMember => descriptions.uppercase_member()?,
        };
        if let Some(value) = value {
            descriptions.add(&mut rule, Candidate { value, direction })?;
        }
    }
    Ok(rule)
}

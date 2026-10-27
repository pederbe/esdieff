//! Whether two Content-format declarations have the same established meaning, for a
//! Revision comparison. Follows ADR 0033: Content-format model meaning and the meaning
//! qualities of ADR 0039: Client-side revision comparison.
//!
//! A declaration's meaning is its media type and subtype without ASCII case, its parameter
//! meaning under the supported media rules, and its codings in order. A numeric identifier
//! means its complete recorded CoAP tuple, read through the same media rules where they
//! apply. Only numeric identifiers and codings consult recorded evidence, under the request's
//! recorded registry policy. Classification, media rules and compared rows cost what they
//! cost in model checking, and the final comparison is charged too, all on the operation
//! allowance.
use super::lookup::{self, CODINGS, DEPRECATED, NUMERIC, Names};
use super::{grammar, meaning, table};
use crate::allowance::charge;
use crate::registry::{CONTENT_FORMAT_MEANING, State};

/// Why two declarations' meanings could not be compared.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Unsettled {
    /// A declaration has no established meaning.
    Unestablished,
    /// The request withholds the recorded evidence a declaration needs.
    Withheld,
    /// The operation allowance ran out.
    Limited,
}

/// Whether two declarations mean the same. Both spellings and their media rules are read
/// first, so a declaration without meaning on its own spelling needs no recorded evidence.
/// Then the earlier declaration's evidence is consulted before the later one's.
pub(crate) fn same_meaning(
    texts: [&str; 2],
    registry: &mut State,
    work: &mut usize,
) -> Result<bool, Unsettled> {
    let earlier = spelled(texts[0], registry, work)?;
    let later = spelled(texts[1], registry, work)?;
    let earlier = earlier.establish(registry, work)?;
    let later = later.establish(registry, work)?;
    same(&earlier, &later, work)
}

/// The established meaning of one declaration.
struct Meaning<'a> {
    name: &'a str,
    parameters: Parameters<'a>,
    /// Authored coding tokens, each after an `@`.
    codings: &'a str,
    /// The coding of a recorded tuple.
    recorded: Option<&'static str>,
}
impl<'a> Meaning<'a> {
    fn codings(&self) -> impl Iterator<Item = &'a str> {
        self.codings.split('@').skip(1).chain(self.recorded)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Parameters<'a> {
    /// Parameters the supported media rules establish.
    Supported(meaning::Parameters),
    /// The registered parameters of a recorded tuple outside those rules.
    Recorded(&'a str),
}

/// A declaration whose spelling is read: a media name the supported rules establish, whose
/// codings are still to be looked up, or a numeric identifier.
enum Spelled<'a> {
    Media(Meaning<'a>),
    Number { number: u16, bytes: usize },
}
impl<'a> Spelled<'a> {
    fn establish(self, registry: &mut State, work: &mut usize) -> Result<Meaning<'a>, Unsettled> {
        let (number, bytes) = match self {
            Self::Media(meaning) => {
                for coding in meaning.codings() {
                    // A deprecated coding such as `x-gzip` is equivalent to another under
                    // RFC 9110 Section 8.4.1, so its spelling alone does not settle meaning.
                    if coding_flags(coding, registry, work)?
                        .is_none_or(|flags| flags & DEPRECATED != 0)
                    {
                        return Err(Unsettled::Unestablished);
                    }
                }
                return Ok(meaning);
            }
            Self::Number { number, bytes } => (number, bytes),
        };
        let row = numeric(number, bytes, registry, work)?.ok_or(Unsettled::Unestablished)?;
        // The canonical tuple's complete static fields fit 128 bytes, and its content type
        // is classified like an authored declaration.
        if !charge(work, 129 + 32 * row.content_type.len() + 1) {
            return Err(Unsettled::Limited);
        }
        let Some(grammar::Declaration::Media {
            name, parameters, ..
        }) = grammar::classify(row.content_type).filter(|_| meaning::numeric(row))
        else {
            return Err(Unsettled::Unestablished);
        };
        let supported = rule(name, parameters, registry, work)?;
        Ok(Meaning {
            name,
            parameters: supported.map_or(Parameters::Recorded(parameters), Parameters::Supported),
            codings: "",
            recorded: row.coding,
        })
    }
}

/// Read one declaration's spelling and, for a media name, its supported media rule.
fn spelled<'a>(
    text: &'a str,
    registry: &mut State,
    work: &mut usize,
) -> Result<Spelled<'a>, Unsettled> {
    let cost = text.len().checked_mul(32).and_then(|n| n.checked_add(1));
    if !cost.is_some_and(|cost| charge(work, cost)) {
        return Err(Unsettled::Limited);
    }
    match grammar::classify(text) {
        Some(grammar::Declaration::Number(Some(number))) => Ok(Spelled::Number {
            number,
            bytes: text.len(),
        }),
        Some(grammar::Declaration::Media {
            name,
            codings,
            parameters,
        }) => {
            let parameters =
                rule(name, parameters, registry, work)?.ok_or(Unsettled::Unestablished)?;
            Ok(Spelled::Media(Meaning {
                name,
                parameters: Parameters::Supported(parameters),
                codings,
                recorded: None,
            }))
        }
        // Invalid spelling and identifiers out of range have no meaning.
        _ => Err(Unsettled::Unestablished),
    }
}

/// Consult the supported media rules for a media name and its parameters.
fn rule(
    name: &str,
    parameters: &str,
    registry: &mut State,
    work: &mut usize,
) -> Result<Option<meaning::Parameters>, Unsettled> {
    let cost = name
        .len()
        .checked_add(parameters.len())
        .and_then(|n| n.checked_mul(8))
        .and_then(|n| n.checked_add(64));
    if !cost.is_some_and(|cost| charge(work, cost)) {
        return Err(Unsettled::Limited);
    }
    registry.interpretations |= CONTENT_FORMAT_MEANING;
    Ok(meaning::established(name, parameters))
}

/// Whether two established meanings are the same. Recorded tuples are distinct
/// registrations, so a tuple outside the supported rules differs from any declaration with
/// another media name or other parameters.
fn same(earlier: &Meaning<'_>, later: &Meaning<'_>, work: &mut usize) -> Result<bool, Unsettled> {
    let bytes = [earlier, later].map(|meaning| {
        meaning.name.len() + meaning.codings.len() + meaning.recorded.map_or(0, str::len)
    });
    if !charge(work, bytes[0] + bytes[1] + 1) {
        return Err(Unsettled::Limited);
    }
    if !earlier.name.eq_ignore_ascii_case(later.name) {
        return Ok(false);
    }
    let parameters = match (earlier.parameters, later.parameters) {
        (Parameters::Supported(a), Parameters::Supported(b)) => a == b,
        (Parameters::Recorded(a), Parameters::Recorded(b)) => a == b,
        // One media name with parameters both inside and outside the supported rules.
        _ => return Err(Unsettled::Unestablished),
    };
    let (mut a, mut b) = (earlier.codings(), later.codings());
    let codings = loop {
        match (a.next(), b.next()) {
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => {}
            (None, None) => break true,
            _ => break false,
        }
    };
    Ok(parameters && codings)
}

/// The recorded CoAP tuple with this identifier, whose spelling is `bytes` long.
fn numeric(
    number: u16,
    bytes: usize,
    registry: &mut State,
    work: &mut usize,
) -> Result<Option<table::Numeric>, Unsettled> {
    if registry.policy == crate::RecordedRegistryPolicy::WithholdRecorded {
        registry.withheld |= NUMERIC;
        return Err(Unsettled::Withheld);
    }
    lookup::numeric(number, bytes, registry, work).map_err(|()| Unsettled::Limited)
}

/// The recorded flags of a positive HTTP content coding, compared without ASCII case, or
/// `None` without a recorded positive, as for the reserved `identity`.
fn coding_flags(
    coding: &str,
    registry: &mut State,
    work: &mut usize,
) -> Result<Option<u8>, Unsettled> {
    if registry.policy == crate::RecordedRegistryPolicy::WithholdRecorded {
        registry.withheld |= CODINGS;
        return Err(Unsettled::Withheld);
    }
    lookup::named(coding, Names::Codings, registry, work).map_err(|()| Unsettled::Limited)
}

//! Classify each difference from the Client's side, following ADR 0039: Client-side
//! revision comparison.
//!
//! Every changed quality reaches `quality` with its context and its member in each
//! revision. Documentary qualities are compatible. A quality without its own rule is
//! undetermined because it is not classified yet, so a change Esdieff cannot classify is
//! never presented as safe. Each later rule adds one arm to `quality` for its Quality Name
//! or context.
//!
//! A changed data `unit` instead reaches `unit`, which looks up both names in the recorded
//! SenML evidence, and a changed data `contentFormat` reaches `content_format`, which
//! compares the established meanings of both declarations.
//!
//! A data quality that constrains admitted values has a monotone rule in `rule`: how the
//! later revision's admitted values relate to the earlier's, and the candidate Breaking
//! witnesses the change itself suggests. A rule adds representative values of the revision
//! that must admit a candidate through `Descriptions::representatives`, and every change has
//! at most `CANDIDATES` candidates. `witness` settles each change together with the other
//! changed qualities of its description. A new classified data quality adds one arm to
//! `rule` and a module beside `bounds`. Beside a choice, `choice` judges a rule on the
//! alternatives that inherit the changed quality, and `meaning` judges a meaning quality alike.
//! A map `properties` entry that only one revision describes reaches `entry` instead.
use super::{
    CLIENT, ContentFormatReason, DifferenceKind, Direction, Perspective, Revision, THING,
    UndeterminedReason, UnitReason, Verdict, read,
};
use crate::allowance::{OperationBudget, push};
use crate::content_format::Unsettled;
use crate::representative::{self, Exhausted};
use crate::sample::ValueAllowance;
use crate::{EffectiveJson, ExactNumber, JsonBuilder, JsonValue};
use std::cmp::Ordering;

mod array;
mod bounds;
mod choice;
mod counts;
mod entry;
mod enumeration;
mod instances;
mod multiple;
mod nullable;
mod required;
mod restriction;
mod sdf_type;
mod types;
pub(super) use array::enclosing_items;
pub(super) use choice::{Choices, alternative, choices, inherited};

/// The Client directions of a data description, among those both revisions offer.
#[derive(Clone, Copy)]
pub(super) struct DataDirections {
    pub(super) directions: u8,
    /// Why a Property characteristic needed for the directions is unavailable.
    pub(super) unknown: Option<UndeterminedReason>,
}
impl DataDirections {
    pub(super) fn fixed(directions: u8) -> Self {
        Self {
            directions,
            unknown: None,
        }
    }
}

/// Where a changed quality sits.
#[derive(Clone, Copy)]
pub(super) enum Context {
    /// A quality of an sdfThing or sdfObject.
    Grouping,
    /// A quality of an Action or Event.
    Affordance,
    /// An Action's input data as a whole, when the revisions do not both have it as a map.
    Input,
    /// An Action's or Event's output data as a whole, when the revisions do not both have it
    /// as a map.
    Output,
    /// A Property's readable, writable or observable characteristic.
    Characteristic,
    /// A quality of Property data, Action input or output, Event output, or one of their
    /// map `properties` entries.
    Data(DataDirections),
}

/// A removed grouping or affordance breaks its Clients, and an added one does not.
pub(super) fn definition(kind: DifferenceKind) -> Verdict {
    match kind {
        DifferenceKind::Removed => Verdict::Breaking,
        _ => Verdict::Compatible,
    }
}

/// The monotone rule of a map `properties` entry `name` that only one revision describes, in
/// map data whose corresponding descriptions are `owners`, following ADR 0039: Client-side
/// revision comparison. `None` when no Client of the earlier revision uses the entry, which is
/// then compatible in every direction. A used entry is settled with the rest of its map, in
/// the directions the data carries.
pub(super) fn entry(
    owners: [EffectiveJson<'_>; 2],
    name: &str,
    kind: DifferenceKind,
    data: DataDirections,
    perspective: Perspective,
    allowance: &mut ValueAllowance,
) -> Option<Result<Rule, UndeterminedReason>> {
    let revision = match kind {
        DifferenceKind::Removed => Revision::Earlier,
        _ => Revision::Later,
    };
    let mut descriptions = Descriptions::new(owners, data.directions, allowance);
    // Implementations must accept all admitted inputs, including optional entries
    // newly unconstrained by an open map. The client-only usage filter is not a
    // proof about implementation obligations.
    if perspective == Perspective::Client {
        match entry::used(&mut descriptions, name, revision) {
            Ok(false) => return None,
            Ok(true) => {}
            Err(reason) => return Some(Err(reason)),
        }
    }
    Some(match data.unknown {
        Some(reason) => Err(reason),
        None => entry::rule(&mut descriptions, name, revision),
    })
}

/// Classify a changed quality from its context and its member in each revision, absent where
/// that revision lacks it.
pub(super) fn quality(
    quality: &str,
    context: Context,
    members: [Option<EffectiveJson<'_>>; 2],
) -> Verdict {
    if documentary(quality, context) {
        return Verdict::Compatible;
    }
    if let Context::Characteristic = context {
        return characteristic(members);
    }
    if let Context::Input = context {
        return input(members);
    }
    if let Context::Output = context {
        return output(members);
    }
    if let Context::Data(DataDirections {
        unknown: Some(reason),
        ..
    }) = context
    {
        return Verdict::Undetermined(reason);
    }
    if let Context::Data(_) = context
        && quality == "sdfType"
    {
        return sdf_type::changed(members);
    }
    Verdict::NOT_CLASSIFIED
}

/// A changed `unit` in data, following the unit rule of ADR 0039: Client-side revision
/// comparison. Each name is absent (`None`), text, or another value. A change between two
/// different current names in the recorded SenML evidence, from either its SenML or its
/// Secondary Units section, makes Clients misread values in every direction the data
/// carries, so it is breaking. An added or removed unit, a URI name, an unrecorded spelling
/// or a legacy name leaves the change undetermined, in that order of precedence. Only
/// names that need recorded evidence are looked up: `lookup` returns a positive's legacy
/// flag, `None` for an unrecorded spelling, or why the lookup is unfinished.
pub(super) fn unit(
    names: [Option<Option<&str>>; 2],
    mut lookup: impl FnMut(&str) -> Result<Option<bool>, UndeterminedReason>,
) -> Verdict {
    let unit = |reason| Verdict::Undetermined(UndeterminedReason::Unit(reason));
    let names = match names {
        [None, _] => return unit(UnitReason::Added),
        [_, None] => return unit(UnitReason::Removed),
        [Some(Some(earlier)), Some(Some(later))] => [earlier, later],
        _ => return Verdict::NOT_CLASSIFIED,
    };
    // RFC 9880 Section 4.7 makes every name containing a colon a URI name.
    if names.iter().any(|name| name.contains(':')) {
        return unit(UnitReason::UriName);
    }
    let mut legacy = false;
    for name in names {
        match lookup(name) {
            Ok(Some(flag)) => legacy |= flag,
            Ok(None) => return unit(UnitReason::UnrecordedSpelling),
            Err(reason) => return Verdict::Undetermined(reason),
        }
    }
    if legacy {
        unit(UnitReason::LegacyName)
    } else {
        Verdict::Breaking
    }
}

/// The mandatory declarations of corresponding groupings, following the requiredness rule of
/// ADR 0039: Client-side revision comparison and ADR 0054: Implementer declaration
/// requiredness, or `None` when they did not change. A declaration of both revisions that
/// stops being mandatory (`dropped`) breaks Clients relying on it. One that becomes
/// mandatory (`entered`) can require an existing implementation to change. A partial or
/// unavailable `resolution` leaves such a change undetermined, and also a changed
/// `sdfRequired` that `changed` reports, since what it resolves to is unknown.
pub(super) fn requiredness<E>(
    perspective: Perspective,
    dropped: bool,
    entered: bool,
    resolution: Result<(), UndeterminedReason>,
    changed: impl FnOnce() -> Result<bool, E>,
) -> Result<Option<Verdict>, E> {
    Ok(Some(match resolution {
        Ok(())
            if match perspective {
                Perspective::Client => dropped,
                Perspective::Implementer => entered,
            } =>
        {
            Verdict::Breaking
        }
        Ok(()) if dropped || entered => Verdict::Compatible,
        Ok(()) => return Ok(None),
        Err(reason) if dropped || entered || changed()? => Verdict::Undetermined(reason),
        Err(_) => return Ok(None),
    }))
}

/// A changed `minItems` or `maxItems` of corresponding groupings, which bound the instance
/// count of a Grouping array, with every other quality as it is.
pub(super) fn instances(quality: &str, groupings: &mut Descriptions<'_, '_>) -> Verdict {
    let end = match quality {
        "minItems" => bounds::End::Lower,
        _ => bounds::End::Upper,
    };
    instances::verdict(end, groupings).unwrap_or_else(Verdict::Undetermined)
}

/// A changed `contentFormat` in data, following the meaning-quality rule of ADR 0039:
/// Client-side revision comparison, or `None` when both declarations have one established
/// meaning, so that nothing changed. Each declaration is absent (`None`), text, or another
/// value. Two different established meanings make Clients misread values in every
/// direction the data carries, so the change is breaking. An added or removed declaration,
/// or one without established meaning, leaves it undetermined. `same` compares the
/// established meanings of two declarations under ADR 0033: Content-format model meaning.
pub(super) fn content_format(
    declarations: [Option<Option<&str>>; 2],
    same: impl FnOnce([&str; 2]) -> Result<bool, Unsettled>,
) -> Option<Verdict> {
    let content_format = |reason| {
        Some(Verdict::Undetermined(UndeterminedReason::ContentFormat(
            reason,
        )))
    };
    let texts = match declarations {
        [None, _] => return content_format(ContentFormatReason::Added),
        [_, None] => return content_format(ContentFormatReason::Removed),
        [Some(Some(earlier)), Some(Some(later))] => [earlier, later],
        _ => return Some(Verdict::NOT_CLASSIFIED),
    };
    match same(texts) {
        Ok(true) => None,
        Ok(false) => Some(Verdict::Breaking),
        Err(Unsettled::Unestablished) => content_format(ContentFormatReason::Unestablished),
        Err(Unsettled::Withheld) => content_format(ContentFormatReason::EvidenceWithheld),
        Err(Unsettled::Limited) => Some(Verdict::Undetermined(UndeterminedReason::limit(
            "comparison work limit",
        ))),
    }
}

/// The verdict of a changed meaning quality, `unit`, `contentFormat` or `sdfType`, of data
/// whose corresponding descriptions are `pair`, given its `verdict` at those descriptions.
/// Beside a choice the quality tells a Client how to read only the values of the alternatives
/// that inherit it, which read it as the description does, so it changes nothing when every
/// alternative replaces it. When a revision's alternatives are not established, a breaking
/// verdict cannot be shown and the change is not classified. Other qualities keep `verdict`.
pub(super) fn meaning(
    quality: &str,
    verdict: Verdict,
    pair: [EffectiveJson<'_>; 2],
    budget: &mut OperationBudget,
) -> Verdict {
    if !matches!(quality, "unit" | "contentFormat" | "sdfType") {
        return verdict;
    }
    match choice::reaches(pair, quality, budget) {
        Ok(Some(true)) => verdict,
        Ok(Some(false)) => Verdict::Compatible,
        Ok(None) if verdict == Verdict::Breaking => Verdict::NOT_CLASSIFIED,
        Ok(None) => verdict,
        Err(_) if matches!(verdict, Verdict::Undetermined(_)) => verdict,
        Err(reason) => Verdict::Undetermined(reason),
    }
}

/// Changes to `description`, `label`, `$comment` and documentary `default` never affect an
/// interaction.
fn documentary(quality: &str, context: Context) -> bool {
    matches!(quality, "description" | "label" | "$comment")
        || quality == "default" && matches!(context, Context::Data(_))
}

/// A Property characteristic that becomes false breaks the interaction it allowed, and one
/// that becomes true offers another. An absent characteristic takes the `true` of its
/// Specified default characteristic. A value that is not a boolean is not classified.
fn characteristic(members: [Option<EffectiveJson<'_>>; 2]) -> Verdict {
    let value = |member: Option<EffectiveJson<'_>>| {
        member.map_or(Some(true), |json| json.boolean().present())
    };
    match members.map(value) {
        [Some(true), Some(false)] => Verdict::Breaking,
        [Some(false), Some(true)] => Verdict::Compatible,
        _ => Verdict::NOT_CLASSIFIED,
    }
}

/// Input data added to an Action that had none breaks existing invocations, which send none,
/// and removed input data breaks those that send it.
fn input(members: [Option<EffectiveJson<'_>>; 2]) -> Verdict {
    match members {
        [None, Some(_)] | [Some(_), None] => Verdict::Breaking,
        _ => Verdict::NOT_CLASSIFIED,
    }
}

/// Removed output data breaks Clients that read it. Clients use only what the earlier revision
/// describes, so output data added to an affordance that had none reaches no Client of it.
fn output(members: [Option<EffectiveJson<'_>>; 2]) -> Verdict {
    match members {
        [Some(_), None] => Verdict::Breaking,
        [None, Some(_)] => Verdict::Compatible,
        _ => Verdict::NOT_CLASSIFIED,
    }
}

/// How the later revision's admitted values relate to the earlier revision's under one
/// changed quality, with every other quality as it is.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Relation {
    /// Both admit the same values, as when a redundant bound changes.
    Same,
    /// The later revision admits every value the earlier one admits.
    Wider,
    /// The later revision admits only values the earlier one admits.
    Narrower,
    /// Neither is established.
    Unknown,
}
impl Relation {
    /// Whether the change loosens in the safe direction for each of `directions`:
    /// Client-supplied data may only widen and Thing-supplied data only narrow.
    pub(super) fn safe(self, directions: u8) -> bool {
        let client = directions & CLIENT == 0 || matches!(self, Self::Same | Self::Wider);
        let thing = directions & THING == 0 || matches!(self, Self::Same | Self::Narrower);
        client && thing
    }
}

/// A value that may separate the revisions, and the direction it breaks when it does:
/// Client-supplied data when the earlier revision admits it and the later rejects it, and
/// Thing-supplied data for the reverse.
pub(super) struct Candidate {
    pub(super) value: JsonValue,
    pub(super) direction: Direction,
}

/// What a monotone rule establishes about one changed quality.
pub(super) struct Rule {
    pub(super) relation: Relation,
    pub(super) candidates: Vec<Candidate>,
    /// Directions in which the rule offers no candidates for this change yet, so that the
    /// change is not classified there rather than unproven.
    pub(super) unclassified: u8,
}
impl Rule {
    fn of(relation: Relation) -> Self {
        Self {
            relation,
            candidates: Vec::new(),
            unclassified: 0,
        }
    }
}

/// The most candidate witnesses one change may have, which bounds what settling a change
/// costs on the one operation allowance.
const CANDIDATES: usize = 8;

/// A monotone rule, reading the corresponding descriptions of a changed quality.
pub(super) type RuleFn = fn(&mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason>;

/// The monotone rule for a changed data quality, or `None` while the quality has none.
pub(super) fn rule(quality: &str) -> Option<RuleFn> {
    let rule: RuleFn = match quality {
        "minimum" | "exclusiveMinimum" => {
            |descriptions| bounds::rule(bounds::End::Lower, descriptions)
        }
        "maximum" | "exclusiveMaximum" => {
            |descriptions| bounds::rule(bounds::End::Upper, descriptions)
        }
        "minLength" => |descriptions| counts::length(bounds::End::Lower, descriptions),
        "maxLength" => |descriptions| counts::length(bounds::End::Upper, descriptions),
        "multipleOf" => multiple::rule,
        "const" => |descriptions| restriction::rule("const", descriptions),
        "pattern" => |descriptions| restriction::rule("pattern", descriptions),
        "format" => restriction::format,
        "enum" => enumeration::rule,
        "type" => types::rule,
        "nullable" => nullable::rule,
        "required" => required::rule,
        "sdfType" => sdf_type::rule,
        "minItems" => |descriptions| counts::items(bounds::End::Lower, descriptions),
        "maxItems" => |descriptions| counts::items(bounds::End::Upper, descriptions),
        "uniqueItems" => array::unique,
        "items" => array::items,
        "sdfChoice" => choice::rule,
        _ => return None,
    };
    Some(rule)
}

/// What the monotone rule of a changed data quality establishes, or `None` while the quality
/// has none. Beside a choice, the rule is judged on the alternatives that inherit the quality.
pub(super) fn ruled(
    quality: &str,
    descriptions: &mut Descriptions<'_, '_>,
) -> Option<Result<Rule, UndeterminedReason>> {
    rule(quality).map(|rule| choice::judged(rule, quality, descriptions))
}

/// What a rule reads: the corresponding descriptions of both revisions, carrying data in
/// `directions`, with lookups, numeric work and candidates charged to the one operation
/// allowance.
pub(super) struct Descriptions<'s, 'c> {
    descriptions: [EffectiveJson<'c>; 2],
    directions: u8,
    allowance: &'s mut ValueAllowance,
}
impl<'s, 'c> Descriptions<'s, 'c> {
    pub(super) fn new(
        descriptions: [EffectiveJson<'c>; 2],
        directions: u8,
        allowance: &'s mut ValueAllowance,
    ) -> Self {
        Self {
            descriptions,
            directions,
            allowance,
        }
    }
    /// One revision's description of the changed quality.
    fn description(&self, revision: Revision) -> EffectiveJson<'c> {
        self.descriptions[revision.index()]
    }
    /// The member of one revision's description with this name.
    fn member(
        &mut self,
        revision: Revision,
        key: &str,
    ) -> Result<Option<EffectiveJson<'c>>, UndeterminedReason> {
        read::member(
            self.descriptions[revision.index()],
            key,
            &mut self.allowance.budget,
        )
    }
    /// Whether `json` is an available array of decoded strings.
    fn strings(&mut self, json: EffectiveJson<'_>) -> Result<bool, UndeterminedReason> {
        read::strings(json, &mut self.allowance.budget)
    }
    /// Whether `text` is among the decoded strings of `values`.
    fn contains(
        &mut self,
        values: Option<EffectiveJson<'_>>,
        text: &str,
    ) -> Result<bool, UndeterminedReason> {
        read::contains(values, text, &mut self.allowance.budget)
    }
    /// Whether every decoded string of `values` is among those of `others`.
    fn within(
        &mut self,
        values: Option<EffectiveJson<'_>>,
        others: Option<EffectiveJson<'_>>,
    ) -> Result<bool, UndeterminedReason> {
        read::within(values, others, &mut self.allowance.budget)
    }
    fn compare(
        &mut self,
        a: ExactNumber<'_>,
        b: ExactNumber<'_>,
    ) -> Result<Ordering, UndeterminedReason> {
        self.allowance
            .compare(a, b)
            .map_err(UndeterminedReason::limit)
    }
    /// Whether `value` is an exact integer multiple of `step`.
    fn multiple(
        &mut self,
        value: ExactNumber<'_>,
        step: ExactNumber<'_>,
    ) -> Result<bool, UndeterminedReason> {
        self.allowance
            .multiple(value, step)
            .map_err(UndeterminedReason::limit)
    }
    /// Both revisions share one `Limits` configuration.
    fn limits(&self) -> &'c crate::Limits {
        &self.descriptions[0].model.catalogue.limits
    }
    fn coefficient_digits(&self) -> usize {
        self.limits().coefficient
    }
    /// A number with this lexeme, or `None` when the lexeme exceeds the number capabilities.
    fn number(&mut self, lexeme: &str) -> Result<Option<JsonValue>, UndeterminedReason> {
        self.value(lexeme.len(), |builder| builder.number(lexeme))
    }
    /// A string with this text, or `None` when the text exceeds the value capabilities.
    fn text(&mut self, text: &str) -> Result<Option<JsonValue>, UndeterminedReason> {
        self.value(text.len(), |builder| builder.text(text))
    }
    fn null(&mut self) -> Result<Option<JsonValue>, UndeterminedReason> {
        self.value(4, JsonBuilder::null)
    }
    /// An array of `length` copies of the first representative value of one revision's
    /// `items`, or of `null` without an `items` description, or `None` when that item has no
    /// representative value or the array exceeds the value capabilities.
    fn items(
        &mut self,
        revision: Revision,
        length: usize,
    ) -> Result<Option<JsonValue>, UndeterminedReason> {
        representative::items(self.description(revision), length, self.allowance)
            .map_err(generating)
    }
    /// The value `build` makes from about `size` bytes, charged as the generated value it
    /// becomes, or `None` when it exceeds the value capabilities.
    fn value(
        &mut self,
        size: usize,
        build: impl FnOnce(&JsonBuilder) -> Result<JsonValue, &'static str>,
    ) -> Result<Option<JsonValue>, UndeterminedReason> {
        let Ok(builder) = JsonBuilder::with_limits(self.limits().clone()) else {
            return Ok(None);
        };
        representative::built(&builder, &mut self.allowance.budget, size, build).map_err(
            |exhausted| {
                UndeterminedReason::limit(match exhausted {
                    Exhausted::Work => "comparison work limit",
                    Exhausted::Allocation => "comparison allocation limit",
                })
            },
        )
    }
    /// How many more candidates a change may have in `direction`: none in a direction its
    /// data does not carry, and none beyond the per-change cap.
    fn room(&self, rule: &Rule, direction: Direction) -> usize {
        if self.directions & direction.bit() == 0 {
            return 0;
        }
        CANDIDATES.saturating_sub(rule.candidates.len())
    }
    /// Add a candidate while the change has room for it.
    fn add(&mut self, rule: &mut Rule, candidate: Candidate) -> Result<(), UndeterminedReason> {
        if self.room(rule, candidate.direction) == 0 {
            return Ok(());
        }
        push(
            &mut rule.candidates,
            candidate,
            &mut self.allowance.budget.allocation,
        )
        .map_err(|_| UndeterminedReason::limit("comparison allocation limit"))
    }
    /// Add representative values of the revision that must admit a candidate in `direction`
    /// as candidates there, while the change has room for them: the earlier revision's for
    /// Client-supplied data and the later revision's for Thing-supplied data. Each value
    /// stands at the position of the description under comparison.
    pub(super) fn representatives(
        &mut self,
        rule: &mut Rule,
        direction: Direction,
    ) -> Result<(), UndeterminedReason> {
        let room = self.room(rule, direction);
        if room == 0 {
            return Ok(());
        }
        let description = self.description(direction.admitting());
        for (_, value) in
            representative::values(description, room, self.allowance).map_err(generating)?
        {
            self.add(rule, Candidate { value, direction })?;
        }
        Ok(())
    }
}

/// Generating representative values ran out of the operation allowance, which leaves the
/// difference undetermined with `operation.limit`.
pub(super) fn generating(_: Exhausted) -> UndeterminedReason {
    UndeterminedReason::limit("representative value generation exhausted the operation allowance")
}

/// The map that both revisions' corresponding `owners` admit around `value` at their entry
/// `name`, when the `admitting` revision admits `value` there: `value` beside a
/// representative value of that revision's description of every other entry that either
/// revision requires. `None` when one of them has none, the map exceeds the value
/// capabilities, or the revisions describe a filled entry differently apart from its
/// documentation, since the other revision could then reject the map for that difference
/// alone.
pub(in crate::comparison) fn enclosing(
    owners: [EffectiveJson<'_>; 2],
    admitting: Revision,
    name: &str,
    value: JsonValue,
    allowance: &mut ValueAllowance,
) -> Result<Option<JsonValue>, UndeterminedReason> {
    let (owner, other) = (owners[admitting.index()], owners[admitting.other().index()]);
    let mut same = |name: &str, allowance: &mut ValueAllowance| {
        read::same_entry([owner, other], name, allowance).map_err(|_| Exhausted::Work)
    };
    representative::enclosing(owner, name, value, Some((other, &mut same)), allowance)
        .map_err(generating)
}

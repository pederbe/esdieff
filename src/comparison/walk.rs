//! Pair Corresponding definitions and record their differences.
//!
//! Top-level groupings correspond by their position in corresponding documents, and nested
//! groupings, affordances, map `properties` entries and choice alternatives by Given Name.
//! An array's `items` corresponds to the other revision's `items` as the nested description
//! of its values. Top-level affordances and `sdfData` definitions are not interfaces: their
//! changes appear through the groupings that use them. The walk uses an explicit task stack,
//! because composed content can nest deeper than its sources, and charges every visit to the
//! one operation allowance.
use super::classify::{self, Context, DataDirections};
use super::read::{self, Equality, identity_unavailable, unavailable_reason};
use super::requiredness::Requiredness;
use super::witness::{self, Batch, Change};
use super::{
    CLIENT, Classification, DifferenceKind, DifferenceSubject, Perspective, Presence, Record,
    Revision, Side, THING, UndeterminedReason, Verdict, failure,
};
use crate::allowance::push;
use crate::role::{Role, SetAside};
use crate::sample::{ValueAllowance, ValueInterpretations};
use crate::{Class, EffectiveJson, EffectiveModel, Field, JsonKind, SourceId};

/// A document of one revision: its source and its effective model, or why it has none.
#[derive(Clone, Copy)]
pub(super) struct Root {
    pub(super) id: SourceId,
    pub(super) model: Result<usize, UndeterminedReason>,
}

/// What the walks of earlier documents found, carried into the walk of the next.
pub(super) struct Outcome<'a> {
    pub(super) perspective: Perspective,
    pub(super) records: Vec<Record<'a>>,
    pub(super) counts: [u32; 3],
    pub(super) unretained: u32,
    /// The recorded evidence policy for unit names and content formats, and the sections
    /// consulted.
    pub(super) registry: crate::registry::State,
    /// The value interpretations that Breaking witnesses relied on.
    pub(super) interpretations: ValueInterpretations,
}
impl Outcome<'_> {
    pub(super) fn new(policy: crate::RecordedRegistryPolicy, perspective: Perspective) -> Self {
        Self {
            perspective,
            records: Vec::new(),
            counts: [0; 3],
            unretained: 0,
            registry: crate::registry::State {
                policy,
                ..Default::default()
            },
            interpretations: ValueInterpretations::default(),
        }
    }
}

/// The allowance ran out, so the rest of the document is not compared.
struct Stop(UndeterminedReason);
impl From<UndeterminedReason> for Stop {
    fn from(reason: UndeterminedReason) -> Self {
        Self(reason)
    }
}
type Step<T> = Result<T, Stop>;

#[derive(Clone, Copy)]
enum Task<'c> {
    /// Corresponding groupings or affordances of one class.
    Definition { class: Class, pair: Pair<'c> },
    /// Corresponding data descriptions carrying data in the given directions, inside the
    /// corresponding root descriptions that Sample-value checking selects.
    Data {
        root: Pair<'c>,
        pair: Pair<'c>,
        data: DataDirections,
    },
    /// Corresponding alternatives of the choices of corresponding data descriptions inside
    /// `root`, compared apart from the qualities both inherit from their description.
    Alternative {
        root: Pair<'c>,
        pair: Pair<'c>,
        data: DataDirections,
    },
}
type Pair<'c> = [EffectiveJson<'c>; 2];
/// The members of the same name in each revision, when present.
type Members<'c> = [Option<EffectiveJson<'c>>; 2];

pub(super) struct Walk<'c, 'a> {
    models: &'c [EffectiveModel<'a>],
    /// The requiredness of each model's groupings.
    required: &'c [Requiredness],
    /// The one operation allowance, which witness checks share.
    allowance: &'c mut ValueAllowance,
    outcome: Outcome<'a>,
    /// The document of each revision under comparison.
    roots: [Option<Root>; 2],
    /// Extension vocabulary of the descriptions whose qualities are being compared.
    aside: SetAside,
    /// The changed qualities of the data description being compared.
    batch: Batch<'a>,
}
impl<'c, 'a> Walk<'c, 'a> {
    pub(super) fn new(
        models: &'c [EffectiveModel<'a>],
        required: &'c [Requiredness],
        allowance: &'c mut ValueAllowance,
        outcome: Outcome<'a>,
    ) -> Self {
        Self {
            models,
            required,
            allowance,
            outcome,
            roots: [None, None],
            aside: SetAside::default(),
            batch: Batch::default(),
        }
    }
    pub(super) fn finish(self) -> Outcome<'a> {
        self.outcome
    }

    /// Compare the groupings of two corresponding documents.
    pub(super) fn document(&mut self, earlier: Root, later: Root) {
        self.roots = [Some(earlier), Some(later)];
        let roots = [
            self.root_json(Revision::Earlier),
            self.root_json(Revision::Later),
        ];
        let pair = match roots {
            [Ok(earlier), Ok(later)] => [earlier, later],
            [Err(reason), _] | [_, Err(reason)] => {
                let sides = [
                    self.root_side(Revision::Earlier, &roots[0]),
                    self.root_side(Revision::Later, &roots[1]),
                ];
                let reason = match roots {
                    [Ok(json), Err(_)] => {
                        self.against_unavailable(Revision::Earlier, json, sides, reason)
                    }
                    [Err(_), Ok(json)] => {
                        self.against_unavailable(Revision::Later, json, sides, reason)
                    }
                    _ => Some(reason),
                };
                if let Some(reason) = reason {
                    self.unavailable_record(DifferenceSubject::Document, 0, reason, sides);
                }
                return;
            }
        };
        if let Err(Stop(reason)) = self.compare(pair) {
            self.unfinished(pair, reason);
        }
    }

    /// Record each top-level grouping of an available document as undetermined, because the
    /// corresponding document of the other revision could not be established. The other
    /// side of each difference is that unavailable document, so no grouping reads as removed
    /// or added. Returns the reason for one document-level record instead when there is no
    /// grouping to name or the allowance ran out.
    fn against_unavailable(
        &mut self,
        revision: Revision,
        json: EffectiveJson<'c>,
        sides: [Option<Side<'a>>; 2],
        reason: UndeterminedReason,
    ) -> Option<UndeterminedReason> {
        let mut named = false;
        let result = [Class::Thing, Class::Object]
            .into_iter()
            .try_for_each(|class| {
                let Some(group) = self.member(json, class.keyword())? else {
                    return Ok(());
                };
                let mut definitions = Vec::new();
                if available_map(group) {
                    definitions = self.children(group)?;
                } else {
                    keep(&mut definitions, group, self.allowance)?;
                }
                for definition in definitions {
                    let mut sides = sides;
                    sides[revision.index()] =
                        Some(self.side(revision, definition, presence(definition)));
                    self.unavailable_record(DifferenceSubject::Definition(class), 0, reason, sides);
                    named = true;
                }
                Ok(())
            });
        match result {
            Ok(()) if named => None,
            Ok(()) => Some(reason),
            Err(Stop(limit)) => Some(limit),
        }
    }

    /// Record every top-level grouping of a document that only one revision has.
    pub(super) fn unmatched(&mut self, revision: Revision, root: Root) {
        self.roots = [None, None];
        self.roots[revision.index()] = Some(root);
        let json = self.root_json(revision);
        let mut sides = [None, None];
        sides[revision.index()] = self.root_side(revision, &json);
        let json = match json {
            Ok(json) => json,
            Err(reason) => {
                self.unavailable_record(DifferenceSubject::Document, 0, reason, sides);
                return;
            }
        };
        let mut members = [None, None];
        let result = [Class::Thing, Class::Object]
            .into_iter()
            .try_for_each(|class| {
                members[revision.index()] = self.member(json, class.keyword())?;
                self.definitions(class, members)
            });
        if let Err(Stop(reason)) = result {
            let mut sides = [None, None];
            sides[revision.index()] = Some(self.side(revision, json, Presence::Present));
            self.unavailable_record(DifferenceSubject::Document, 0, reason, sides);
        }
    }

    fn compare(&mut self, pair: Pair<'c>) -> Step<()> {
        let mut tasks = Vec::new();
        for class in [Class::Thing, Class::Object] {
            let members = [
                self.member(pair[0], class.keyword())?,
                self.member(pair[1], class.keyword())?,
            ];
            self.groups(class, pair, members, &mut tasks)?;
        }
        while let Some(task) = tasks.pop() {
            match task {
                Task::Definition { class, pair } => self.definition(class, pair, &mut tasks)?,
                Task::Data { root, pair, data } => {
                    self.start_description();
                    for members in self.described(pair)? {
                        let key = self.key(members);
                        self.data_quality(data, root, pair, key, members, &mut tasks)?;
                    }
                    self.settle(root, pair, data);
                }
                Task::Alternative { root, pair, data } => {
                    self.start_description();
                    for members in self.described(pair)? {
                        let key = self.key(members);
                        // Their description compares what both alternatives inherit.
                        if classify::inherited(pair, key, members, &mut self.allowance.budget)? {
                            continue;
                        }
                        self.data_quality(data, root, pair, key, members, &mut tasks)?;
                    }
                    self.settle(root, pair, data);
                }
            }
        }
        Ok(())
    }

    /// Compare the qualities and contained definitions of corresponding definitions.
    fn definition(&mut self, class: Class, pair: Pair<'c>, tasks: &mut Vec<Task<'c>>) -> Step<()> {
        if class == Class::Context {
            return self.context(pair.map(Some));
        }
        let data = if class == Class::Property {
            self.property_data(pair)?
        } else {
            DataDirections::fixed(0)
        };
        self.start_description();
        for members in self.described(pair)? {
            let key = self.key(members);
            if let Some(child) = crate::role::class(key)
                && class.allows_child(child)
            {
                if child != Class::Data {
                    self.groups(child, pair, members, tasks)?;
                }
                continue;
            }
            match (class, key) {
                // Requiredness belongs to the enclosing grouping, which compares it whole.
                (_, "sdfRequired") => {}
                // Writable allows Client-supplied data, and readable or observable
                // Thing-supplied data.
                (Class::Property, "writable") => {
                    self.quality(Context::Characteristic, pair, members, CLIENT)?
                }
                (Class::Property, "readable" | "observable") => {
                    self.quality(Context::Characteristic, pair, members, THING)?
                }
                (Class::Property, _) => self.data_quality(data, pair, pair, key, members, tasks)?,
                (Class::Action, "sdfInputData") => self.data_member(
                    DataDirections::fixed(CLIENT),
                    Context::Input,
                    pair,
                    members,
                    tasks,
                )?,
                (Class::Action | Class::Event, "sdfOutputData") => self.data_member(
                    DataDirections::fixed(THING),
                    Context::Output,
                    pair,
                    members,
                    tasks,
                )?,
                (Class::Thing | Class::Object, _) => {
                    self.quality(Context::Grouping, pair, members, 0)?
                }
                _ => self.quality(Context::Affordance, pair, members, 0)?,
            }
        }
        if matches!(class, Class::Thing | Class::Object) {
            self.requiredness(pair)?;
        }
        self.settle(pair, pair, data);
        Ok(())
    }

    /// Pair the declarations mandatory in corresponding groupings by their position in the
    /// document, and record a change to them. A declaration that only one revision has is a
    /// removal or an addition of its own.
    fn requiredness(&mut self, pair: Pair<'c>) -> Step<()> {
        let [earlier, later] = [Revision::Earlier, Revision::Later].map(|revision| {
            let required = self.required;
            &required[self.model(revision)]
        });
        let mut dropped = false;
        for &(_, declaration) in earlier.mandatory(pair[0].node) {
            if let Some(node) = self.counterpart(Revision::Earlier, declaration)?
                && !later.is_mandatory(pair[1].node, node)
            {
                dropped = true;
                break;
            }
        }
        let mut entered = false;
        for &(_, declaration) in later.mandatory(pair[1].node) {
            if let Some(node) = self.counterpart(Revision::Later, declaration)?
                && !earlier.is_mandatory(pair[0].node, node)
            {
                entered = true;
                break;
            }
        }
        let resolution = earlier.resolution().and(later.resolution());
        let changed = || self.requirements_changed(pair);
        let Some(mut verdict) = classify::requiredness(dropped, entered, resolution, changed)?
        else {
            return Ok(());
        };
        if self.outcome.perspective == Perspective::Implementer
            && !matches!(verdict, Verdict::Undetermined(_))
        {
            verdict = Verdict::NOT_CLASSIFIED;
        }
        let members = [
            self.member(pair[0], "sdfRequired")?,
            self.member(pair[1], "sdfRequired")?,
        ];
        let sides = self.named_sides(pair, members, "sdfRequired");
        let subject = DifferenceSubject::Quality("sdfRequired");
        self.record(DifferenceKind::Changed, subject, 0, verdict, sides);
        Ok(())
    }

    /// Whether the `sdfRequired` of corresponding groupings, or of any of their
    /// corresponding direct affordances, differs.
    fn requirements_changed(&mut self, pair: Pair<'c>) -> Step<bool> {
        let mut owners = Vec::new();
        keep(&mut owners, pair, self.allowance)?;
        for class in [Class::Property, Class::Action, Class::Event] {
            let groups = [
                self.member(pair[0], class.keyword())?,
                self.member(pair[1], class.keyword())?,
            ];
            let [Some(earlier), Some(later)] = groups else {
                continue;
            };
            for child in earlier.children() {
                let Some(key) = child.n().key else {
                    continue;
                };
                if let Some(other) = self.member(later, key)? {
                    keep(&mut owners, [child, other], self.allowance)?;
                }
            }
        }
        for owner in owners {
            let members = [
                self.member(owner[0], "sdfRequired")?,
                self.member(owner[1], "sdfRequired")?,
            ];
            let equal = match members {
                [None, None] => true,
                [Some(earlier), Some(later)] => {
                    matches!(self.equal(earlier, later)?, Equality::Equal)
                }
                _ => false,
            };
            if !equal {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The node at the same position in the other revision's document, if it has one.
    fn counterpart(&mut self, revision: Revision, node: usize) -> Step<Option<usize>> {
        let models = self.models;
        let model = &models[self.model(revision)];
        let mut keys = Vec::new();
        let mut current = node;
        while let Some(parent) = model.nodes[current].parent {
            self.charge(1)?;
            let Some(key) = model.nodes[current].key else {
                return Ok(None);
            };
            keep(&mut keys, key, self.allowance)?;
            current = parent;
        }
        let Ok(mut json) = self.root_json(revision.other()) else {
            return Ok(None);
        };
        for key in keys.into_iter().rev() {
            match self.member(json, key)? {
                Some(child) => json = child,
                None => return Ok(None),
            }
        }
        Ok(Some(json.node))
    }

    /// The index of a compared revision's effective model.
    fn model(&self, revision: Revision) -> usize {
        self.roots[revision.index()]
            .and_then(|root| root.model.ok())
            .expect("composed revision")
    }

    /// Pair the named definitions of one class group of corresponding owners.
    fn groups(
        &mut self,
        class: Class,
        owner: Pair<'c>,
        members: Members<'c>,
        tasks: &mut Vec<Task<'c>>,
    ) -> Step<()> {
        if self.unavailable_members(DifferenceSubject::Definition(class), owner, members, 0) {
            return Ok(());
        }
        let groups = match members {
            [Some(earlier), Some(later)] => [earlier, later],
            _ => return self.definitions(class, members),
        };
        for definitions in self.members(groups)? {
            match definitions {
                [Some(earlier), Some(later)] if available_map(earlier) && available_map(later) => {
                    keep(
                        tasks,
                        Task::Definition {
                            class,
                            pair: [earlier, later],
                        },
                        self.allowance,
                    )?;
                }
                [Some(_), Some(_)] => {
                    self.unavailable(DifferenceSubject::Definition(class), groups, definitions, 0)
                }
                _ => self.one_sided(DifferenceSubject::Definition(class), definitions, 0)?,
            }
        }
        Ok(())
    }

    /// Record each definition of a class group that only one revision has.
    fn definitions(&mut self, class: Class, members: Members<'c>) -> Step<()> {
        for revision in [Revision::Earlier, Revision::Later] {
            let Some(group) = members[revision.index()] else {
                continue;
            };
            if !available_map(group) {
                let mut sides = [None, None];
                sides[revision.index()] = Some(self.side(revision, group, presence(group)));
                let reason = unavailable_reason(group);
                self.unavailable_record(DifferenceSubject::Definition(class), 0, reason, sides);
                continue;
            }
            for child in self.children(group)? {
                let mut definitions = [None, None];
                definitions[revision.index()] = Some(child);
                self.one_sided(DifferenceSubject::Definition(class), definitions, 0)?;
            }
        }
        Ok(())
    }

    /// Compare Action input or Action or Event output, classifying the member as a whole in
    /// `context` when the revisions do not both have it as a map.
    fn data_member(
        &mut self,
        data: DataDirections,
        context: Context,
        owner: Pair<'c>,
        members: Members<'c>,
        tasks: &mut Vec<Task<'c>>,
    ) -> Step<()> {
        match members {
            [Some(earlier), Some(later)] if available_map(earlier) && available_map(later) => keep(
                tasks,
                Task::Data {
                    root: [earlier, later],
                    pair: [earlier, later],
                    data,
                },
                self.allowance,
            ),
            _ => self.quality(context, owner, members, data.directions),
        }
    }

    /// Compare one quality of corresponding data descriptions inside `root`.
    fn data_quality(
        &mut self,
        data: DataDirections,
        root: Pair<'c>,
        owner: Pair<'c>,
        key: &str,
        members: Members<'c>,
        tasks: &mut Vec<Task<'c>>,
    ) -> Step<()> {
        let entries = match members {
            // The nested content of an opaque Direct items description has no established
            // meaning either.
            _ if self.batch.opaque => {
                return self.quality(Context::Data(data), owner, members, data.directions);
            }
            [Some(earlier), Some(later)] if key == "properties" => [earlier, later],
            [Some(earlier), Some(later)] if key == "sdfChoice" => {
                return self.choice(data, root, owner, [earlier, later], tasks);
            }
            // An array's `items` is the nested description of its values.
            [Some(earlier), Some(later)]
                if key == "items" && available_map(earlier) && available_map(later) =>
            {
                let pair = [earlier, later];
                return keep(tasks, Task::Data { root, pair, data }, self.allowance);
            }
            _ => return self.quality(Context::Data(data), owner, members, data.directions),
        };
        if self.unavailable_members(
            DifferenceSubject::Quality("properties"),
            owner,
            members,
            data.directions,
        ) {
            return Ok(());
        }
        for members in self.members(entries)? {
            match members {
                [Some(earlier), Some(later)] if available_map(earlier) && available_map(later) => {
                    keep(
                        tasks,
                        Task::Data {
                            root,
                            pair: [earlier, later],
                            data,
                        },
                        self.allowance,
                    )?;
                }
                [Some(_), Some(_)] => {
                    self.unavailable(DifferenceSubject::Entry, entries, members, data.directions)
                }
                _ => self.entry(owner, members, data)?,
            }
        }
        Ok(())
    }

    /// Hold a map `properties` entry that only one revision describes, in map data whose
    /// corresponding descriptions are `owner`, until the rest of its map is compared. An entry
    /// that no Client of the earlier revision uses is compatible.
    fn entry(&mut self, owner: Pair<'c>, members: Members<'c>, data: DataDirections) -> Step<()> {
        let kind = removal(members);
        let sides = self.present_sides(members);
        let subject = DifferenceSubject::Entry;
        let mut change = Change::new(subject, data.directions, Verdict::NOT_CLASSIFIED, sides);
        change.kind = kind;
        let name = self.key(members);
        match classify::entry(
            owner,
            name,
            kind,
            data,
            self.outcome.perspective,
            self.allowance,
        ) {
            Some(rule) => change = change.ruled(rule),
            None => change.verdict = Verdict::Compatible,
        }
        keep(&mut self.batch.changes, change, self.allowance)
    }

    /// Compare corresponding choices by the effective alternatives they derive, which
    /// correspond by Given Name, following ADR 0039: Client-side revision comparison.
    /// Corresponding alternatives are compared as descriptions of their own, and an
    /// alternative that only one revision has is held until the rest of its description is
    /// compared. Choices whose alternatives are not established in both revisions are compared
    /// as a quality.
    fn choice(
        &mut self,
        data: DataDirections,
        root: Pair<'c>,
        owner: Pair<'c>,
        choices: Pair<'c>,
        tasks: &mut Vec<Task<'c>>,
    ) -> Step<()> {
        let members = choices.map(Some);
        // Alternatives can differ only where their branches do.
        let changed = match self.equal(choices[0], choices[1])? {
            Equality::Equal => return Ok(()),
            Equality::Different => classify::choices(owner, &mut self.allowance.budget)?,
            Equality::Unavailable(_) => classify::Choices::Unestablished,
        };
        let classify::Choices::Paired(alternatives) = changed else {
            return self.quality(Context::Data(data), owner, members, data.directions);
        };
        for alternatives in alternatives {
            match alternatives {
                [Some(earlier), Some(later)] => keep(
                    tasks,
                    Task::Alternative {
                        root,
                        pair: [earlier, later],
                        data,
                    },
                    self.allowance,
                )?,
                _ => self.alternative(alternatives, choices, data)?,
            }
        }
        Ok(())
    }

    /// Hold an alternative that only one revision's choice has until the rest of its
    /// description is compared. It is located at that alternative in both revisions.
    fn alternative(
        &mut self,
        alternatives: Members<'c>,
        choices: Pair<'c>,
        data: DataDirections,
    ) -> Step<()> {
        let (revision, alternative) = match alternatives {
            [Some(json), None] => (Revision::Earlier, json),
            [None, Some(json)] => (Revision::Later, json),
            _ => unreachable!("an alternative of one revision"),
        };
        let name = self.quality_name(alternatives);
        let sides = [Revision::Earlier, Revision::Later].map(|revision| {
            let index = revision.index();
            Some(match alternatives[index] {
                Some(json) => self.side(revision, json, Presence::Present),
                None => self.absent(revision, choices[index], name),
            })
        });
        let verdict = data
            .unknown
            .map_or(Verdict::NOT_CLASSIFIED, Verdict::Undetermined);
        let subject = DifferenceSubject::Quality("sdfChoice");
        let mut change = Change::new(subject, data.directions, verdict, sides);
        change.kind = removal(alternatives);
        if verdict == Verdict::NOT_CLASSIFIED && data.directions != 0 {
            let directions = data.directions;
            let rule = classify::alternative(alternative, revision, directions, self.allowance);
            change = change.ruled(rule);
        }
        keep(&mut self.batch.changes, change, self.allowance)
    }

    /// Compare a quality structurally and classify a change.
    fn quality(
        &mut self,
        context: Context,
        owner: Pair<'c>,
        members: Members<'c>,
        directions: u8,
    ) -> Step<()> {
        let key = self.key(members);
        let equality = match members {
            [Some(earlier), Some(later)] => {
                if matches!(context, Context::Data(_)) && matches!(key, "enum" | "required") {
                    self.set_equal(earlier, later)?
                } else {
                    self.equal(earlier, later)?
                }
            }
            [Some(json), None] | [None, Some(json)] => match specified_default(key, context, owner)
            {
                Some(default)
                    if presence(json) == Presence::Present
                        && json.boolean().present() == Some(default) =>
                {
                    Equality::Equal
                }
                _ => Equality::Different,
            },
            [None, None] => unreachable!("a paired member"),
        };
        let subject = DifferenceSubject::Quality(self.quality_name(members));
        match equality {
            Equality::Equal => {}
            Equality::Different => {
                let verdict = match context {
                    // A Direct items description carrying a quality outside its vocabulary has
                    // no established meaning, so only its documentary qualities are classified.
                    Context::Data(data) if self.batch.opaque && !Role::DirectItem.metadata(key) => {
                        data.unknown
                            .map_or(Verdict::NOT_CLASSIFIED, Verdict::Undetermined)
                    }
                    // Two spellings of one established meaning are no change at all.
                    Context::Data(data) if key == "contentFormat" => {
                        match self.content_format(members) {
                            None => return Ok(()),
                            Some(verdict) => data.unknown.map_or(verdict, Verdict::Undetermined),
                        }
                    }
                    Context::Data(DataDirections { unknown: None, .. }) if key == "unit" => {
                        self.unit(members)
                    }
                    Context::Grouping if matches!(key, "minItems" | "maxItems") => {
                        // Grouping counts carry no data, so they have no candidate witnesses.
                        let mut groupings = classify::Descriptions::new(owner, 0, self.allowance);
                        classify::instances(key, &mut groupings)
                    }
                    _ => classify::quality(key, context, members),
                };
                let verdict = match context {
                    Context::Data(DataDirections { unknown: None, .. }) if !self.batch.opaque => {
                        classify::meaning(key, verdict, owner, &mut self.allowance.budget)
                    }
                    _ => verdict,
                };
                let sides = self.sides(owner, members);
                self.changed(context, subject, directions, verdict, sides)?;
            }
            Equality::Unavailable(reason) => {
                let sides = self.sides(owner, members);
                self.unavailable_record(subject, directions, reason, sides);
            }
        }
        Ok(())
    }

    /// Classify a changed unit, looking up each name in the recorded SenML evidence on the
    /// one operation allowance.
    fn unit(&mut self, members: Members<'c>) -> Verdict {
        use super::UnitReason;
        use crate::units::{Counter, Unfinished, lookup};
        let names = members.map(|json| json.map(|json| json.text().present()));
        let (registry, work) = (&mut self.outcome.registry, &mut self.allowance.budget.work);
        classify::unit(names, |name| match lookup(name, false, registry, work) {
            Ok((Counter::Unknown, _)) => Ok(None),
            Ok((_, legacy)) => Ok(Some(legacy)),
            Err(Unfinished::Withheld) => {
                Err(UndeterminedReason::Unit(UnitReason::EvidenceWithheld))
            }
            Err(Unfinished::Limited) => Err(UndeterminedReason::limit("comparison work limit")),
        })
    }

    /// Classify a changed `contentFormat` by the established meaning of each declaration,
    /// consulting recorded evidence on the one operation allowance, or `None` when both
    /// declarations mean the same.
    fn content_format(&mut self, members: Members<'c>) -> Option<Verdict> {
        let declarations = members.map(|json| json.map(|json| json.text().present()));
        let (registry, work) = (&mut self.outcome.registry, &mut self.allowance.budget.work);
        classify::content_format(declarations, |texts| {
            crate::content_format::same_meaning(texts, registry, work)
        })
    }

    /// The Property's data directions offered by both revisions, from their effective
    /// characteristics including Specified default characteristics.
    fn property_data(&mut self, pair: Pair<'c>) -> Step<DataDirections> {
        let mut directions = CLIENT | THING;
        let mut unknown = None;
        for json in pair {
            self.charge(3)?;
            let property = crate::EffectiveDefinition::from_json(json, Class::Property);
            let mut flag = |field: Field<crate::Characteristic>| match field {
                Field::Present(characteristic) => characteristic.value(),
                Field::Absent => true,
                Field::Unavailable(message) => {
                    unknown.get_or_insert(UndeterminedReason::Unavailable {
                        code: "comparison.directions_unavailable",
                        message,
                    });
                    false
                }
            };
            let writable = flag(property.writable_characteristic());
            let readable = flag(property.readable_characteristic());
            let observable = flag(property.observable_characteristic());
            let mut offered = 0;
            if writable {
                offered |= CLIENT;
            }
            if readable || observable {
                offered |= THING;
            }
            directions &= offered;
        }
        Ok(DataDirections {
            directions,
            unknown,
        })
    }

    fn equal(&mut self, earlier: EffectiveJson<'c>, later: EffectiveJson<'c>) -> Step<Equality> {
        Ok(read::equal(earlier, later, self.allowance)?)
    }

    /// Compare `enum` values or `required` names as sets of decoded strings, so that
    /// reordering changes nothing. Other shapes use the structural comparison.
    fn set_equal(
        &mut self,
        earlier: EffectiveJson<'c>,
        later: EffectiveJson<'c>,
    ) -> Step<Equality> {
        if !read::strings(earlier, &mut self.allowance.budget)?
            || !read::strings(later, &mut self.allowance.budget)?
        {
            return self.equal(earlier, later);
        }
        let (earlier, later) = (Some(earlier), Some(later));
        Ok(
            if read::within(earlier, later, &mut self.allowance.budget)?
                && read::within(later, earlier, &mut self.allowance.budget)?
            {
                Equality::Equal
            } else {
                Equality::Different
            },
        )
    }

    /// Record an unavailable class group or `properties` map, returning whether one was.
    fn unavailable_members(
        &mut self,
        subject: DifferenceSubject<'a>,
        owner: Pair<'c>,
        members: Members<'c>,
        directions: u8,
    ) -> bool {
        if members.iter().flatten().all(|json| available_map(*json)) {
            return false;
        }
        let reason = members
            .iter()
            .flatten()
            .find(|json| !available_map(**json))
            .map(|json| unavailable_reason(*json))
            .expect("an unavailable member");
        let sides = self.sides(owner, members);
        self.unavailable_record(subject, directions, reason, sides);
        true
    }

    /// Record corresponding content that an effective view could not establish.
    fn unavailable(
        &mut self,
        subject: DifferenceSubject<'a>,
        owner: Pair<'c>,
        members: Members<'c>,
        directions: u8,
    ) {
        let reason = members
            .iter()
            .flatten()
            .find(|json| !available_map(**json))
            .map_or(identity_unavailable(), |json| unavailable_reason(*json));
        let sides = self.sides(owner, members);
        self.unavailable_record(subject, directions, reason, sides);
    }

    /// Context is compared as non-interactive metadata, without proving compatibility for
    /// metadata consumers. The structural reader establishes availability before equality.
    fn context(&mut self, members: Members<'c>) -> Step<()> {
        let subject = DifferenceSubject::Definition(Class::Context);
        let sides = self.present_sides(members);
        let equality = read::context_equal(members, self.allowance)?;
        let kind = match members {
            [Some(_), Some(_)] => DifferenceKind::Changed,
            _ => removal(members),
        };
        match equality {
            Equality::Equal => {}
            Equality::Different => self.record(kind, subject, 0, Verdict::Compatible, sides),
            Equality::Unavailable(reason) => self.unavailable_record(subject, 0, reason, sides),
        }
        Ok(())
    }

    /// Record a definition that only one revision has.
    fn one_sided(
        &mut self,
        subject: DifferenceSubject<'a>,
        members: Members<'c>,
        directions: u8,
    ) -> Step<()> {
        if subject == DifferenceSubject::Definition(Class::Context) {
            return self.context(members);
        }
        let kind = removal(members);
        let sides = self.present_sides(members);
        let verdict = if self.outcome.perspective == Perspective::Implementer {
            Verdict::NOT_CLASSIFIED
        } else {
            classify::definition(kind)
        };
        self.record(kind, subject, directions, verdict, sides);
        Ok(())
    }
    /// The side of each revision that has the member.
    fn present_sides(&self, members: Members<'c>) -> [Option<Side<'a>>; 2] {
        [Revision::Earlier, Revision::Later].map(|revision| {
            members[revision.index()].map(|json| self.side(revision, json, Presence::Present))
        })
    }

    /// Both revisions' sides for a member of corresponding owners.
    fn sides(&mut self, owner: Pair<'c>, members: Members<'c>) -> [Option<Side<'a>>; 2] {
        let key = self.quality_name(members);
        self.named_sides(owner, members, key)
    }
    /// Both revisions' sides for the member `key` of corresponding owners, which neither
    /// revision may have.
    fn named_sides(
        &mut self,
        owner: Pair<'c>,
        members: Members<'c>,
        key: &'a str,
    ) -> [Option<Side<'a>>; 2] {
        [Revision::Earlier, Revision::Later].map(|revision| {
            Some(match members[revision.index()] {
                Some(json) => self.side(revision, json, presence(json)),
                None => self.absent(revision, owner[revision.index()], key),
            })
        })
    }
    fn side(&self, revision: Revision, json: EffectiveJson<'c>, presence: Presence) -> Side<'a> {
        let root = self.roots[revision.index()].expect("walked document");
        Side {
            root: root.id,
            model: root.model.ok(),
            node: Some(json.node),
            trailing: None,
            presence,
            origin: Some(json.node),
        }
    }
    /// A member absent from its owner, located where it would be. A deletion through a
    /// composition patch is its origin.
    fn absent(&mut self, revision: Revision, owner: EffectiveJson<'c>, key: &'a str) -> Side<'a> {
        let root = self.roots[revision.index()].expect("walked document");
        let mut deleted = None;
        let mut child = owner.n().first;
        while let Some(id) = child {
            let node = &owner.model.nodes[id];
            if node.deleted && node.key == Some(key) {
                deleted = Some(id);
            }
            child = node.next;
        }
        Side {
            root: root.id,
            model: root.model.ok(),
            node: Some(owner.node),
            trailing: Some(key),
            presence: Presence::Absent,
            origin: deleted,
        }
    }
    fn root_side(
        &self,
        revision: Revision,
        json: &Result<EffectiveJson<'c>, UndeterminedReason>,
    ) -> Option<Side<'a>> {
        let root = self.roots[revision.index()]?;
        Some(match json {
            Ok(json) => self.side(revision, *json, Presence::Present),
            Err(_) => Side {
                root: root.id,
                model: root.model.ok(),
                node: root
                    .model
                    .ok()
                    .and_then(|model| self.models[model].selected),
                trailing: None,
                presence: Presence::Unavailable,
                origin: None,
            },
        })
    }
    /// Note that the allowance stopped the comparison of a document. Changes found in the
    /// unfinished description keep their own classification, but none is proven or witnessed.
    fn unfinished(&mut self, pair: Pair<'c>, reason: UndeterminedReason) {
        witness::stop(&mut self.batch, reason);
        for change in std::mem::take(&mut self.batch.changes) {
            self.retain_change(change);
        }
        let sides = [
            Some(self.side(Revision::Earlier, pair[0], Presence::Present)),
            Some(self.side(Revision::Later, pair[1], Presence::Present)),
        ];
        self.unavailable_record(DifferenceSubject::Document, 0, reason, sides);
    }

    fn record(
        &mut self,
        kind: DifferenceKind,
        subject: DifferenceSubject<'a>,
        directions: u8,
        verdict: Verdict,
        sides: [Option<Side<'a>>; 2],
    ) {
        if kind == DifferenceKind::Unavailable {
            self.batch.unavailable = true;
        }
        self.retain(Record {
            kind,
            subject,
            directions,
            verdict,
            sides,
            witness: None,
            acknowledged: false,
            pointers: None,
        });
    }
    /// Record content that an effective view could not establish, or that the allowance
    /// left uncompared.
    fn unavailable_record(
        &mut self,
        subject: DifferenceSubject<'a>,
        directions: u8,
        reason: UndeterminedReason,
        sides: [Option<Side<'a>>; 2],
    ) {
        let verdict = Verdict::Undetermined(reason);
        self.record(
            DifferenceKind::Unavailable,
            subject,
            directions,
            verdict,
            sides,
        );
    }
    fn retain(&mut self, record: Record<'a>) {
        let classification = record.verdict.classification();
        self.outcome.counts[classification.rank()] += 1;
        // A definite classification of a quality relies on reading the extension vocabulary
        // beside it as ignorable, following ADR 0037: Undeclared extension vocabulary.
        if classification != Classification::Undetermined
            && matches!(record.subject, DifferenceSubject::Quality(_))
        {
            self.allowance.budget.set_aside |= self.aside;
        }
        if let Some(witness) = &record.witness {
            self.outcome.interpretations |= witness.interpretations;
        }
        if push(
            &mut self.outcome.records,
            record,
            &mut self.allowance.budget.allocation,
        )
        .is_err()
        {
            self.outcome.unretained += 1;
        }
    }

    /// Record a changed quality, holding a data quality until its whole description is
    /// compared.
    fn changed(
        &mut self,
        context: Context,
        subject: DifferenceSubject<'a>,
        directions: u8,
        verdict: Verdict,
        sides: [Option<Side<'a>>; 2],
    ) -> Step<()> {
        if !matches!(context, Context::Data(_)) {
            // Endpoint and capability obligations need their own implementer rules.
            // Documentary changes retain their existing perspective-independent meaning.
            let verdict = if self.outcome.perspective == Perspective::Implementer
                && !matches!(verdict, Verdict::Undetermined(_))
                && !matches!(
                    subject,
                    DifferenceSubject::Quality("description" | "label" | "$comment" | "default")
                ) {
                Verdict::NOT_CLASSIFIED
            } else {
                verdict
            };
            self.record(DifferenceKind::Changed, subject, directions, verdict, sides);
            return Ok(());
        }
        let change = Change::new(subject, directions, verdict, sides);
        keep(&mut self.batch.changes, change, self.allowance)
    }
    /// Start comparing the members of one description, none of them unavailable yet.
    fn start_description(&mut self) {
        self.batch.unavailable = false;
    }
    /// Classify the changed qualities of a compared description inside `root` together,
    /// since a Compatibility proof covers all of them.
    fn settle(&mut self, root: Pair<'c>, pair: Pair<'c>, data: DataDirections) {
        if self.batch.changes.is_empty() {
            return;
        }
        witness::settle(&mut self.batch, root, pair, data, self.allowance);
        for change in std::mem::take(&mut self.batch.changes) {
            self.retain_change(change);
        }
    }
    fn retain_change(&mut self, change: Change<'a>) {
        self.retain(Record {
            kind: change.kind,
            subject: change.subject,
            directions: change.directions,
            verdict: change.verdict,
            sides: change.sides,
            witness: change.witness,
            acknowledged: false,
            pointers: None,
        });
    }

    /// The effective root of a document, or why it has none.
    fn root_json(&self, revision: Revision) -> Result<EffectiveJson<'c>, UndeterminedReason> {
        let root = self.roots[revision.index()].expect("walked document");
        let model = &self.models[root.model?];
        match model.json() {
            Field::Present(json) => Ok(json),
            _ => Err(
                match model
                    .selected
                    .and_then(|node| model.nodes[node].unavailable)
                    .or(model.failure)
                {
                    Some(failure) => UndeterminedReason::Unavailable {
                        code: failure.code(),
                        message: failure.reason,
                    },
                    None => source_reason(model.catalogue.source(root.id).expect("model source")),
                },
            ),
        }
    }

    /// The members of corresponding maps, paired by name: each earlier member with its later
    /// namesake, then the later members without one.
    /// A member without a decoded name has no correspondent, so its owners' comparison is
    /// unavailable there.
    fn members(&mut self, pair: Pair<'c>) -> Step<Vec<Members<'c>>> {
        let mut members = Vec::new();
        let mut keyless = false;
        for (index, revision) in [Revision::Earlier, Revision::Later].into_iter().enumerate() {
            for child in pair[index].children() {
                let Some(key) = child.n().key else {
                    keyless = true;
                    continue;
                };
                let other = read::member(pair[1 - index], key, &mut self.allowance.budget)?;
                let mut paired = [None, None];
                paired[revision.index()] = Some(child);
                if revision == Revision::Earlier {
                    paired[1] = other;
                } else if other.is_some() {
                    continue;
                }
                keep(&mut members, paired, self.allowance)?;
            }
        }
        if keyless {
            let sides = [
                Some(self.side(Revision::Earlier, pair[0], Presence::Present)),
                Some(self.side(Revision::Later, pair[1], Presence::Present)),
            ];
            let reason = identity_unavailable();
            self.unavailable_record(DifferenceSubject::Quality(""), 0, reason, sides);
        }
        Ok(members)
    }
    /// The members of corresponding descriptions, noting the extension vocabulary among them
    /// for the classifications of their qualities, and whether they are Direct items
    /// descriptions carrying a quality outside their vocabulary.
    fn described(&mut self, pair: Pair<'c>) -> Step<Vec<Members<'c>>> {
        let members = self.members(pair)?;
        // `required` and `properties` belong to the vocabulary of object items.
        let mut objects = [false; 2];
        for (object, owner) in objects.iter_mut().zip(pair) {
            if owner.n().role == Some(Role::DirectItem)
                && let Some(json) = self.member(owner, "type")?
            {
                *object = json.text().present() == Some("object");
            }
        }
        let mut aside = SetAside::default();
        let mut opaque = false;
        for paired in &members {
            for ((owner, member), object) in pair.iter().zip(paired).zip(objects) {
                if let (Some(role), Some(key)) =
                    (owner.n().role, member.and_then(|json| json.n().key))
                {
                    self.charge(key.len() + 1)?;
                    let vocabulary = aside.classify(role, key, object);
                    opaque |= role == Role::DirectItem && vocabulary.blocks_base_meaning();
                }
            }
        }
        self.aside = aside;
        self.batch.opaque = opaque;
        Ok(members)
    }
    fn children(&mut self, json: EffectiveJson<'c>) -> Step<Vec<EffectiveJson<'c>>> {
        let mut children = Vec::new();
        for child in json.children() {
            self.charge(1)?;
            keep(&mut children, child, self.allowance)?;
        }
        Ok(children)
    }
    fn member(&mut self, json: EffectiveJson<'c>, key: &str) -> Step<Option<EffectiveJson<'c>>> {
        if json.kind() != JsonKind::Map {
            return Ok(None);
        }
        Ok(read::member(json, key, &mut self.allowance.budget)?)
    }
    fn key(&self, members: Members<'c>) -> &'c str {
        let json = members
            .into_iter()
            .flatten()
            .next()
            .expect("a paired member");
        json.n().key.unwrap_or("")
    }
    /// The member name with the catalogue's lifetime, for retained records.
    fn quality_name(&self, members: Members<'c>) -> &'a str {
        for revision in [Revision::Earlier, Revision::Later] {
            if let (Some(json), Some(root)) =
                (members[revision.index()], self.roots[revision.index()])
                && let Ok(model) = root.model
                && let Some(key) = self.models[model].nodes[json.node].key
            {
                return key;
            }
        }
        ""
    }
    fn charge(&mut self, cost: usize) -> Step<()> {
        Ok(read::work(&mut self.allowance.budget, cost)?)
    }
}

/// Push `item` within the allocation allowance, or stop the walk.
fn keep<T>(items: &mut Vec<T>, item: T, allowance: &mut ValueAllowance) -> Step<()> {
    push(items, item, &mut allowance.budget.allocation)
        .map_err(|_| Stop(UndeterminedReason::limit("comparison allocation limit")))
}
/// The Specified default of a characteristic, which an absent quality takes, so that absent
/// and explicit values compare alike. An absent `uniqueItems` reads as `false`. A Direct items
/// description has neither a Nullable characteristic nor `uniqueItems`, following ADR 0024:
/// Effective alternatives and nullable characteristics, so absent and explicit values differ
/// there.
fn specified_default(key: &str, context: Context, owner: Pair<'_>) -> Option<bool> {
    match (context, key) {
        (Context::Characteristic, _) => Some(true),
        (Context::Data(_), _)
            if owner
                .iter()
                .any(|json| json.n().role == Some(Role::DirectItem)) =>
        {
            None
        }
        (Context::Data(_), "nullable") => Some(true),
        (Context::Data(_), "uniqueItems") => Some(false),
        _ => None,
    }
}
fn removal(members: Members<'_>) -> DifferenceKind {
    if members[0].is_some() {
        DifferenceKind::Removed
    } else {
        DifferenceKind::Added
    }
}
fn available_map(json: EffectiveJson<'_>) -> bool {
    failure(json).is_none() && json.kind() == JsonKind::Map
}
fn presence(json: EffectiveJson<'_>) -> Presence {
    if failure(json).is_some() {
        Presence::Unavailable
    } else {
        Presence::Present
    }
}
fn source_reason(source: &crate::Source) -> UndeterminedReason {
    if source.acquisition == crate::Acquisition::Failed {
        return UndeterminedReason::Unavailable {
            code: "input.acquisition",
            message: "source content could not be acquired",
        };
    }
    match source
        .parsed
        .issues
        .first()
        .or(source.parsed.allocation_issue.as_ref())
    {
        Some(issue) => UndeterminedReason::Unavailable {
            code: issue.code,
            message: issue.reason,
        },
        None => UndeterminedReason::Unavailable {
            code: "input.acquisition",
            message: "source content unavailable",
        },
    }
}

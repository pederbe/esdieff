//! Required declaration identities are separate from target capabilities.
//! Follows ADR 0026: Required declaration relationships and source evidence.
use super::*;
use crate::{Definition, EffectiveDefinition, Lookup};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RequiredDeclarationsState {
    Available,
    Partial,
    Unavailable,
}
impl RequiredDeclarationsState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RequirednessState {
    Absent,
    Available,
    Partial,
    Invalid,
    Unavailable,
}
impl RequirednessState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Available => "available",
            Self::Partial => "partial",
            Self::Invalid => "invalid",
            Self::Unavailable => "unavailable",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RequiredReferenceForm<'a> {
    Carrier,
    DirectName(&'a str),
    NameReference(&'a str),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RequiredReferenceStatus {
    Resolved,
    Partial,
    Missing,
    Ambiguous,
    Unavailable,
    WrongTarget,
    InvalidSyntax,
    UnsupportedContext,
    /// The Context extension draft leaves Context requiredness undefined.
    UndefinedContext,
}
impl RequiredReferenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Partial => "partial",
            Self::Missing => "missing",
            Self::Ambiguous => "ambiguous",
            Self::Unavailable => "unavailable",
            Self::WrongTarget => "wrong_target",
            Self::InvalidSyntax => "invalid_syntax",
            Self::UnsupportedContext => "unsupported_context",
            Self::UndefinedContext => "undefined_context",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RequiredTargetCapability {
    Established,
    Partial,
    Unavailable,
    NotRequested,
}
impl RequiredTargetCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Established => "established",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
            Self::NotRequested => "not_requested",
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ViewNode<'a> {
    Source(JsonView<'a>),
    Effective(usize),
}
pub(crate) struct Target<'a> {
    pub(crate) source: Option<JsonView<'a>>,
    pub(crate) effective: Option<usize>,
    pub(crate) class: Class,
    pub(crate) name: &'a str,
    pub(crate) complete: bool,
    pub(crate) capability_unavailable: bool,
}
impl Target<'_> {
    pub(crate) fn capability(
        &self,
        model: Option<&EffectiveModel<'_>>,
    ) -> RequiredTargetCapability {
        if self.capability_unavailable {
            return RequiredTargetCapability::Unavailable;
        }
        match (model, self.effective) {
            (Some(model), Some(node)) if model.nodes[node].unavailable.is_some() => {
                RequiredTargetCapability::Unavailable
            }
            (Some(model), Some(node)) if model.nodes[node].complete => {
                RequiredTargetCapability::Established
            }
            (Some(_), Some(_)) => RequiredTargetCapability::Partial,
            _ => RequiredTargetCapability::NotRequested,
        }
    }
}
#[derive(Clone, Copy)]
enum TargetScope {
    Outside,
    Processed(usize),
    Deleted,
    Unprocessed,
}
pub(crate) struct Reference<'a> {
    pub(crate) json: ViewNode<'a>,
    pub(crate) source: JsonView<'a>,
    pub(crate) occurrence: usize,
    pub(crate) form: Option<RequiredReferenceForm<'a>>,
    pub(crate) status: RequiredReferenceStatus,
    pub(crate) complete: bool,
    pub(crate) targets: Vec<Target<'a>>,
    pub(crate) related: Vec<JsonView<'a>>,
    pub(crate) error_already_counted: bool,
}
pub(crate) struct Owner<'a> {
    pub(crate) json: ViewNode<'a>,
    pub(crate) source: JsonView<'a>,
    pub(crate) class: Option<Class>,
    pub(crate) syntax: Option<ViewNode<'a>>,
    pub(crate) state: RequirednessState,
    pub(crate) complete: bool,
    pub(crate) references: Vec<Reference<'a>>,
}
pub(crate) struct Collection<'a> {
    pub(crate) state: RequiredDeclarationsState,
    pub(crate) complete: bool,
    pub(crate) reason: Option<&'static str>,
    pub(crate) owners: Vec<Owner<'a>>,
    pub(crate) owners_known: u32,
    pub(crate) references_known: u32,
    pub(crate) targets_known: u32,
    pub(crate) references_retained: u32,
    pub(crate) targets_retained: u32,
    pub(crate) errors: u32,
    pub(crate) notices: u32,
    pub(crate) conditions: u32,
    /// The collection relied on `copied-reference-relocation/1`, which a
    /// source view reports without an effective model.
    pub(crate) relocation_interpretation: bool,
    /// Every definition in the requested context was visited, so no owner is missing,
    /// although an owner's own relationships may still be unfinished.
    pub(crate) traversed: bool,
}
impl<'a> Collection<'a> {
    fn new() -> Self {
        Self {
            relocation_interpretation: false,
            traversed: true,
            state: RequiredDeclarationsState::Available,
            complete: true,
            reason: None,
            owners: Vec::new(),
            owners_known: 0,
            references_known: 0,
            targets_known: 0,
            references_retained: 0,
            targets_retained: 0,
            errors: 0,
            notices: 0,
            conditions: 0,
        }
    }
    /// The traversal stopped or could not see part of the requested context.
    fn limited(&mut self, reason: &'static str) {
        self.traversed = false;
        self.unfinished(reason);
    }
    /// A visited owner's relationships are unfinished.
    fn unfinished(&mut self, reason: &'static str) {
        self.complete = false;
        self.reason = Some(reason);
        self.state = if self.owners.is_empty() {
            RequiredDeclarationsState::Unavailable
        } else {
            RequiredDeclarationsState::Partial
        };
    }
}
#[derive(Clone, Copy)]
pub struct RequiredDeclarations<'a> {
    pub(crate) collection: &'a Collection<'a>,
    pub(crate) model: Option<&'a EffectiveModel<'a>>,
}
impl<'a> RequiredDeclarations<'a> {
    pub fn state(self) -> RequiredDeclarationsState {
        self.collection.state
    }
    pub fn complete(self) -> bool {
        self.collection.complete
    }
    pub fn unavailable_reason(self) -> Option<&'static str> {
        self.collection.reason
    }
    pub fn owners(self) -> RequiredDeclarationSets<'a> {
        RequiredDeclarationSets {
            view: self,
            rows: self.collection.owners.iter(),
        }
    }
    pub fn owners_known(self) -> u32 {
        self.collection.owners_known
    }
    pub fn references_known(self) -> u32 {
        self.collection.references_known
    }
    pub fn targets_known(self) -> u32 {
        self.collection.targets_known
    }
}
pub struct RequiredDeclarationSets<'a> {
    view: RequiredDeclarations<'a>,
    rows: std::slice::Iter<'a, Owner<'a>>,
}
impl<'a> Iterator for RequiredDeclarationSets<'a> {
    type Item = RequiredDeclarationSet<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.rows.next().map(|row| RequiredDeclarationSet {
            view: self.view,
            row,
        })
    }
}
#[derive(Clone, Copy)]
pub struct RequiredDeclarationSet<'a> {
    view: RequiredDeclarations<'a>,
    row: &'a Owner<'a>,
}
impl<'a> RequiredDeclarationSet<'a> {
    pub fn source_owner(self) -> Field<JsonView<'a>> {
        Field::Present(self.row.source)
    }
    pub fn effective_owner(self) -> Field<EffectiveJson<'a>> {
        self.view.effective(self.row.json)
    }
    pub fn class(self) -> Field<Class> {
        self.row.class.map_or(Field::Absent, Field::Present)
    }
    pub fn state(self) -> RequirednessState {
        self.row.state
    }
    pub fn complete(self) -> bool {
        self.row.complete
    }
    pub fn source_syntax(self) -> Field<JsonView<'a>> {
        self.row.source.get("sdfRequired")
    }
    pub fn effective_syntax(self) -> Field<EffectiveJson<'a>> {
        self.row
            .syntax
            .map_or(Field::Absent, |node| self.view.effective(node))
    }
    pub fn references(self) -> Field<RequiredReferences<'a>> {
        match self.row.syntax {
            None => Field::Absent,
            Some(ViewNode::Source(json)) if json.kind() != JsonKind::Array => {
                Field::Unavailable("required array shape unavailable")
            }
            Some(ViewNode::Effective(node))
                if self
                    .view
                    .model
                    .is_none_or(|model| model.nodes[node].source.kind() != JsonKind::Array) =>
            {
                Field::Unavailable("required array shape unavailable")
            }
            _ => Field::Present(RequiredReferences {
                view: self.view,
                rows: self.row.references.iter(),
            }),
        }
    }
}
impl<'a> RequiredDeclarations<'a> {
    fn effective(self, node: ViewNode<'a>) -> Field<EffectiveJson<'a>> {
        match (self.model, node) {
            (Some(model), ViewNode::Effective(node)) => {
                Field::Present(EffectiveJson { model, node })
            }
            _ => Field::Absent,
        }
    }
}
pub struct RequiredReferences<'a> {
    view: RequiredDeclarations<'a>,
    rows: std::slice::Iter<'a, Reference<'a>>,
}
impl<'a> Iterator for RequiredReferences<'a> {
    type Item = RequiredReferenceInspection<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.rows.next().map(|row| RequiredReferenceInspection {
            view: self.view,
            row,
        })
    }
}
#[derive(Clone, Copy)]
pub struct RequiredReferenceInspection<'a> {
    view: RequiredDeclarations<'a>,
    row: &'a Reference<'a>,
}
impl<'a> RequiredReferenceInspection<'a> {
    pub fn occurrence(self) -> usize {
        self.row.occurrence
    }
    pub fn source_json(self) -> JsonView<'a> {
        self.row.source
    }
    pub fn effective_json(self) -> Field<EffectiveJson<'a>> {
        self.view.effective(self.row.json)
    }
    pub fn form(self) -> Field<RequiredReferenceForm<'a>> {
        self.row.form.map_or(
            Field::Unavailable("required syntax unavailable"),
            Field::Present,
        )
    }
    pub fn status(self) -> RequiredReferenceStatus {
        self.row.status
    }
    pub fn complete(self) -> bool {
        self.row.complete
    }
    pub fn targets(self) -> Lookup<RequiredTargets<'a>> {
        if !self.row.targets.is_empty() {
            Lookup::Found(RequiredTargets {
                view: self.view,
                rows: self.row.targets.iter(),
            })
        } else {
            match self.row.status {
                RequiredReferenceStatus::Missing => Lookup::NotFound,
                RequiredReferenceStatus::Ambiguous => Lookup::Ambiguous,
                _ => Lookup::Unavailable("required relationship unavailable"),
            }
        }
    }
    pub fn related_sources(self) -> impl Iterator<Item = JsonView<'a>> {
        self.row.related.iter().copied()
    }
}
pub struct RequiredTargets<'a> {
    view: RequiredDeclarations<'a>,
    rows: std::slice::Iter<'a, Target<'a>>,
}
impl<'a> Iterator for RequiredTargets<'a> {
    type Item = RequiredTarget<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.rows.next().map(|row| RequiredTarget {
            view: self.view,
            row,
        })
    }
}
#[derive(Clone, Copy)]
pub struct RequiredTarget<'a> {
    view: RequiredDeclarations<'a>,
    row: &'a Target<'a>,
}
impl<'a> RequiredTarget<'a> {
    pub fn class(self) -> Class {
        self.row.class
    }
    pub fn given_name(self) -> &'a str {
        self.row.name
    }
    pub fn source_definition(self) -> Field<Definition<'a>> {
        self.row.source.map_or(
            Field::Unavailable("declaration source position unavailable"),
            |json| {
                if json.kind() == JsonKind::Map {
                    Field::Present(Definition::from_json(json, self.row.class))
                } else {
                    Field::Unavailable("definition map shape unavailable")
                }
            },
        )
    }
    pub fn effective_definition(self) -> Field<EffectiveDefinition<'a>> {
        if self.row.capability_unavailable {
            return Field::Unavailable("target processing context unavailable");
        }
        match (self.view.model, self.row.effective) {
            (Some(model), Some(node)) => {
                let json = EffectiveJson { model, node };
                if let Some(reason) = json.unavailable_reason() {
                    Field::Unavailable(reason)
                } else if json.kind() != JsonKind::Map {
                    Field::Unavailable("definition map shape unavailable")
                } else {
                    Field::Present(EffectiveDefinition::from_json(json, self.row.class))
                }
            }
            _ => Field::Absent,
        }
    }
    pub fn complete(self) -> bool {
        self.row.complete
    }
    pub fn capability(self) -> RequiredTargetCapability {
        self.row.capability(self.view.model)
    }
}

/// The declaration class at `position` inside a definition of class `enclosing`.
fn declaration_class(position: Position, enclosing: Option<Class>) -> Option<Class> {
    match (position, enclosing) {
        (Position::Vocabulary(Role::Definition(class)), Some(parent))
            if !matches!(class, Class::Data | Class::Context)
                && ((parent == Class::Thing && parent.allows_child(class))
                    || (parent == Class::Object
                        && matches!(class, Class::Property | Class::Action | Class::Event))) =>
        {
            Some(class)
        }
        _ => None,
    }
}

impl<'a> Resolver<'a> {
    fn required_source(&self, node: ViewNode<'a>) -> JsonView<'a> {
        match node {
            ViewNode::Source(json) => json,
            ViewNode::Effective(node) => self.model.nodes[node].source,
        }
    }
    fn required_evidence(&self, node: ViewNode<'a>) -> JsonView<'a> {
        match node {
            ViewNode::Source(json) => json,
            ViewNode::Effective(node) => self.model.nodes[node].declaration_source,
        }
    }
    fn required_children(&mut self, node: ViewNode<'a>) -> Result<Vec<ViewNode<'a>>, Failure> {
        let mut out = Vec::new();
        match node {
            ViewNode::Source(json) => {
                for child in json.children() {
                    self.required_work(1)?;
                    push(&mut out, ViewNode::Source(child), &mut self.allocation)?;
                }
            }
            ViewNode::Effective(node) => {
                let mut child = self.model.nodes[node].first;
                while let Some(id) = child {
                    self.required_work(1)?;
                    let n = &self.model.nodes[id];
                    child = n.next;
                    if !n.deleted {
                        push(&mut out, ViewNode::Effective(id), &mut self.allocation)?;
                    }
                }
            }
        }
        Ok(out)
    }
    fn required_name(&self, node: ViewNode<'a>) -> Option<&'a str> {
        match node {
            ViewNode::Source(json) => json.member_name().present(),
            ViewNode::Effective(node) => self.model.nodes[node].key,
        }
    }
    fn required_field(
        &mut self,
        node: ViewNode<'a>,
        key: &str,
    ) -> Result<Option<ViewNode<'a>>, Failure> {
        let mut found = None;
        for child in self.required_children(node)? {
            let name = self
                .required_name(child)
                .ok_or(Failure::meaning("required member identity unavailable"))?;
            self.required_work(name.len() + key.len() + 1)?;
            if name == key {
                found = Some(child);
            }
        }
        Ok(found)
    }
    fn required_work(&mut self, cost: usize) -> Result<(), Failure> {
        if charge(&mut self.work, cost) {
            Ok(())
        } else {
            Err(Failure::limit("required traversal work limit"))
        }
    }
    fn required_record(&mut self) -> Result<(), Failure> {
        self.required_work(1)?;
        if charge(&mut self.nodes_left, 1) {
            Ok(())
        } else {
            Err(Failure::limit("required record node limit"))
        }
    }
    fn source_position(
        &mut self,
        json: JsonView<'a>,
    ) -> Result<(Position, Option<Class>), Failure> {
        let mut ancestry = Vec::new();
        let mut current = json;
        while let Some(parent) = current.n().parent {
            self.required_work(1)?;
            push(&mut ancestry, current, &mut self.allocation)?;
            current = JsonView {
                source: current.source,
                node: parent,
            };
        }
        let mut pos = Position::Vocabulary(Role::Document);
        let mut enclosing = None;
        for child in ancestry.into_iter().rev() {
            self.required_work(1)?;
            let Some(key) = child.n().key.as_deref() else {
                return Ok((Position::Opaque, None));
            };
            self.required_work(key.len())?;
            if let Position::Vocabulary(Role::Definition(class)) = pos {
                enclosing = Some(class);
            } else if !matches!(pos, Position::Named(_)) {
                enclosing = None;
            }
            let object = matches!(pos, Position::Vocabulary(Role::DirectItem))
                && self
                    .source_field(current, "type")?
                    .is_some_and(|j| matches!(j.text(), Field::Present("object")));
            pos = pos.child(key, object);
            current = child;
        }
        Ok((pos, enclosing))
    }
    fn declaration_position(&mut self, json: JsonView<'a>) -> Result<Option<Class>, Failure> {
        let (position, enclosing) = self.source_position(json)?;
        Ok(declaration_class(position, enclosing))
    }
    fn required_target(
        &mut self,
        node: ViewNode<'a>,
        class: Class,
        complete: bool,
    ) -> Result<Target<'a>, Failure> {
        self.required_record()?;
        let evidence = self.required_evidence(node);
        let (pos, _) = self.source_position(evidence)?;
        let source = matches!(pos, Position::Vocabulary(Role::Definition(c)) if c == class)
            .then_some(evidence);
        let (effective, capability_unavailable) = match node {
            ViewNode::Effective(node)
                if self.model.nodes[node].role == Some(Role::Definition(class)) =>
            {
                (Some(node), false)
            }
            ViewNode::Effective(_) => (None, true),
            ViewNode::Source(_) => (None, false),
        };
        Ok(Target {
            source,
            effective,
            class,
            name: self.required_name(node).unwrap_or(""),
            complete: complete && source.is_some(),
            capability_unavailable,
        })
    }
    fn processed_target(
        &mut self,
        source: JsonView<'a>,
        class: Class,
    ) -> Result<TargetScope, Failure> {
        let Some(selected) = self.model.selected else {
            return Ok(TargetScope::Outside);
        };
        if source.source_id() != self.model.source {
            return Ok(TargetScope::Outside);
        }
        let mut pending = Vec::new();
        push(&mut pending, selected, &mut self.allocation)?;
        while let Some(id) = pending.pop() {
            self.required_work(1)?;
            if self.same_required_coordinate(source, id)? {
                let node = &self.model.nodes[id];
                return Ok(if node.deleted {
                    TargetScope::Deleted
                } else if node.role == Some(Role::Definition(class)) {
                    TargetScope::Processed(id)
                } else {
                    TargetScope::Unprocessed
                });
            }
            let mut child = self.model.nodes[id].first;
            while let Some(id) = child {
                self.required_work(1)?;
                child = self.model.nodes[id].next;
                push(&mut pending, id, &mut self.allocation)?;
            }
        }
        Ok(TargetScope::Outside)
    }
    fn same_required_coordinate(
        &mut self,
        mut source: JsonView<'a>,
        mut node: usize,
    ) -> Result<bool, Failure> {
        loop {
            self.required_work(1)?;
            let effective = &self.model.nodes[node];
            if !charge(
                &mut self.work,
                source.n().key.as_deref().map_or(0, str::len) + effective.key.map_or(0, str::len),
            ) {
                return Err(Failure::limit("required coordinate work limit"));
            }
            if source.n().key.as_deref() != effective.key || source.n().index != effective.index {
                return Ok(false);
            }
            match (source.n().parent, effective.parent) {
                (Some(a), Some(b)) => {
                    source = JsonView {
                        source: source.source,
                        node: a,
                    };
                    node = b;
                }
                (None, None) => return Ok(true),
                _ => return Ok(false),
            }
        }
    }
    /// The selected subtree's element at a root-document `pointer`. An element
    /// whose `sdfRef` stayed uncomposed lies outside the selected subtree.
    fn processed_coordinate(
        &mut self,
        pointer: &str,
        class: Class,
    ) -> Result<TargetScope, Failure> {
        let (Some(mut node), Some(selected)) = (self.model.root, self.model.selected) else {
            return Ok(TargetScope::Outside);
        };
        let mut inside = node == selected;
        for token in pointer
            .strip_prefix('/')
            .into_iter()
            .flat_map(|tail| tail.split('/'))
        {
            self.required_work(token.len() * 4 + 1)?;
            let n = &self.model.nodes[node];
            if n.unavailable.is_some() {
                return Ok(if inside {
                    TargetScope::Unprocessed
                } else {
                    TargetScope::Outside
                });
            }
            if n.source.kind() == JsonKind::Map
                && self
                    .find(node, "sdfRef")?
                    .is_some_and(|id| !self.model.nodes[id].deleted)
            {
                return Ok(TargetScope::Outside);
            }
            let name = token.replace("~1", "/").replace("~0", "~");
            let next = match self.model.nodes[node].source.kind() {
                JsonKind::Map => self.find(node, &name)?,
                JsonKind::Array => {
                    let index = array_index(&name)
                        .ok_or(Failure::reference("reference target unavailable"))?;
                    self.indexed_child(node, index, "required traversal work limit")?
                }
                _ => None,
            };
            let Some(next) = next else {
                return Ok(TargetScope::Outside);
            };
            node = next;
            inside |= node == selected;
            if self.model.nodes[node].deleted {
                return Ok(if inside {
                    TargetScope::Deleted
                } else {
                    TargetScope::Outside
                });
            }
        }
        Ok(if !inside {
            TargetScope::Outside
        } else if self.model.nodes[node].role == Some(Role::Definition(class)) {
            TargetScope::Processed(node)
        } else {
            TargetScope::Unprocessed
        })
    }
    /// Selects a pointer entry's target in the resolved model. For an effective
    /// `syntax` array, it also returns the target's pointer in the requested
    /// document when it has one there. A copied entry into the copied definition
    /// relocates to the importing definition's coordinate, so it selects whatever
    /// declaration the final model has there, following ADR 0036: Copied
    /// reference relocation.
    fn required_selection(
        &mut self,
        source: JsonView<'a>,
        text: &str,
        syntax: ViewNode<'a>,
    ) -> Result<(Selected<'a>, Option<String>), Failure> {
        let ViewNode::Effective(syntax) = syntax else {
            return Ok((self.pointer_source(source, false)?, None));
        };
        let copied_through = self.model.nodes[syntax].context;
        let selected = self.pointer_source(source, copied_through.is_some())?;
        let (selected, relocated) = self.relocate(selected, copied_through)?;
        if let Some((document, pointer)) = relocated {
            return Ok((selected, (document == self.model.source).then_some(pointer)));
        }
        let coordinate = if matches!(selected.target, Element::Composed(_))
            && text.starts_with('#')
            && source.source_id() == self.model.source
        {
            Some(self.decode_fragment(text)?)
        } else {
            None
        };
        Ok((selected, coordinate))
    }
    pub(super) fn collect_required(
        &mut self,
        selected: Option<(ViewNode<'a>, Position)>,
    ) -> Collection<'a> {
        let mut collection = Collection::new();
        let Some((selected, position)) = selected else {
            collection.limited("requested relationship context unavailable");
            return collection;
        };
        let result = (|| {
            let mut pending = Vec::new();
            push(&mut pending, (selected, position), &mut self.allocation)?;
            while let Some((node, position)) = pending.pop() {
                self.required_work(1)?;
                if let ViewNode::Effective(id) = node
                    && self.model.nodes[id].unavailable.is_some()
                {
                    collection.limited("required owner capability unavailable");
                    continue;
                }
                let source = self.required_source(node);
                if source.kind() != JsonKind::Map {
                    if matches!(
                        position,
                        Position::Named(_)
                            | Position::Vocabulary(Role::Definition(
                                Class::Thing
                                    | Class::Object
                                    | Class::Property
                                    | Class::Action
                                    | Class::Event
                            ))
                    ) {
                        collection.limited("required declaration map context unavailable");
                    }
                    continue;
                }
                let syntax = if matches!(position, Position::Vocabulary(_)) {
                    self.required_field(node, "sdfRequired")?
                } else {
                    None
                };
                let class = match position {
                    Position::Vocabulary(Role::Definition(class))
                        if !matches!(class, Class::Data | Class::Context) =>
                    {
                        Some(class)
                    }
                    _ => None,
                };
                if class.is_some() || syntax.is_some() {
                    collection.owners_known = collection
                        .owners_known
                        .checked_add(1)
                        .expect("bounded required observations");
                    self.required_record()?;
                    admit(&mut collection.owners, &mut self.allocation)?;
                    let mut owner = Owner {
                        json: node,
                        source: self.required_evidence(node),
                        class,
                        syntax,
                        state: RequirednessState::Absent,
                        complete: true,
                        references: Vec::new(),
                    };
                    let mut owner_targets = 0u32;
                    let inspected = (|| {
                        if let Some(syntax) = syntax {
                            owner.state = RequirednessState::Available;
                            if class.is_none() {
                                owner.state = RequirednessState::Unavailable;
                                owner.complete = false;
                            }
                            let array = self.required_source(syntax);
                            if array.kind() != JsonKind::Array {
                                if class.is_some() {
                                    collection.errors = collection
                                        .errors
                                        .checked_add(1)
                                        .expect("bounded required findings");
                                    collection.conditions = collection
                                        .conditions
                                        .checked_add(1)
                                        .expect("bounded conditions");
                                    owner.state = RequirednessState::Invalid;
                                } else {
                                    owner.complete = false;
                                }
                            } else {
                                for (occurrence, reference) in
                                    self.required_children(syntax)?.into_iter().enumerate()
                                {
                                    collection.references_known = collection
                                        .references_known
                                        .checked_add(1)
                                        .expect("bounded required observations");
                                    self.required_record()?;
                                    if !charge(&mut self.refs, 1) {
                                        return Err(Failure::limit("required reference limit"));
                                    }
                                    admit(&mut owner.references, &mut self.allocation)?;
                                    let mut row = self.inspect_required_reference(
                                        &owner,
                                        syntax,
                                        reference,
                                        occurrence,
                                        &mut collection,
                                    )?;
                                    if !owner.complete && class.is_none() {
                                        row.status = if matches!(
                                            position,
                                            Position::Vocabulary(Role::Definition(Class::Context))
                                        ) {
                                            RequiredReferenceStatus::UndefinedContext
                                        } else {
                                            RequiredReferenceStatus::UnsupportedContext
                                        };
                                        row.complete = false;
                                        row.targets.clear();
                                    }
                                    match row.status {
                                        RequiredReferenceStatus::WrongTarget
                                        | RequiredReferenceStatus::InvalidSyntax => {
                                            row.error_already_counted =
                                                self.required_error_already_counted(row.source)?;
                                            if !row.error_already_counted {
                                                collection.errors = collection
                                                    .errors
                                                    .checked_add(1)
                                                    .expect("bounded required findings");
                                            }
                                            owner.state = RequirednessState::Invalid;
                                        }
                                        RequiredReferenceStatus::Missing => {
                                            collection.notices = collection
                                                .notices
                                                .checked_add(1)
                                                .expect("bounded required findings")
                                        }
                                        _ => {}
                                    }
                                    if row.status != RequiredReferenceStatus::Resolved
                                        && !row.error_already_counted
                                    {
                                        collection.conditions = collection
                                            .conditions
                                            .checked_add(1)
                                            .expect("bounded required conditions");
                                    }
                                    owner.complete &= row.complete;
                                    owner_targets = owner_targets
                                        .checked_add(
                                            u32::try_from(row.targets.len())
                                                .expect("bounded required targets"),
                                        )
                                        .expect("bounded required targets");
                                    owner.references.push(row);
                                }
                            }
                        }
                        Ok::<_, Failure>(())
                    })();
                    if let Err(reason) = inspected {
                        owner.complete = false;
                        owner.state = RequirednessState::Partial;
                        collection.limited(reason.reason);
                    }
                    if !owner.complete {
                        if owner.state == RequirednessState::Available {
                            owner.state = RequirednessState::Partial;
                        }
                        collection.unfinished("required relationship meaning unavailable");
                    }
                    let owner_references =
                        u32::try_from(owner.references.len()).expect("bounded required references");
                    collection.owners.push(owner);
                    collection.references_retained = collection
                        .references_retained
                        .checked_add(owner_references)
                        .expect("bounded required references");
                    collection.targets_retained = collection
                        .targets_retained
                        .checked_add(owner_targets)
                        .expect("bounded required targets");
                    inspected?;
                }
                for child in self.required_children(node)?.into_iter().rev() {
                    let Some(key) = self.required_name(child) else {
                        collection.limited("required member identity unavailable");
                        continue;
                    };
                    self.required_work(key.len())?;
                    let object = matches!(position, Position::Vocabulary(Role::DirectItem))
                        && self.required_field(node, "type")?.is_some_and(|n| {
                            matches!(self.required_source(n).text(), Field::Present("object"))
                        });
                    let next = position.child(key, object);
                    if !matches!(next, Position::Opaque) {
                        push(&mut pending, (child, next), &mut self.allocation)?;
                    }
                }
            }
            Ok::<_, Failure>(())
        })();
        if let Err(reason) = result {
            collection.limited(reason.reason);
        }
        if !collection.complete && !collection.owners.is_empty() {
            collection.state = RequiredDeclarationsState::Partial;
        }
        collection.relocation_interpretation = self.model.relocation_interpretation;
        collection
    }
    fn inspect_required_reference(
        &mut self,
        owner: &Owner<'a>,
        syntax: ViewNode<'a>,
        reference: ViewNode<'a>,
        occurrence: usize,
        collection: &mut Collection<'a>,
    ) -> Result<Reference<'a>, Failure> {
        let source = self.required_source(reference);
        let mut row = Reference {
            json: reference,
            source,
            occurrence,
            form: None,
            status: RequiredReferenceStatus::Resolved,
            complete: true,
            targets: Vec::new(),
            related: Vec::new(),
            error_already_counted: false,
        };
        if owner.class.is_none() {
            row.status = RequiredReferenceStatus::UnsupportedContext;
            row.complete = false;
            return Ok(row);
        }
        if matches!(source.boolean(), Field::Present(true)) {
            row.form = Some(RequiredReferenceForm::Carrier);
            collection.targets_known = collection
                .targets_known
                .checked_add(1)
                .expect("bounded required targets");
            let target =
                self.required_target(owner.json, owner.class.expect("owner class"), true)?;
            row.complete = target.complete;
            push(&mut row.targets, target, &mut self.allocation)?;
            return Ok(row);
        }
        let Field::Present(text) = source.text() else {
            if source.kind() == JsonKind::String {
                row.status = RequiredReferenceStatus::Unavailable;
                row.complete = false;
            } else {
                row.status = RequiredReferenceStatus::InvalidSyntax;
            }
            return Ok(row);
        };
        self.required_work(text.len() + 1)?;
        if !text.contains(':') && !text.contains('#') {
            row.form = Some(RequiredReferenceForm::DirectName(text));
            for class in [
                Class::Thing,
                Class::Object,
                Class::Property,
                Class::Action,
                Class::Event,
            ] {
                self.required_work(1)?;
                if !owner.class.expect("owner class").allows_child(class) {
                    continue;
                }
                if let Some(group) = self.required_field(owner.json, class.keyword())? {
                    for child in self.required_children(group)? {
                        self.required_work(
                            text.len() + self.required_name(child).map_or(0, str::len) + 1,
                        )?;
                        if self.required_name(child) == Some(text) {
                            collection.targets_known = collection
                                .targets_known
                                .checked_add(1)
                                .expect("bounded required targets");
                            let target = self.required_target(child, class, true)?;
                            row.complete &= target.complete;
                            push(&mut row.targets, target, &mut self.allocation)?;
                        }
                    }
                }
            }
            if matches!(owner.class, Some(Class::Thing | Class::Object))
                && let ViewNode::Source(json) = owner.json
                && self.source_field(json, "sdfRef")?.is_some()
            {
                row.complete = false;
            }
            if row.targets.is_empty()
                && owner
                    .class
                    .is_some_and(|class| class.allows_child(Class::Context))
                && let Some(group) = self.required_field(owner.json, Class::Context.keyword())?
            {
                for child in self.required_children(group)? {
                    self.required_work(
                        text.len() + self.required_name(child).map_or(0, str::len) + 1,
                    )?;
                    if self.required_name(child) == Some(text) {
                        row.status = RequiredReferenceStatus::UndefinedContext;
                        row.complete = false;
                        return Ok(row);
                    }
                }
            }
            if row.targets.is_empty() {
                row.status = if row.complete {
                    RequiredReferenceStatus::Missing
                } else {
                    RequiredReferenceStatus::Partial
                };
                row.complete = false;
            } else if !row.complete {
                row.status = RequiredReferenceStatus::Partial;
            }
            return Ok(row);
        }
        row.form = Some(RequiredReferenceForm::NameReference(text));
        match self.required_selection(source, text, syntax) {
            Ok((selected, coordinate)) => {
                let context = match selected.target {
                    Element::Composed(_) => matches!(
                        selected.position,
                        Position::Vocabulary(Role::Definition(Class::Context))
                    ),
                    Element::Written(target) => matches!(
                        self.source_position(target)?.0,
                        Position::Vocabulary(Role::Definition(Class::Context))
                    ),
                };
                if context {
                    row.status = RequiredReferenceStatus::UndefinedContext;
                    row.complete = false;
                    return Ok(row);
                }
                let class = match selected.target {
                    Element::Composed(_) => {
                        declaration_class(selected.position, selected.enclosing)
                    }
                    Element::Written(target) => self.declaration_position(target)?,
                };
                if let Some(class) = class {
                    let scope = match (owner.json, selected.target, coordinate) {
                        (ViewNode::Source(_), _, _) => TargetScope::Outside,
                        (ViewNode::Effective(_), Element::Written(target), _) => {
                            self.processed_target(target, class)?
                        }
                        // Pointer entries resolve at their coordinate in the resolved
                        // model, following ADR 0036: Copied reference relocation.
                        (ViewNode::Effective(_), Element::Composed(_), Some(pointer)) => {
                            self.processed_coordinate(&pointer, class)?
                        }
                        (ViewNode::Effective(_), Element::Composed(_), None) => {
                            TargetScope::Outside
                        }
                    };
                    if matches!(scope, TargetScope::Deleted) {
                        row.status = RequiredReferenceStatus::Missing;
                        row.complete = false;
                        return Ok(row);
                    }
                    collection.targets_known = collection
                        .targets_known
                        .checked_add(1)
                        .expect("bounded required targets");
                    let addressed = match (scope, selected.target) {
                        (TargetScope::Processed(node), _) => {
                            let mut addressed =
                                self.required_target(ViewNode::Effective(node), class, true)?;
                            addressed.complete = true;
                            addressed
                        }
                        (_, Element::Composed(node)) => {
                            self.required_record()?;
                            // A declaration selected through path composition keeps its
                            // Declaration contribution as source evidence, following
                            // ADR 0026: Required declaration relationships and source
                            // evidence.
                            let evidence = self.model.nodes[node].declaration_source;
                            let (position, _) = self.source_position(evidence)?;
                            let source = matches!(
                                position,
                                Position::Vocabulary(Role::Definition(found)) if found == class
                            )
                            .then_some(evidence);
                            Target {
                                source,
                                effective: None,
                                class,
                                name: self.model.nodes[node].key.unwrap_or(""),
                                complete: source.is_some(),
                                capability_unavailable: matches!(scope, TargetScope::Unprocessed),
                            }
                        }
                        (_, Element::Written(target)) => {
                            self.required_record()?;
                            Target {
                                source: Some(target),
                                effective: None,
                                class,
                                name: target.member_name().present().unwrap_or(""),
                                complete: true,
                                capability_unavailable: matches!(scope, TargetScope::Unprocessed),
                            }
                        }
                    };
                    push(&mut row.targets, addressed, &mut self.allocation)?;
                } else {
                    row.status = RequiredReferenceStatus::WrongTarget;
                }
            }
            Err(reason) if reason.is_limit() => return Err(reason),
            Err(reason) => {
                row.complete = false;
                row.status = if reason.definite() {
                    row.complete = true;
                    RequiredReferenceStatus::InvalidSyntax
                } else if reason.kind == FailureKind::Ambiguous {
                    row.related = std::mem::take(&mut self.pending_conflicts);
                    for related in &row.related {
                        self.unavailable_source(related.source_id())?;
                    }
                    RequiredReferenceStatus::Ambiguous
                } else if reason.kind == FailureKind::Reference
                    && matches!(
                        reason.reason,
                        "reference target unavailable" | "supplied global target unavailable"
                    )
                {
                    RequiredReferenceStatus::Missing
                } else {
                    RequiredReferenceStatus::Unavailable
                };
            }
        }
        Ok(row)
    }
    fn required_error_already_counted(&mut self, source: JsonView<'a>) -> Result<bool, Failure> {
        if self.model.reference_errors == 0 {
            return Ok(false);
        }
        for node in &self.model.nodes {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("required condition work limit"));
            }
            if node.needed_ref.is_some_and(|needed| {
                needed.source_id() == source.source_id() && needed.node == source.node
            }) && node.meaning_unavailable.is_some_and(Failure::definite)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Catalogue {
    pub(crate) fn source_required<'a>(
        &'a self,
        source: SourceId,
        pointer: &str,
        budget: &mut OperationBudget,
        gate: crate::structure::FeatureContext<'a>,
    ) -> Result<(Option<JsonView<'a>>, Collection<'a>, EffectiveModel<'a>), &'static str> {
        let mut resolver = Resolver::new(
            self,
            source,
            budget,
            matches!(gate, crate::structure::FeatureContext::Clear(_)),
            Vec::new(),
            false,
        );
        let selected = self
            .source(source)
            .and_then(|source| source.json().present())
            .map(|root| resolver.select_inspected(root, pointer))
            .transpose();
        let (json, mut collection) = match selected {
            Ok(Some(Some((json, position)))) => {
                let collection = if resolver.model.clear_feature {
                    resolver.collect_required(Some((ViewNode::Source(json), position)))
                } else {
                    let mut collection = Collection::new();
                    collection.limited("required feature context unavailable");
                    collection
                };
                (Some(json), collection)
            }
            Ok(Some(None)) => return Err("unknown inspection selector"),
            Err(reason) if reason.kind == FailureKind::Selector => return Err(reason.reason),
            Err(reason) => {
                let mut collection = Collection::new();
                collection.limited(reason.reason);
                (None, collection)
            }
            _ => {
                let mut collection = Collection::new();
                collection.limited("requested relationship context unavailable");
                (None, collection)
            }
        };
        if json.is_none() {
            collection.complete = false;
        }
        resolver.model.derived_remaining = resolver.allocation;
        resolver.model.work_remaining = resolver.work;
        budget.work = resolver.work;
        budget.allocation = resolver.allocation;
        budget.nodes = resolver.nodes_left;
        budget.references = resolver.refs;
        Ok((json, collection, resolver.model))
    }
}

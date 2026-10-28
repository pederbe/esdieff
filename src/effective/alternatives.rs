//! Bounded semantic alternatives, separate from faithfully composed JSON.
//! Follows ADR 0005: Nullability and presence.
use super::*;
use crate::{DataRole, EffectiveDataDescription};

/// Authored choice identity or the occurrence of an enum value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AlternativeId<'a> {
    Choice(&'a str),
    Enum(usize),
}

pub(super) struct Group {
    owner: usize,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) failure: Option<Failure>,
}
pub(super) struct Row<'a> {
    id: AlternativeId<'a>,
    pub(super) node: usize,
    default_boundary: DefaultBoundary,
}
#[derive(Clone, Copy)]
enum DefaultBoundary {
    Inherited,
    Local,
    Unavailable,
}

/// A known collection retains the identities of individually unavailable branches.
pub struct EffectiveAlternatives<'a> {
    model: &'a EffectiveModel<'a>,
    next: usize,
    end: usize,
}
impl<'a> Iterator for EffectiveAlternatives<'a> {
    type Item = (AlternativeId<'a>, Field<EffectiveDataDescription<'a>>);
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            return None;
        }
        let row = &self.model.alternative_rows[self.next];
        self.next += 1;
        let json = EffectiveJson {
            model: self.model,
            node: row.node,
        };
        let value = if let Some(reason) = json.n().unavailable {
            Field::Unavailable(reason.reason)
        } else if json.kind() != JsonKind::Map {
            Field::Unavailable("alternative data-description map shape")
        } else {
            Field::Present(EffectiveDataDescription(json, DataRole::Full))
        };
        Some((row.id, value))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.end - self.next;
        (left, Some(left))
    }
}
impl ExactSizeIterator for EffectiveAlternatives<'_> {}

impl<'a> EffectiveJson<'a> {
    pub fn explanation(self) -> Explanation<'a> {
        Explanation {
            json: self,
            default: false,
            trailing: None,
        }
    }
    /// Nearest content or deletion origin, without constructing a pointer selector.
    pub fn quality_explanation(self, key: &str) -> Field<Explanation<'a>> {
        if let Some(reason) = self.n().unavailable {
            return Field::Unavailable(reason.reason);
        }
        if self.kind() != JsonKind::Map || key.len() > 8192 {
            return Field::Unavailable("explanation map or key capability");
        }
        let mut work = self.model.catalogue.limits.work;
        let mut next = self.n().first;
        let mut found = None;
        while let Some(node) = next {
            let n = &self.model.nodes[node];
            next = n.next;
            if !charge(&mut work, n.key.map_or(0, str::len) + key.len() + 1) {
                return Field::Unavailable("effective explanation work limit");
            }
            match n.key {
                Some(name) if name == key => found = Some(node),
                None => return Field::Unavailable("map member identity unavailable"),
                _ => {}
            }
        }
        found.map_or(Field::Absent, |node| {
            Field::Present(
                EffectiveJson {
                    model: self.model,
                    node,
                }
                .explanation(),
            )
        })
    }
    /// Established specified Boolean default; authored content is never a default source.
    pub fn default_explanation(self, key: &str) -> Field<Explanation<'a>> {
        let key = match key {
            "nullable" => "nullable",
            "readable" => "readable",
            "writable" => "writable",
            "observable" => "observable",
            _ => return Field::Absent,
        };
        let mut work = self.model.catalogue.limits.work;
        self.model.default_at(self.node, key, &mut work)
    }
    pub(crate) fn semantic_get(self, key: &str) -> Field<Self> {
        match self.get(key) {
            Field::Present(value) => value
                .n()
                .donor_unavailable
                .map_or(Field::Present(value), |reason| {
                    Field::Unavailable(reason.reason)
                }),
            Field::Absent if key == "nullable" && self.context_data() => {
                Field::Unavailable(crate::context::DEFAULT_UNAVAILABLE)
            }
            field => field,
        }
    }
    /// Charged member lookup: pay for every child key scanned, then `semantic_get`.
    pub(crate) fn lookup(self, key: &str, budget: &mut OperationBudget) -> Field<Self> {
        for child in self.children() {
            if !charge(
                &mut budget.work,
                2 * (key.len() + child.n().key.map_or(0, str::len) + 1),
            ) {
                return Field::Unavailable("sample lookup work limit");
            }
        }
        self.semantic_get(key)
    }
    pub(crate) fn alternatives(self) -> Field<EffectiveAlternatives<'a>> {
        if let Some(reason) = self.n().unavailable {
            return Field::Unavailable(reason.reason);
        }
        if let Some(id) = self.n().alternative_group {
            let group = &self.model.alternative_groups[id];
            if let Some(reason) = group.failure {
                return Field::Unavailable(reason.reason);
            }
            return Field::Present(EffectiveAlternatives {
                model: self.model,
                next: group.start,
                end: group.end,
            });
        }
        match (self.get("enum"), self.get("sdfChoice")) {
            (Field::Absent, Field::Absent) => Field::Absent,
            (Field::Unavailable(reason), _) | (_, Field::Unavailable(reason)) => {
                Field::Unavailable(reason)
            }
            _ => Field::Unavailable("alternative context has not been established"),
        }
    }
    pub(crate) fn consistency_meaning_available(self) -> bool {
        match self.n().meaning_unavailable {
            None => true,
            Some(reason) if reason.kind == FailureKind::AlternativeIncomplete => self
                .n()
                .alternative_group
                .and_then(|id| self.model.alternative_groups.get(id))
                .is_some_and(|group| group.owner == self.node && group.failure.is_none()),
            _ => false,
        }
    }
    /// Final alternatives own model meaning; shared source grammar is separate.
    /// Documentary defaults remain stated at their own alternative owner.
    pub(crate) fn model_meaning_needed(self, work: &mut usize) -> Option<bool> {
        let final_alternatives = |node: usize| {
            self.model.nodes[node]
                .alternative_group
                .and_then(|id| self.model.alternative_groups.get(id))
                .is_some_and(|group| group.failure.is_none() && group.start < group.end)
        };
        let mut node = self.node;
        loop {
            // Fixed-size group lookups and the three bounded quality-name tests.
            if !charge(work, 32) {
                return None;
            }
            if node == self.node && final_alternatives(node) {
                return Some(false);
            }
            let n = &self.model.nodes[node];
            let Some(parent) = n.parent else {
                return Some(true);
            };
            if final_alternatives(parent)
                && !matches!(n.key, Some("enum" | "sdfChoice"))
                && !(node == self.node && n.key == Some("default"))
            {
                // Branch copies pass through the collection. Shared qualities,
                // including their original descendants, are assessed in copies.
                return Some(false);
            }
            node = parent;
        }
    }
}
impl<'a> EffectiveModel<'a> {
    pub(crate) fn local_default(&self, node: usize, work: &mut usize) -> Field<bool> {
        for row in &self.alternative_rows {
            if !charge(work, 1) {
                return Field::Unavailable("default boundary work limit");
            }
            if row.node == node {
                return match row.default_boundary {
                    DefaultBoundary::Inherited => Field::Present(false),
                    DefaultBoundary::Local => Field::Present(true),
                    DefaultBoundary::Unavailable => {
                        Field::Unavailable("default contribution boundary unavailable")
                    }
                };
            }
        }
        Field::Present(true)
    }
    // Registry assessments share the original operation budget, including failed
    // groups and excluded rows that the convenience iterator filters internally.
    pub(crate) fn next_assessment_alternative(
        &self,
        cursor: &mut (usize, usize),
        work: &mut usize,
    ) -> Result<Option<usize>, ()> {
        while let Some(group) = self.alternative_groups.get(cursor.0) {
            if cursor.1 == 0 {
                if !charge(work, 1) {
                    return Err(());
                }
                cursor.1 = group.start + 1;
            }
            if group.failure.is_some() || cursor.1 > group.end {
                cursor.0 += 1;
                cursor.1 = 0;
                continue;
            }
            if !charge(work, 1) {
                return Err(());
            }
            let node = self.alternative_rows[cursor.1 - 1].node;
            cursor.1 += 1;
            return Ok(Some(node));
        }
        Ok(None)
    }
    pub(crate) fn alternative_roots(
        &self,
    ) -> impl Iterator<Item = (usize, AlternativeId<'a>)> + '_ {
        self.alternative_groups
            .iter()
            .filter(|group| group.failure.is_none())
            .flat_map(|group| {
                self.alternative_rows[group.start..group.end]
                    .iter()
                    .map(|row| (row.node, row.id))
            })
    }
    pub(crate) fn alternative_scope_unknown(&self, owner: usize) -> bool {
        self.nodes[owner]
            .alternative_group
            .is_none_or(|id| self.alternative_groups[id].failure.is_some())
    }
    pub(crate) fn alternative_owners(&self) -> impl Iterator<Item = EffectiveJson<'_>> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(node, _)| EffectiveJson { model: self, node })
    }
}

impl<'a> Resolver<'a> {
    pub(super) fn derive_alternatives(&mut self) -> Result<(), Failure> {
        let Some(root) = self.model.selected else {
            return Ok(());
        };
        let position = self.model.nodes[root]
            .role
            .map_or(Position::Opaque, Position::Vocabulary);
        let mut jobs = Vec::new();
        push(&mut jobs, (root, position), &mut self.allocation)?;
        while let Some((node, position)) = jobs.pop() {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("alternative traversal work limit"));
            }
            if self.model.nodes[node].sample_skipped
                || self.model.nodes[node].donor_unavailable.is_some()
            {
                continue;
            }
            if let Position::Vocabulary(role) = position {
                self.model.nodes[node].role = Some(role);
            }
            let role = self.model.nodes[node].role;
            let is_data = role.is_some_and(|r| r.full_data() || matches!(r, Role::DirectItem));
            if is_data {
                self.model.nodes[node].alternative_scanned = true;
                match self.derive_group(node) {
                    Ok(Some(group)) => {
                        for row in self.model.alternative_groups[group].start
                            ..self.model.alternative_groups[group].end
                        {
                            push(
                                &mut jobs,
                                (
                                    self.model.alternative_rows[row].node,
                                    Position::Vocabulary(Role::Data),
                                ),
                                &mut self.allocation,
                            )?;
                        }
                    }
                    Ok(None) => {}
                    Err(reason) => {
                        self.model.nodes[node].meaning_unavailable = Some(reason);
                        if let Some(group) = self.model.nodes[node].alternative_group {
                            self.model.alternative_groups[group].failure = Some(reason);
                        }
                    }
                }
            }
            if self.model.nodes[node].unavailable.is_some() {
                continue;
            }
            let object = self.model.nodes[node].source.kind() == JsonKind::Map
                && self
                    .find(node, "type")?
                    .filter(|id| !self.model.nodes[*id].deleted)
                    .is_some_and(|id| {
                        self.model.nodes[id].meaning_unavailable.is_none()
                            && matches!(
                                self.model.nodes[id].source.text(),
                                Field::Present("object")
                            )
                    });
            let mut next = self.model.nodes[node].first;
            while let Some(child) = next {
                let n = &self.model.nodes[child];
                next = n.next;
                if n.deleted || n.sample_skipped || is_data && n.key == Some("sdfChoice") {
                    continue;
                }
                let child_position = if n.meaning_unavailable.is_some() {
                    Position::Opaque
                } else {
                    n.key
                        .map_or(Position::Opaque, |key| position.child(key, object))
                };
                let child_position = if matches!(position, Position::Opaque) {
                    n.role.map_or(child_position, Position::Vocabulary)
                } else {
                    child_position
                };
                push(&mut jobs, (child, child_position), &mut self.allocation)?;
            }
        }
        for group in (0..self.model.alternative_groups.len()).rev() {
            let start = self.model.alternative_groups[group].start;
            let end = self.model.alternative_groups[group].end;
            let mut incomplete = self.model.alternative_groups[group].failure;
            for row in start..end {
                let node = self.model.alternative_rows[row].node;
                self.finish_root_capabilities(node)?;
                if self.model.nodes[node].source.kind() != JsonKind::Map
                    || !self.model.nodes[node].complete
                {
                    incomplete.get_or_insert(Failure::alternative_incomplete());
                }
            }
            if let Some(reason) = incomplete {
                self.model.nodes[self.model.alternative_groups[group].owner]
                    .meaning_unavailable
                    .get_or_insert(reason);
            }
        }
        Ok(())
    }

    pub(super) fn derive_group(&mut self, owner: usize) -> Result<Option<usize>, Failure> {
        if self.model.nodes[owner].unavailable.is_some() {
            return Ok(None);
        }
        let enumeration = self
            .find(owner, "enum")?
            .filter(|id| !self.model.nodes[*id].deleted);
        let choice = self
            .find(owner, "sdfChoice")?
            .filter(|id| !self.model.nodes[*id].deleted);
        if enumeration.is_none() && choice.is_none() {
            return Ok(None);
        }
        let group = self.model.alternative_groups.len();
        push(
            &mut self.model.alternative_groups,
            Group {
                owner,
                start: self.model.alternative_rows.len(),
                end: self.model.alternative_rows.len(),
                failure: None,
            },
            &mut self.allocation,
        )?;
        self.model.nodes[owner].alternative_group = Some(group);
        if enumeration.is_some() && choice.is_some() {
            return Err(Failure::meaning("enum and sdfChoice coexist"));
        }
        let container = enumeration.or(choice).expect("present collection");
        if let Some(reason) = self.model.nodes[container]
            .unavailable
            .or(self.model.nodes[container].meaning_unavailable)
        {
            return Err(reason);
        }
        if enumeration.is_some() {
            if self.model.nodes[container].source.kind() != JsonKind::Array
                || self.model.nodes[container].first.is_none()
            {
                return Err(Failure::meaning(
                    "enum nonempty string-array shape unavailable",
                ));
            }
            let mut next = self.model.nodes[container].first;
            while let Some(value) = next {
                if !charge(&mut self.work, 1) {
                    return Err(Failure::limit("enum establishment work limit"));
                }
                next = self.model.nodes[value].next;
                if self.model.nodes[value].unavailable.is_some()
                    || !matches!(self.model.nodes[value].source.text(), Field::Present(_))
                {
                    return Err(Failure::meaning("enum decoded string value unavailable"));
                }
            }
        } else if self.model.nodes[container].source.kind() != JsonKind::Map {
            return Err(Failure::meaning("choice map shape unavailable"));
        }
        let mut next = self.model.nodes[container].first;
        while let Some(value) = next {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("alternative collection work limit"));
            }
            next = self.model.nodes[value].next;
            if self.model.nodes[value].deleted {
                continue;
            }
            let id = if enumeration.is_some() {
                AlternativeId::Enum(
                    self.model.nodes[value]
                        .index
                        .ok_or(Failure::meaning("enum occurrence identity unavailable"))?,
                )
            } else {
                AlternativeId::Choice(
                    self.model.nodes[value]
                        .key
                        .ok_or(Failure::meaning("choice identity unavailable"))?,
                )
            };
            let (branch, default_boundary) = if enumeration.is_none()
                && (self.model.nodes[value].source.kind() != JsonKind::Map
                    || self.model.nodes[value].unavailable.is_some())
            {
                (value, DefaultBoundary::Unavailable)
            } else {
                self.derive_branch(owner, container, value, enumeration.is_some())?
            };
            push(
                &mut self.model.alternative_rows,
                Row {
                    id,
                    node: branch,
                    default_boundary,
                },
                &mut self.allocation,
            )?;
        }
        self.model.alternative_groups[group].end = self.model.alternative_rows.len();
        Ok(Some(group))
    }

    fn derive_branch(
        &mut self,
        owner: usize,
        container: usize,
        original: usize,
        enumeration: bool,
    ) -> Result<(usize, DefaultBoundary), Failure> {
        let mut default_boundary = DefaultBoundary::Inherited;
        let branch = self.copy_alternative_node(
            if enumeration { owner } else { original },
            Some(container),
            if enumeration {
                None
            } else {
                self.model.nodes[original].key
            },
            if enumeration {
                self.model.nodes[original].index
            } else {
                None
            },
            enumeration,
        )?;
        self.model.nodes[branch].role = Some(Role::Data);
        if enumeration {
            self.model.nodes[branch].changed_context = true;
            self.model.nodes[branch].original_role = None;
        }
        let mut next = self.model.nodes[owner].first;
        while let Some(quality) = next {
            let n = &self.model.nodes[quality];
            next = n.next;
            if n.deleted
                || matches!(
                    n.key,
                    Some("enum" | "sdfChoice" | "readable" | "writable" | "observable")
                )
            {
                continue;
            }
            let key = n
                .key
                .ok_or(Failure::meaning("shared quality identity unavailable"))?;
            self.model.nodes[branch].inherited_context = true;
            let copy = self.copy_alternative(quality, Some(branch), Some(key), None, true)?;
            if self.restricted_donor_unknown(owner, key, quality)? {
                let reason =
                    Failure::meaning("inherited direct-item quality meaning is not established");
                self.model.nodes[copy].meaning_unavailable = Some(reason);
                self.model.nodes[copy].donor_unavailable = Some(reason);
            }
            self.append(branch, copy)?;
        }
        if enumeration {
            let constant =
                self.copy_alternative(original, Some(branch), Some("const"), None, false)?;
            if let Some(existing) = self.find(branch, "const")? {
                self.model.nodes[constant].kind = OriginKind::Override;
                self.replace(existing, constant)?;
            } else {
                self.append(branch, constant)?;
            }
        } else {
            let mut next = self.model.nodes[original].first;
            while let Some(quality) = next {
                let n = &self.model.nodes[quality];
                next = n.next;
                // Actual reference tombstones are absent, allowing shared fallback.
                if n.deleted {
                    continue;
                }
                let key = n
                    .key
                    .ok_or(Failure::meaning("alternative quality identity unavailable"))?;
                if key == "default" {
                    default_boundary = if n.unavailable.is_some()
                        || n.donor_unavailable.is_some()
                        || n.meaning_unavailable.is_some()
                    {
                        DefaultBoundary::Unavailable
                    } else {
                        DefaultBoundary::Local
                    };
                }
                let copy = self.copy_alternative(quality, Some(branch), Some(key), None, false)?;
                if let Some(existing) = self.find(branch, key)? {
                    self.model.nodes[copy].kind = OriginKind::Override;
                    self.replace(existing, copy)?;
                } else {
                    self.append(branch, copy)?;
                }
            }
        }
        if self.sample {
            self.model.nodes[branch].sample_stage = SampleStage::RootExpanded;
            self.model.nodes[branch].sample_active =
                self.model.nodes[if enumeration { owner } else { original }].sample_active;
        }
        Ok((branch, default_boundary))
    }

    fn restricted_donor_unknown(
        &mut self,
        owner: usize,
        key: &str,
        quality: usize,
    ) -> Result<bool, Failure> {
        if !matches!(self.model.nodes[owner].role, Some(Role::DirectItem)) {
            return Ok(false);
        }
        if !charge(&mut self.work, key.len() + 1) {
            return Err(Failure::limit("alternative donor meaning work limit"));
        }
        Ok(match key {
            "description" | "$comment" | "sdfRef" | "minimum" | "maximum" | "format"
            | "minLength" | "maxLength" => false,
            "type" => !matches!(
                self.model.nodes[quality].source.text(),
                Field::Present("number" | "integer" | "string" | "boolean" | "object")
            ),
            "required" | "properties" => !self
                .find(owner, "type")?
                .filter(|id| !self.model.nodes[*id].deleted)
                .is_some_and(|id| {
                    matches!(self.model.nodes[id].source.text(), Field::Present("object"))
                }),
            "const" | "default" | "nullable" | "minItems" | "maxItems" | "uniqueItems"
            | "items" | "exclusiveMinimum" | "exclusiveMaximum" | "multipleOf" | "pattern"
            | "unit" | "sdfType" | "contentFormat" | "label" | "sdfRequired" => true,
            _ => false,
        })
    }

    fn copy_alternative(
        &mut self,
        original: usize,
        parent: Option<usize>,
        key: Option<&'a str>,
        index: Option<usize>,
        inherited: bool,
    ) -> Result<usize, Failure> {
        let root = self.copy_alternative_node(original, parent, key, index, inherited)?;
        let mut frames = Vec::new();
        push(
            &mut frames,
            (root, self.model.nodes[original].first, None::<usize>),
            &mut self.allocation,
        )?;
        while let Some((parent, next, previous)) = frames.last_mut() {
            let Some(original) = *next else {
                frames.pop();
                continue;
            };
            let n = &self.model.nodes[original];
            *next = n.next;
            let (key, index, first) = (n.key, n.index, n.first);
            let parent = *parent;
            let id = self.copy_alternative_node(original, Some(parent), key, index, inherited)?;
            if let Some(previous) = previous {
                self.model.nodes[*previous].next = Some(id);
            } else {
                self.model.nodes[parent].first = Some(id);
            }
            *previous = Some(id);
            push(&mut frames, (id, first, None), &mut self.allocation)?;
        }
        Ok(root)
    }
    pub(super) fn copy_alternative_node(
        &mut self,
        original: usize,
        parent: Option<usize>,
        key: Option<&'a str>,
        index: Option<usize>,
        inherited: bool,
    ) -> Result<usize, Failure> {
        // Exhaustive destructuring keeps new semantic/provenance state from being silently dropped.
        let Node {
            source,
            declaration_source,
            parent: _,
            key: _,
            index: _,
            first: _,
            next: _,
            kind,
            previous,
            deleted,
            unavailable,
            sample_skipped,
            sample_stage: _,
            sample_checked: _,
            sample_active: _,
            meaning_unavailable,
            donor_unavailable,
            context,
            role,
            needed_ref,
            changed_context,
            inherited_context,
            original_role,
            complete: _,
            alternative_group: _,
            alternative_scanned: _,
            consumer,
            ref conflicts,
        } = self.model.nodes[original];
        let mut copied_conflicts = Vec::new();
        for conflict in conflicts {
            if !charge(&mut self.work, 1) {
                return Err(Failure::limit("alternative conflict-copy work limit"));
            }
            push(&mut copied_conflicts, *conflict, &mut self.allocation)?;
        }
        let id = self.shallow(
            source,
            parent,
            key,
            index,
            context,
            if inherited {
                OriginKind::Inherited
            } else {
                kind
            },
        )?;
        let n = &mut self.model.nodes[id];
        n.declaration_source = declaration_source;
        n.previous = previous;
        n.deleted = deleted;
        n.unavailable = unavailable.filter(|reason| {
            !self.sample || !sample_skipped || reason.reason != "outside requested sample context"
        });
        n.sample_skipped = !self.sample && sample_skipped;
        n.meaning_unavailable = meaning_unavailable;
        n.donor_unavailable = donor_unavailable;
        n.role = role;
        n.needed_ref = needed_ref;
        n.changed_context = changed_context;
        n.inherited_context = inherited_context;
        n.original_role = original_role;
        n.complete = false;
        n.consumer = if self.sample {
            consumer.or(n.consumer)
        } else {
            consumer
        };
        n.conflicts = copied_conflicts;
        Ok(id)
    }
}

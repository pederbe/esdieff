//! Existing authored qualities retain source context under ADR 0034, Authored-quality editing
//! with source substitution.
use crate::allowance::{OperationBudget, charge};
use crate::effective::Position;
use crate::json_emit::{JsonWriter, NodePolicy};
use crate::role::Role;
use crate::{
    Acquisition, Assessment, Catalogue, Fidelity, Generation, JsonKind, JsonValue, JsonView,
    Outcome, SourceId, SourceRole,
};
mod checking;
mod location;
mod plan;
mod report;
pub use checking::EditChecking;
pub use location::{EditRequestLocation, EditTarget};

pub const FIDELITY_CONTRACT: &str = "esdieff.authored-quality-edit/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EditOperation {
    Replace,
    Remove,
}
impl EditOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Remove => "remove",
        }
    }
}
/// One present authored member. Null replacement is a value, never removal.
pub struct Edit {
    owner: String,
    quality: String,
    operation: EditOperation,
    value: Option<JsonValue>,
}
impl Edit {
    pub fn replace(owner: &str, quality: &str, value: JsonValue) -> Result<Self, &'static str> {
        Self::new(owner, quality, EditOperation::Replace, Some(value))
    }
    pub fn remove(owner: &str, quality: &str) -> Result<Self, &'static str> {
        Self::new(owner, quality, EditOperation::Remove, None)
    }
    /// Explicit fields permit adapters to reject absent versus present-null mistakes.
    pub fn new(
        owner: &str,
        quality: &str,
        operation: EditOperation,
        value: Option<JsonValue>,
    ) -> Result<Self, &'static str> {
        crate::json::validate_pointer(owner)?;
        if owner.len() > 8192 || quality.len() > 256 {
            return Err("edit selector exceeds byte ceiling");
        }
        if (operation == EditOperation::Replace) != value.is_some() {
            return Err("replace requires value and remove forbids value");
        }
        Ok(Self {
            owner: owner.to_owned(),
            quality: quality.to_owned(),
            operation,
            value,
        })
    }
    pub fn owner(&self) -> &str {
        &self.owner
    }
    pub fn quality(&self) -> &str {
        &self.quality
    }
    pub fn operation(&self) -> EditOperation {
        self.operation
    }
    pub fn replacement(&self) -> Option<crate::JsonValueView<'_>> {
        self.value.as_ref().map(JsonValue::view)
    }
}

/// Owns all supplied attempts and explicit editing intent.
pub struct EditRequest {
    catalogue: Catalogue,
    key: String,
    selected: SourceId,
    generated_key: String,
    label: Option<String>,
    edits: Vec<Edit>,
    plan: Option<crate::Source>,
    metadata: usize,
    plan_incomplete: Option<&'static str>,
    request_budget: Option<OperationBudget>,
    checking: Option<Assessment>,
    strict: bool,
}
impl EditRequest {
    pub fn new(
        catalogue: Catalogue,
        key: &str,
        selected: SourceId,
        generated_key: &str,
    ) -> Result<Self, &'static str> {
        if key.len() > 256 || generated_key.len() > 256 {
            return Err("edit request/source key exceeds 256 bytes");
        }
        if catalogue.attempt_count() >= catalogue.limits.documents {
            return Err("generated attempt exceeds source-attempt ceiling");
        }
        if catalogue
            .retained_sources()
            .any(|source| source.key == generated_key)
        {
            return Err("generated key must be new");
        }
        if catalogue
            .source(selected)
            .is_none_or(|source| !source.role.is_model())
        {
            return Err("edit source must be a supplied model source");
        }
        let metadata = key.len() + generated_key.len();
        validate_metadata(&catalogue, metadata)?;
        Ok(Self {
            catalogue,
            key: key.to_owned(),
            selected,
            generated_key: generated_key.to_owned(),
            label: None,
            edits: Vec::new(),
            plan: None,
            metadata,
            plan_incomplete: None,
            request_budget: None,
            checking: None,
            strict: false,
        })
    }
    fn charge_metadata(&mut self, amount: usize) -> Result<(), &'static str> {
        let next = self
            .metadata
            .checked_add(amount)
            .ok_or("edit metadata overflow")?;
        validate_metadata(&self.catalogue, next)?;
        self.admit_check_reporting(next)?;
        self.metadata = next;
        Ok(())
    }
    pub fn generated_label(mut self, label: &str) -> Result<Self, &'static str> {
        if label.len() > 512 || self.label.is_some() {
            return Err("one bounded generated label required");
        }
        self.charge_metadata(label.len())?;
        self.label = Some(label.to_owned());
        Ok(self)
    }
    pub fn with_edit(mut self, edit: Edit) -> Result<Self, &'static str> {
        self.admit_edit(edit, false)?;
        Ok(self)
    }
    fn admit_edit(&mut self, edit: Edit, precharged: bool) -> Result<(), &'static str> {
        if self.edits.len() >= 256 {
            return Err("at most 256 edits required");
        }
        if edit.owner.len() > self.catalogue.limits.pointer_bytes {
            return Err("edit owner exceeds configured pointer ceiling");
        }
        if self
            .edits
            .iter()
            .any(|existing| existing.owner == edit.owner && existing.quality == edit.quality)
        {
            return Err("duplicate decoded edit target");
        }
        self.charge_metadata(edit.owner.len() + edit.quality.len() + 32)?;
        if let Some(value) = &edit.value
            && !precharged
        {
            let (nodes, storage, depth) = value.author_storage();
            if depth > self.catalogue.limits.depth {
                return Err("replacement depth exceeds configured ceiling");
            }
            self.catalogue.parsed_nodes_remaining = self
                .catalogue
                .parsed_nodes_remaining
                .checked_sub(nodes)
                .ok_or("edit replacement parsed node ceiling")?;
            self.catalogue.parsed_allocation_remaining = self
                .catalogue
                .parsed_allocation_remaining
                .checked_sub(storage)
                .ok_or("edit replacement parsed allocation ceiling")?;
        }
        self.edits.push(edit);
        Ok(())
    }
    pub fn key(&self) -> &str {
        &self.key
    }
    /// Request one ordinary assessment of the entire edited source.
    pub fn checking(mut self, assessment: Assessment) -> Result<Self, &'static str> {
        if self.checking.is_some()
            || !matches!(assessment, Assessment::Initial | Assessment::FullBaseline)
        {
            return Err("one edit check initial-v1 or full-baseline required");
        }
        self.checking = Some(assessment);
        self.admit_check_reporting(self.metadata)?;
        Ok(self)
    }
    pub fn strict_warnings(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
    fn admit_check_reporting(&self, metadata: usize) -> Result<(), &'static str> {
        if let Some(assessment) = self.checking {
            let reserve =
                self.catalogue.reserve + metadata * 6 + 20480 + checking::MANDATORY_RESERVE;
            let profile = crate::assessment::profile(assessment);
            if reserve + profile.report_reserve() > self.catalogue.limits.report_bytes {
                return Err("report cannot reserve combined edit and checking envelope");
            }
            profile.admit_reporting(reserve, self.catalogue.limits.report_bytes, false)?;
        }
        Ok(())
    }
    pub fn selected_source(&self) -> SourceId {
        self.selected
    }
    pub fn edits(&self) -> impl ExactSizeIterator<Item = &Edit> {
        self.edits.iter()
    }
    pub fn plan_text(&self) -> Option<&str> {
        self.plan.as_ref().and_then(crate::Source::text)
    }

    pub fn edit(mut self) -> Result<EditResult, &'static str> {
        if self.edits.is_empty() {
            return Err("one through 256 edits required");
        }
        self.admit_check_reporting(self.metadata)?;
        self.catalogue.reserve += self.metadata * 6 + 20480;
        if self.checking.is_some() {
            self.catalogue.reserve += checking::MANDATORY_RESERVE;
        }
        let generated = SourceId((self.catalogue.attempt_count() + 1) as u32);
        let mut budget = self
            .request_budget
            .take()
            .unwrap_or_else(|| OperationBudget::new(&self.catalogue.limits));
        budget.work = budget.work.min(self.catalogue.decoding_work_remaining);
        let target_storage = self.edits.len()
            * (std::mem::size_of::<Option<Target>>() + std::mem::size_of::<(usize, usize)>());
        let source = self
            .catalogue
            .source(self.selected)
            .expect("selected source");
        let root = source.json().present();
        let mut conditions = EditConditions::default();
        let mut targets = Vec::new();
        let mut established = true;
        if let Some(reason) = self.plan_incomplete {
            conditions.push(EditCondition::incomplete("edit.plan_limit", reason, 0));
            established = false;
        } else if !charge(&mut budget.allocation, target_storage)
            || !charge(&mut budget.work, self.edits.len())
        {
            conditions.push(EditCondition::incomplete(
                "edit.target_limit",
                "Edit target storage or work limit",
                0,
            ));
            established = false;
        } else if !source.complete
            || source.parsed.errors > 0
            || root.is_none_or(|root| !root.capability_complete())
        {
            conditions.push(EditCondition::incomplete(
                "edit.source_unavailable",
                "Selected authored source is unreadable or incompletely decoded",
                0,
            ));
            established = false;
        } else {
            targets.reserve_exact(self.edits.len());
            for (index, edit) in self.edits.iter().enumerate() {
                match establish(root.expect("source root"), edit, &mut budget) {
                    Ok(target) => targets.push(Some(target)),
                    Err(Establishment::Request(reason)) => return Err(reason),
                    Err(Establishment::Incomplete(reason)) => {
                        conditions.push(EditCondition::incomplete(
                            "edit.target_limit",
                            reason,
                            index,
                        ));
                        targets.push(None);
                        established = false;
                        break;
                    }
                }
            }
        }
        let mut order = Vec::with_capacity(targets.len());
        for (index, target) in targets.iter().enumerate() {
            if let Some(target) = target {
                order.push((target.member, index));
            }
        }
        if !charge(&mut budget.work, order.len() * 8 + 1) {
            established = false;
            conditions.push(EditCondition::incomplete(
                "edit.target_limit",
                "Edit target ordering work limit",
                0,
            ));
        }
        order.sort_unstable();
        let mut bytes = Vec::new();
        let mut emitted = false;
        if established {
            let cap = self.catalogue.limits.document_bytes.min(
                self.catalogue
                    .limits
                    .catalogue_bytes
                    .saturating_sub(self.catalogue.retained_bytes()),
            );
            let mut writer = Writer {
                output: JsonWriter::new(
                    cap,
                    &mut budget,
                    NodePolicy::Edit {
                        depth: self.catalogue.limits.depth,
                    },
                ),
                edits: &self.edits,
                order: &order,
            };
            let emission = writer
                .source(root.expect("source root"), 0)
                .and_then(|()| writer.output.emit("\n"));
            bytes = writer.output.into_bytes();
            match emission {
                Ok(()) => emitted = true,
                Err(reason) => conditions.push(EditCondition::incomplete(
                    "edit.generation_limit",
                    reason,
                    0,
                )),
            }
        }
        // The decode allowance stays lent through the preservation check, so the work it
        // spends settles into the catalogue together with the decoding itself.
        let (generated_source, generation, fidelity) =
            self.catalogue.lend_decoding(|catalogue, decoding| {
                decoding.work = decoding.work.min(budget.work);
                let parsed = if emitted {
                    crate::json::parse(&bytes, true, &catalogue.limits, decoding)
                } else {
                    crate::json::Parsed::unavailable()
                };
                budget.work = budget.work.min(decoding.work);
                let usable =
                    emitted && parsed.root.is_some() && !parsed.rejected && parsed.errors == 0;
                let generation = if usable {
                    Generation::Complete
                } else if bytes.is_empty() {
                    Generation::Unavailable
                } else {
                    Generation::Partial
                };
                if emitted && !usable {
                    conditions.push(EditCondition::incomplete(
                        "edit.generated_decode_limit",
                        "Generated decoding could not complete",
                        0,
                    ));
                }
                let generated_source = crate::Source {
                    role: SourceRole::Generated,
                    original: true,
                    id: generated,
                    key: self.generated_key.clone(),
                    bytes,
                    label: self.label.clone(),
                    uri: None,
                    version: None,
                    digest: None,
                    raw_path: None,
                    acquisition: Acquisition::Generated,
                    label_omitted: false,
                    complete: emitted,
                    parsed,
                };
                let mut fidelity = Fidelity::Undetermined;
                if usable {
                    // Re-read the original root here: the outer borrow cannot outlive the lend.
                    let root = catalogue
                        .source(self.selected)
                        .expect("selected source")
                        .json()
                        .present()
                        .expect("source root");
                    match preserve(
                        root,
                        generated_source.json().require()?,
                        &self.edits,
                        &order,
                        &mut budget,
                    ) {
                        Ok(true) => fidelity = Fidelity::Established,
                        Ok(false) => conditions.push(EditCondition {
                            code: "edit.preservation_failed",
                            message: "Generated changes or unchanged authored content differ from the request",
                            index: 0,
                            error: true,
                        }),
                        Err(reason) => conditions.push(EditCondition::incomplete(
                            "edit.fidelity_limit",
                            reason,
                            0,
                        )),
                    }
                }
                decoding.work = decoding.work.min(budget.work);
                Ok::<_, &'static str>((generated_source, generation, fidelity))
            })?;
        let source_index = self
            .catalogue
            .sources
            .iter()
            .position(|source| source.id == self.selected)
            .expect("original source index");
        let original = self.catalogue.sources.remove(source_index);
        self.catalogue.evidence.push(original);
        self.catalogue.sources.push(generated_source);
        let checking = self.checking.map(|assessment| {
            match self.catalogue.check_with_budget(
                crate::CheckRequest::new()
                    .root(generated)
                    .assessment(assessment)
                    .strict_warnings(self.strict),
                &mut budget,
            ) {
                Ok(report) => checking::Checking::from_report(report),
                Err(_) => checking::Checking::unavailable(assessment, generated, self.strict),
            }
        });
        let errors = conditions.errors
            + checking
                .as_ref()
                .map_or(0, |check| check.outcome().errors());
        let warnings = checking
            .as_ref()
            .map_or(0, |check| check.outcome().warnings());
        let notices = checking
            .as_ref()
            .map_or(0, |check| check.outcome().notices());
        let complete = generation == Generation::Complete
            && fidelity == Fidelity::Established
            && checking
                .as_ref()
                .is_none_or(|check| check.outcome().complete());
        let outcome =
            crate::report::author_outcome(complete, errors, warnings, notices, self.strict);
        Ok(EditResult {
            request: self,
            generated,
            generation,
            fidelity,
            targets,
            conditions,
            outcome,
            checking,
        })
    }
}

#[derive(Clone, Copy)]
struct Target {
    owner: usize,
    member: usize,
}
struct EditCondition {
    code: &'static str,
    message: &'static str,
    index: usize,
    error: bool,
}
// Fixed operation statuses use the pre-reserved mandatory envelope even when
// derived allocation has been exhausted. Optional report detail stays bounded.
#[derive(Default)]
struct EditConditions {
    retained: [Option<EditCondition>; 5],
    count: usize,
    errors: u32,
}
impl EditConditions {
    fn push(&mut self, condition: EditCondition) {
        self.errors += u32::from(condition.error);
        let slot = self
            .retained
            .get_mut(self.count)
            .expect("bounded editing phases");
        *slot = Some(condition);
        self.count += 1;
    }
    fn iter(&self) -> impl Iterator<Item = &EditCondition> {
        self.retained[..self.count].iter().flatten()
    }
}
impl EditCondition {
    fn incomplete(code: &'static str, message: &'static str, index: usize) -> Self {
        Self {
            code,
            message,
            index,
            error: false,
        }
    }
}
enum Establishment {
    Request(&'static str),
    Incomplete(&'static str),
}
fn validate_metadata(catalogue: &Catalogue, request_metadata: usize) -> Result<(), &'static str> {
    let source_metadata: usize = catalogue
        .retained_sources()
        .map(|s| {
            s.key.len()
                + s.label.as_ref().map_or(0, String::len)
                + s.uri.as_ref().map_or(0, String::len)
                + s.version.as_ref().map_or(0, String::len)
                + s.digest.as_ref().map_or(0, String::len)
                + s.raw_path.as_ref().map_or(0, Vec::len)
        })
        .sum();
    if source_metadata + request_metadata > 1024 * 1024 {
        return Err("caller metadata exceeds whole-request budget");
    }
    if catalogue.reserve + request_metadata * 6 + 20480 > catalogue.limits.report_bytes {
        return Err("report cannot reserve edit identity/substitution envelope");
    }
    Ok(())
}
fn field<'a>(
    map: JsonView<'a>,
    key: &str,
    budget: &mut OperationBudget,
) -> Result<Option<JsonView<'a>>, &'static str> {
    let mut found = None;
    for child in map.children() {
        let name = child.member_name().require()?;
        if !charge(&mut budget.work, name.len() + key.len() + 1) {
            return Err("Authored target lookup work limit");
        }
        if name == key {
            found = Some(child);
        }
    }
    Ok(found)
}
fn establish(
    root: JsonView<'_>,
    edit: &Edit,
    budget: &mut OperationBudget,
) -> Result<Target, Establishment> {
    let mut current = root;
    let mut position = Position::Vocabulary(Role::Document);
    for token in edit
        .owner
        .strip_prefix('/')
        .into_iter()
        .flat_map(|tail| tail.split('/'))
    {
        if let Position::Vocabulary(role) = position
            && role.reference_context()
            && field(current, "sdfRef", budget)
                .map_err(Establishment::Incomplete)?
                .is_some()
        {
            return Err(Establishment::Request(
                "target is within an authored sdfRef composition boundary",
            ));
        }
        if current.kind() != JsonKind::Map {
            return Err(Establishment::Request(
                "edit owner is not an authored named definition",
            ));
        }
        if !charge(&mut budget.work, token.len() * 4 + 1)
            || !charge(&mut budget.allocation, token.len() * 2)
        {
            return Err(Establishment::Incomplete(
                "Authored selector work or allocation limit",
            ));
        }
        let token = token.replace("~1", "/").replace("~0", "~");
        let object = matches!(position, Position::Vocabulary(Role::DirectItem))
            && field(current, "type", budget)
                .map_err(Establishment::Incomplete)?
                .is_some_and(|value| value.text().present() == Some("object"));
        position = position.child(&token, object);
        current = field(current, &token, budget)
            .map_err(Establishment::Incomplete)?
            .ok_or(Establishment::Request("edit owner does not exist"))?;
    }
    let Position::Vocabulary(Role::Definition(class)) = position else {
        return Err(Establishment::Request(
            "edit owner must be an authored base named definition",
        ));
    };
    if current.kind() != JsonKind::Map {
        return Err(Establishment::Request(
            "edit owner must be a present definition map",
        ));
    }
    if field(current, "sdfRef", budget)
        .map_err(Establishment::Incomplete)?
        .is_some()
    {
        return Err(Establishment::Request(
            "target is within an authored sdfRef composition boundary",
        ));
    }
    let common = matches!(edit.quality.as_str(), "label" | "description" | "$comment");
    let data = matches!(class, crate::Class::Property | crate::Class::Data)
        && matches!(
            edit.quality.as_str(),
            "type"
                | "sdfType"
                | "const"
                | "default"
                | "enum"
                | "minimum"
                | "maximum"
                | "exclusiveMinimum"
                | "exclusiveMaximum"
                | "multipleOf"
                | "minLength"
                | "maxLength"
                | "pattern"
                | "format"
                | "minItems"
                | "maxItems"
                | "uniqueItems"
                | "nullable"
                | "contentFormat"
                | "unit"
        );
    if !common && !data {
        return Err(Establishment::Request(
            "unsupported editing Quality Name for owner role",
        ));
    }
    let member = field(current, &edit.quality, budget)
        .map_err(Establishment::Incomplete)?
        .ok_or(Establishment::Request(
            "edit quality does not exist in original owner",
        ))?;
    Ok(Target {
        owner: current.node,
        member: member.node,
    })
}
fn operation_for(
    node: usize,
    order: &[(usize, usize)],
    work: &mut usize,
) -> Result<Option<usize>, &'static str> {
    let mut low = 0;
    let mut high = order.len();
    while low < high {
        if !charge(work, 1) {
            return Err("Edit transformation work limit");
        }
        let mid = low + (high - low) / 2;
        match order[mid].0.cmp(&node) {
            std::cmp::Ordering::Less => low = mid + 1,
            std::cmp::Ordering::Greater => high = mid,
            std::cmp::Ordering::Equal => return Ok(Some(order[mid].1)),
        }
    }
    Ok(None)
}
struct Writer<'a> {
    output: JsonWriter<'a>,
    edits: &'a [Edit],
    order: &'a [(usize, usize)],
}
impl Writer<'_> {
    fn source(&mut self, value: JsonView<'_>, depth: usize) -> Result<(), &'static str> {
        self.output.start(depth)?;
        match value.kind() {
            JsonKind::Map | JsonKind::Array => {
                let map = value.kind() == JsonKind::Map;
                self.output.emit(if map { "{" } else { "[" })?;
                let mut any = false;
                for child in value.children() {
                    let edit = operation_for(child.node, self.order, self.output.work())?
                        .map(|index| &self.edits[index]);
                    if edit.is_some_and(|edit| edit.operation == EditOperation::Remove) {
                        continue;
                    }
                    if any {
                        self.output.emit(",")?;
                    }
                    self.output.emit("\n")?;
                    self.output.indent(depth + 1)?;
                    if map {
                        self.output.quoted(child.member_name().require()?)?;
                        self.output.emit(": ")?;
                    }
                    if let Some(edit) = edit {
                        self.output.value(
                            edit.value.as_ref().expect("replace value").view(),
                            depth + 1,
                        )?;
                    } else {
                        self.source(child, depth + 1)?;
                    }
                    any = true;
                }
                if any {
                    self.output.emit("\n")?;
                    self.output.indent(depth)?;
                }
                self.output.emit(if map { "}" } else { "]" })
            }
            JsonKind::String => self.output.quoted(value.text().require()?),
            JsonKind::Number => self.output.emit(value.number().require()?.lexeme()),
            JsonKind::Boolean => self.output.emit(if value.boolean().require()? {
                "true"
            } else {
                "false"
            }),
            JsonKind::Null => self.output.emit("null"),
        }
    }
}
fn preserve(
    original: JsonView<'_>,
    generated: JsonView<'_>,
    edits: &[Edit],
    order: &[(usize, usize)],
    budget: &mut OperationBudget,
) -> Result<bool, &'static str> {
    if !charge(&mut budget.work, 1) {
        return Err("Editing fidelity traversal work limit");
    }
    if original.kind() != generated.kind() {
        return Ok(false);
    }
    match original.kind() {
        JsonKind::Map | JsonKind::Array => {
            let mut expected = 0;
            for child in original.children() {
                let edit =
                    operation_for(child.node, order, &mut budget.work)?.map(|index| &edits[index]);
                if edit.is_some_and(|edit| edit.operation == EditOperation::Remove) {
                    continue;
                }
                let candidate = if original.kind() == JsonKind::Map {
                    field(generated, child.member_name().require()?, budget)?
                } else {
                    let mut found = None;
                    for entry in generated.children() {
                        if !charge(&mut budget.work, 1) {
                            return Err("Editing fidelity array lookup work limit");
                        }
                        if entry.n().index == child.n().index {
                            found = Some(entry);
                            break;
                        }
                    }
                    found
                };
                let Some(candidate) = candidate else {
                    return Ok(false);
                };
                if let Some(edit) = edit {
                    if !replacement_equal(
                        edit.value.as_ref().expect("replace value").view(),
                        candidate,
                        budget,
                    )? {
                        return Ok(false);
                    }
                } else if !preserve(child, candidate, edits, order, budget)? {
                    return Ok(false);
                }
                expected += 1;
            }
            let mut actual = 0;
            for _ in generated.children() {
                if !charge(&mut budget.work, 1) {
                    return Err("Editing fidelity member-count work limit");
                }
                actual += 1;
            }
            Ok(expected == actual)
        }
        JsonKind::String => equal_text(
            original.text().require()?,
            generated.text().require()?,
            budget,
        ),
        JsonKind::Number => equal_text(
            original.number().require()?.lexeme(),
            generated.number().require()?.lexeme(),
            budget,
        ),
        JsonKind::Boolean => Ok(original.boolean().require()? == generated.boolean().require()?),
        JsonKind::Null => Ok(true),
    }
}
fn equal_text(a: &str, b: &str, budget: &mut OperationBudget) -> Result<bool, &'static str> {
    if !charge(&mut budget.work, a.len() + b.len() + 1) {
        return Err("Editing fidelity scalar work limit");
    }
    Ok(a == b)
}
fn replacement_equal(
    value: crate::JsonValueView<'_>,
    generated: JsonView<'_>,
    budget: &mut OperationBudget,
) -> Result<bool, &'static str> {
    if !charge(&mut budget.work, 1) {
        return Err("Replacement fidelity work limit");
    }
    if value.kind() != generated.kind() {
        return Ok(false);
    }
    match value.kind() {
        JsonKind::Map | JsonKind::Array => {
            let mut expected = 0;
            for child in value.children() {
                let candidate = if value.kind() == JsonKind::Map {
                    field(generated, child.member_name().require()?, budget)?
                } else {
                    let mut found = None;
                    for entry in generated.children() {
                        if !charge(&mut budget.work, 1) {
                            return Err("Replacement fidelity lookup work limit");
                        }
                        if entry.n().index == Some(expected) {
                            found = Some(entry);
                            break;
                        }
                    }
                    found
                };
                let Some(candidate) = candidate else {
                    return Ok(false);
                };
                if !replacement_equal(child, candidate, budget)? {
                    return Ok(false);
                }
                expected += 1;
            }
            let mut actual = 0;
            for _ in generated.children() {
                if !charge(&mut budget.work, 1) {
                    return Err("Replacement fidelity count work limit");
                }
                actual += 1;
            }
            Ok(expected == actual)
        }
        JsonKind::String => {
            equal_text(value.text().require()?, generated.text().require()?, budget)
        }
        JsonKind::Number => equal_text(
            value.number().require()?.lexeme(),
            generated.number().require()?.lexeme(),
            budget,
        ),
        JsonKind::Boolean => Ok(value.boolean().require()? == generated.boolean().require()?),
        JsonKind::Null => Ok(true),
    }
}
pub struct EditResult {
    request: EditRequest,
    generated: SourceId,
    generation: Generation,
    fidelity: Fidelity,
    targets: Vec<Option<Target>>,
    conditions: EditConditions,
    outcome: Outcome,
    checking: Option<checking::Checking>,
}
impl EditResult {
    pub fn catalogue(&self) -> &Catalogue {
        &self.request.catalogue
    }
    pub fn into_catalogue(self) -> Catalogue {
        self.request.catalogue
    }
    pub fn request(&self) -> &EditRequest {
        &self.request
    }
    pub fn original_source(&self) -> &crate::Source {
        self.catalogue()
            .evidence
            .iter()
            .find(|source| source.id == self.request.selected)
            .expect("original evidence")
    }
    pub fn generated_source(&self) -> SourceId {
        self.generated
    }
    pub fn generated_document(&self) -> Option<&[u8]> {
        let bytes = self.catalogue().source(self.generated)?.raw_bytes();
        (!bytes.is_empty()).then_some(bytes)
    }
    pub fn generation(&self) -> Generation {
        self.generation
    }
    pub fn fidelity(&self) -> Fidelity {
        self.fidelity
    }
    pub fn fidelity_contract(&self) -> &'static str {
        FIDELITY_CONTRACT
    }
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
    pub fn checking(&self) -> EditChecking<'_> {
        EditChecking::new(self.checking.as_ref())
    }
    pub fn requested_coverage(&self) -> crate::RequestedCoverage {
        crate::report::edit_coverage(self.outcome.complete())
    }
    pub fn document_deliverable(&self) -> bool {
        self.generation == Generation::Complete
            && self.fidelity == Fidelity::Established
            && self.outcome.disposition() == crate::Disposition::Passed
    }
    pub fn diagnostics(&self) -> crate::Diagnostics {
        crate::report::author_diagnostics(report::wire(self)["diagnostics"].take())
    }
    pub fn to_json(&self) -> String {
        report::to_json(self)
    }
    pub fn to_human(&self) -> String {
        self.to_human_with_style(crate::HumanStyle::Plain)
    }
    pub fn to_human_with_style(&self, style: crate::HumanStyle) -> String {
        report::to_human(self, style)
    }
}

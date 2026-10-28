//! Reference-preserving new sdfThing authoring follows ADR 0017, Reference-preserving sdfThing
//! authoring.
use crate::allowance::OperationBudget;
use crate::effective::Position;
use crate::json_emit::{JsonWriter, NodePolicy};
use crate::role::Role;
use crate::{
    Acquisition, Assessment, Catalogue, CheckRequest, JsonKind, JsonValue, JsonView, Outcome,
    SourceId, SourceRole,
};
mod origin;
mod report;
pub use origin::AuthorRequestLocation;
pub use report::AuthorChecking;

/// State of the requested document assembly, independently of checking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Generation {
    Complete,
    Partial,
    Unavailable,
}
impl Generation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Fidelity {
    Established,
    Undetermined,
}
impl Fidelity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Established => "established",
            Self::Undetermined => "undetermined",
        }
    }
}

/// One explicitly named, selected reusable sdfObject declaration.
pub struct AuthorObject {
    name: String,
    source: SourceId,
    pointer: String,
    prefix: String,
    refinement: Option<JsonValue>,
    required: bool,
}
impl AuthorObject {
    pub fn new(
        name: &str,
        source: SourceId,
        pointer: &str,
        prefix: &str,
        required: bool,
    ) -> Result<Self, &'static str> {
        name_field(name, false)?;
        name_field(prefix, true)?;
        crate::json::validate_pointer(pointer)?;
        Ok(Self {
            name: name.to_owned(),
            source,
            pointer: pointer.to_owned(),
            prefix: prefix.to_owned(),
            refinement: None,
            required,
        })
    }
    pub fn refinement(mut self, patch: JsonValue) -> Result<Self, &'static str> {
        if self.refinement.is_some() {
            return Err("one refinement setting is allowed");
        }
        if patch.view().kind() != JsonKind::Map || patch.view().get("sdfRef").present().is_some() {
            return Err("refinement must be a map without an owned top-level sdfRef");
        }
        self.refinement = Some(patch);
        Ok(self)
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn source(&self) -> SourceId {
        self.source
    }
    pub fn pointer(&self) -> &str {
        &self.pointer
    }
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
    pub fn required(&self) -> bool {
        self.required
    }
    pub fn refinement_view(&self) -> Option<crate::JsonValueView<'_>> {
        self.refinement.as_ref().map(JsonValue::view)
    }
}

/// Owns supplied sources and explicit naming intent; no source is edited.
pub struct AuthorRequest {
    catalogue: Catalogue,
    key: String,
    generated_key: String,
    label: Option<String>,
    thing: String,
    namespaces: Vec<(String, String)>,
    default: Option<String>,
    info: Option<JsonValue>,
    qualities: Option<JsonValue>,
    objects: Vec<AuthorObject>,
    checking: Option<Assessment>,
    strict: bool,
    plan: Option<crate::Source>,
    metadata: usize,
}
impl AuthorRequest {
    pub fn new(
        catalogue: Catalogue,
        key: &str,
        generated_key: &str,
        thing_name: &str,
    ) -> Result<Self, &'static str> {
        if key.len() > 256 || generated_key.len() > 256 {
            return Err("author request/source key exceeds 256 bytes");
        }
        name_field(thing_name, false)?;
        if catalogue.attempt_count() >= catalogue.limits.documents {
            return Err("generated source reservation exceeds source-attempt ceiling");
        }
        if catalogue
            .retained_sources()
            .any(|source| source.key == generated_key)
        {
            return Err("generated key must be new");
        }
        let mut request = Self {
            catalogue,
            key: key.to_owned(),
            generated_key: generated_key.to_owned(),
            label: None,
            thing: thing_name.to_owned(),
            namespaces: Vec::new(),
            default: None,
            info: None,
            qualities: None,
            objects: Vec::new(),
            checking: None,
            strict: false,
            plan: None,
            metadata: 0,
        };
        request.charge_metadata(key.len() + generated_key.len() + thing_name.len())?;
        Ok(request)
    }
    fn charge_metadata(&mut self, amount: usize) -> Result<(), &'static str> {
        let source_metadata: usize = self
            .catalogue
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
        let next = self
            .metadata
            .checked_add(amount)
            .ok_or("author metadata overflow")?;
        if source_metadata + next > 1024 * 1024 {
            return Err("caller metadata exceeds whole-request budget");
        }
        if self.catalogue.reserve + next * 6 + 16384 > self.catalogue.limits.report_bytes {
            return Err("report cannot reserve author request envelope");
        }
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
    pub fn namespace(mut self, prefix: &str, uri: &str) -> Result<Self, &'static str> {
        name_field(prefix, true)?;
        if uri.len() > 2048 || !crate::uri::valid_uri(uri) {
            return Err("bounded absolute namespace URI required");
        }
        if self.namespaces.len() >= 256 || self.namespaces.iter().any(|(p, _)| p == prefix) {
            return Err("duplicate or too many namespace bindings");
        }
        self.charge_metadata(prefix.len() + uri.len())?;
        self.namespaces.push((prefix.to_owned(), uri.to_owned()));
        Ok(self)
    }
    pub fn default_namespace(mut self, prefix: &str) -> Result<Self, &'static str> {
        name_field(prefix, true)?;
        if self.default.is_some() {
            return Err("one default namespace setting required");
        }
        self.charge_metadata(prefix.len())?;
        self.default = Some(prefix.to_owned());
        Ok(self)
    }
    pub fn info(mut self, info: JsonValue) -> Result<Self, &'static str> {
        if self.info.is_some() || info.view().kind() != JsonKind::Map {
            return Err("one information map required");
        }
        self.info = Some(info);
        Ok(self)
    }
    pub fn thing_qualities(mut self, qualities: JsonValue) -> Result<Self, &'static str> {
        if self.qualities.is_some() || qualities.view().kind() != JsonKind::Map {
            return Err("one sdfThing common-quality map required");
        }
        if qualities.view().children().any(|v| {
            v.member_name().present().is_some_and(|key| {
                crate::role::class(key).is_some() || matches!(key, "sdfRef" | "sdfRequired")
            })
        }) {
            return Err("sdfThing qualities cannot replace owned declarations or references");
        }
        self.qualities = Some(qualities);
        Ok(self)
    }
    pub fn object(mut self, object: AuthorObject) -> Result<Self, &'static str> {
        if self.objects.len() >= 256 || self.objects.iter().any(|o| o.name == object.name) {
            return Err("duplicate or too many authored declarations");
        }
        if self
            .catalogue
            .source(object.source)
            .is_none_or(|s| !s.role.is_model())
        {
            return Err("unknown reusable source selector");
        }
        if object.pointer.len() > self.catalogue.limits.pointer_bytes {
            return Err("author selector exceeds configured pointer ceiling");
        }
        self.charge_metadata(object.name.len() + object.pointer.len() + object.prefix.len() + 16)?;
        self.objects.push(object);
        Ok(self)
    }
    pub fn checking(mut self, assessment: Assessment) -> Result<Self, &'static str> {
        if self.checking.is_some()
            || !matches!(assessment, Assessment::Initial | Assessment::FullBaseline)
        {
            return Err("one author check initial-v1 or full-baseline required");
        }
        self.checking = Some(assessment);
        Ok(self)
    }
    pub fn strict_warnings(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn thing_name(&self) -> &str {
        &self.thing
    }
    pub fn objects(&self) -> impl ExactSizeIterator<Item = &AuthorObject> {
        self.objects.iter()
    }
    pub fn plan_text(&self) -> Option<&str> {
        self.plan.as_ref().and_then(crate::Source::text)
    }

    /// Parse one bounded JSON author plan, retaining its exact accepted number tokens.
    pub fn from_plan(mut catalogue: Catalogue, bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() > (4 * 1024 * 1024).min(catalogue.limits.document_bytes) {
            return Err("author plan exceeds byte ceiling");
        }
        let parsed = catalogue.lend_decoding(|catalogue, budget| {
            crate::json::parse(bytes, true, &catalogue.limits, budget)
        });
        if parsed.rejected || parsed.root.is_none() || parsed.errors > 0 {
            return Err("author plan decoding rejected or incomplete");
        }
        let plan = crate::Source {
            role: SourceRole::Model,
            original: true,
            id: SourceId(0),
            key: String::new(),
            bytes: bytes.to_owned(),
            label: None,
            uri: None,
            version: None,
            digest: None,
            raw_path: None,
            acquisition: Acquisition::Supplied,
            label_omitted: false,
            complete: true,
            parsed,
        };
        let root = plan.json().require()?;
        if root.kind() != JsonKind::Map {
            return Err("author plan must be a map");
        }
        let mut request = Self::new(
            catalogue,
            text(root, "key")?,
            text(root, "generated_key")?,
            text(root, "thing_name")?,
        )?;
        if let Some(label) = root.get("generated_label").present() {
            request = request.generated_label(label.text().require()?)?;
        }
        let namespace = root.get("namespace").require()?;
        if namespace.kind() != JsonKind::Map {
            return Err("namespace map required");
        }
        for binding in namespace.children() {
            request =
                request.namespace(binding.member_name().require()?, binding.text().require()?)?;
        }
        if let Some(default) = root.get("default_namespace").present() {
            request = request.default_namespace(default.text().require()?)?;
        }
        if let Some(info) = root.get("info").present() {
            let info = request.catalogue.lend_decoding(|catalogue, budget| {
                JsonValue::copy_authored(info, &catalogue.limits, budget)
            })?;
            request = request.info(info)?;
        }
        if let Some(qualities) = root.get("thing_qualities").present() {
            let qualities = request.catalogue.lend_decoding(|catalogue, budget| {
                JsonValue::copy_authored(qualities, &catalogue.limits, budget)
            })?;
            request = request.thing_qualities(qualities)?;
        }
        let objects = root.get("objects").require()?;
        if objects.kind() != JsonKind::Array {
            return Err("objects array required");
        }
        for object in objects.children() {
            let source = request
                .catalogue
                .source_id(text(object, "source")?)
                .ok_or("plan source key is not supplied")?;
            let mut declaration = AuthorObject::new(
                text(object, "name")?,
                source,
                text(object, "pointer")?,
                text(object, "prefix")?,
                object.get("required").require()?.boolean().require()?,
            )?;
            if let Some(refinement) = object.get("refinement").present() {
                let refinement = request.catalogue.lend_decoding(|catalogue, budget| {
                    JsonValue::copy_authored(refinement, &catalogue.limits, budget)
                })?;
                declaration = declaration.refinement(refinement)?;
            }
            request = request.object(declaration)?;
        }
        request.plan = Some(plan);
        Ok(request)
    }

    pub fn author(mut self) -> Result<AuthorResult, &'static str> {
        if self.objects.is_empty() {
            return Err("at least one reusable object declaration required");
        }
        if self
            .default
            .as_ref()
            .is_some_and(|p| !self.namespaces.iter().any(|(prefix, _)| prefix == p))
        {
            return Err("default namespace must select a binding");
        }
        self.catalogue.reserve += self.metadata * 6 + 16384;
        if self.plan.is_none() {
            for value in self.info.iter().chain(self.qualities.iter()).chain(
                self.objects
                    .iter()
                    .filter_map(|object| object.refinement.as_ref()),
            ) {
                let (nodes, storage, depth) = value.author_storage();
                if depth > self.catalogue.limits.depth {
                    return Err("author patch depth exceeds configured ceiling");
                }
                self.catalogue.parsed_nodes_remaining = self
                    .catalogue
                    .parsed_nodes_remaining
                    .checked_sub(nodes)
                    .ok_or("author request parsed node ceiling")?;
                self.catalogue.parsed_allocation_remaining = self
                    .catalogue
                    .parsed_allocation_remaining
                    .checked_sub(storage)
                    .ok_or("author request parsed allocation ceiling")?;
            }
        }
        let generated = SourceId((self.catalogue.attempt_count() + 1) as u32);
        let mut budget = OperationBudget::new(&self.catalogue.limits);
        budget.work = budget.work.min(self.catalogue.decoding_work_remaining);
        let mut conditions = AuthorConditions::new(&self.catalogue.limits);
        let mut scope = AuthorScope::new(generated);
        let mut targets = Vec::new();
        let mut assembly = true;
        let contributions: Vec<_> = self
            .catalogue
            .sources()
            .filter(|source| source.role.is_model())
            .map(|source| {
                if source.json().present().is_some() {
                    scope.assessed[source.id.get() as usize] = true;
                }
                let contribution = source
                    .json()
                    .require()
                    .and_then(|root| contribution(root, &self.catalogue, &mut budget));
                (source.id, contribution)
            })
            .collect();
        for (id, contribution) in &contributions {
            if let Err(reason) = contribution {
                scope.unavailable[id.get() as usize] = true;
                conditions.push(AuthorCondition::new(
                    "author.fidelity_unknown",
                    false,
                    0,
                    reason,
                ));
            }
        }
        for (index, object) in self.objects.iter().enumerate() {
            let source = self
                .catalogue
                .source(object.source)
                .expect("validated source");
            let Some(root) = source.json().present() else {
                scope.unavailable[source.id.get() as usize] = true;
                conditions.push(AuthorCondition::new(
                    "reference.unavailable",
                    false,
                    index,
                    "Selected reusable source is unavailable",
                ));
                assembly = false;
                targets.push(None);
                continue;
            };
            let selected = match select(root, &object.pointer, &mut budget) {
                Ok(selected) => selected,
                Err(reason) => {
                    scope.unavailable[source.id.get() as usize] = true;
                    conditions.push(AuthorCondition::new(
                        "author.generation_limit",
                        false,
                        index,
                        reason,
                    ));
                    assembly = false;
                    targets.push(None);
                    continue;
                }
            };
            let Some((selected, position)) = selected else {
                return Err("reusable object selector does not exist");
            };
            if !matches!(
                position,
                Position::Vocabulary(Role::Definition(crate::Class::Object))
            ) || selected.kind() != JsonKind::Map
            {
                return Err("selector must select an sdfObject definition");
            }
            let namespace = match contributions
                .iter()
                .find(|(id, _)| *id == source.id)
                .expect("source contribution")
                .1
            {
                Ok(namespace) => namespace,
                Err(reason) => {
                    scope.unavailable[source.id.get() as usize] = true;
                    conditions.push(AuthorCondition::new(
                        "reference.unavailable",
                        false,
                        index,
                        reason,
                    ));
                    assembly = false;
                    targets.push(None);
                    continue;
                }
            };
            let Some(namespace) = namespace else {
                scope.unavailable[source.id.get() as usize] = true;
                conditions.push(AuthorCondition::new(
                    "author.local_only_object",
                    false,
                    index,
                    "Selected sdfObject has no established global namespace",
                ));
                assembly = false;
                targets.push(None);
                continue;
            };
            if !self
                .namespaces
                .iter()
                .any(|(prefix, uri)| prefix == &object.prefix && uri == namespace)
            {
                return Err("explicit output prefix does not bind selected object namespace");
            }
            let mut contributors = 0;
            let mut contributors_complete = true;
            for candidate in self.catalogue.sources().filter(|s| s.role.is_model()) {
                if let Some(root) = candidate.json().present()
                    && contributions
                        .iter()
                        .find(|(id, _)| *id == candidate.id)
                        .expect("candidate contribution")
                        .1
                        == Ok(Some(namespace))
                {
                    match select(root, &object.pointer, &mut budget) {
                        Ok(Some(_)) => contributors += 1,
                        Ok(None) => {}
                        Err(reason) => {
                            contributors_complete = false;
                            scope.unavailable[candidate.id.get() as usize] = true;
                            conditions.push(AuthorCondition::new(
                                "author.generation_limit",
                                false,
                                index,
                                reason,
                            ));
                            assembly = false;
                        }
                    }
                }
            }
            if contributors > 1 {
                conditions.push(AuthorCondition::new(
                    "author.identity_collision",
                    true,
                    index,
                    "Reusable global name has multiple supplied contributors",
                ));
                assembly = false;
            }
            if contributors == 1
                && contributors_complete
                && contributions.iter().all(|(_, value)| value.is_ok())
            {
                scope.dependencies[source.id.get() as usize] = true;
            }
            targets.push(Some(format!(
                "{}:#{}",
                object.prefix,
                fragment(&object.pointer)
            )));
        }
        if let Some(default) = &self.default {
            let namespace = &self
                .namespaces
                .iter()
                .find(|(prefix, _)| prefix == default)
                .expect("binding")
                .1;
            let pointer = format!("/sdfThing/{}", escape_pointer(&self.thing));
            for source in self.catalogue.sources().filter(|s| s.role.is_model()) {
                if let Some(root) = source.json().present()
                    && contributions
                        .iter()
                        .find(|(id, _)| *id == source.id)
                        .expect("source contribution")
                        .1
                        == Ok(Some(namespace))
                {
                    match select(root, &pointer, &mut budget) {
                        Ok(Some(_)) => {
                            conditions.push(AuthorCondition::new(
                                "author.identity_collision",
                                true,
                                0,
                                "Generated global name already has a supplied contributor",
                            ));
                            assembly = false;
                        }
                        Ok(None) => {}
                        Err(reason) => {
                            scope.unavailable[source.id.get() as usize] = true;
                            conditions.push(AuthorCondition::new(
                                "author.generation_limit",
                                false,
                                0,
                                reason,
                            ));
                            assembly = false;
                        }
                    }
                }
            }
        }
        let bytes = if assembly {
            emit_document(&self, &targets, &mut budget)
        } else {
            Err("requested assembly unavailable")
        };
        let (bytes, mut generation) = match bytes {
            Ok(bytes) => (bytes, Generation::Complete),
            Err(reason) => {
                if assembly {
                    conditions.push(AuthorCondition::new(
                        "author.generation_limit",
                        false,
                        0,
                        reason,
                    ));
                }
                (Vec::new(), Generation::Unavailable)
            }
        };
        let complete = generation == Generation::Complete;
        let parsed = self.catalogue.lend_decoding(|catalogue, decoding| {
            decoding.work = decoding.work.min(budget.work);
            let parsed = if complete {
                crate::json::parse(&bytes, true, &catalogue.limits, decoding)
            } else {
                crate::json::Parsed::unavailable()
            };
            if complete && (parsed.root.is_none() || parsed.rejected || parsed.errors > 0) {
                generation = Generation::Partial;
                conditions.push(AuthorCondition::new(
                    "author.generation_limit",
                    false,
                    0,
                    "Generated document decoding could not complete",
                ));
            }
            budget.work = budget.work.min(decoding.work);
            parsed
        });
        self.catalogue.sources.push(crate::Source {
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
            complete,
            parsed,
        });
        let mut fidelity = if generation == Generation::Complete && !conditions.unknown_fidelity {
            Fidelity::Established
        } else {
            Fidelity::Undetermined
        };
        let mut opaque = false;
        let mut known_fidelity = fidelity == Fidelity::Established;
        let mut known_reference_errors = 0;
        if generation == Generation::Complete {
            let root = self
                .catalogue
                .source(generated)
                .expect("generated source")
                .json()
                .require()?;
            let gate = crate::structure::Structure::preflight(&self.catalogue, root, &mut budget);
            let model = self.catalogue.compose(
                crate::EffectiveRequest::new(generated),
                &mut budget,
                gate,
                Vec::new(),
                false,
            )?;
            known_reference_errors = model.reference_errors;
            for dependency in &model.dependencies {
                scope.dependencies[dependency.get() as usize] = true;
                scope.assessed[dependency.get() as usize] = true;
            }
            for unavailable in &model.unavailable_dependencies {
                scope.unavailable[unavailable.get() as usize] = true;
            }
            if !model.complete() {
                fidelity = Fidelity::Undetermined;
                conditions.push(AuthorCondition::new(
                    "author.fidelity_unknown",
                    false,
                    0,
                    "Effective composition or reference meaning is unavailable",
                ));
            }
            for (index, node) in model.nodes.iter().enumerate() {
                if !crate::allowance::charge(&mut budget.work, 1) {
                    fidelity = Fidelity::Undetermined;
                    conditions.push(AuthorCondition::new(
                        "author.generation_limit",
                        false,
                        0,
                        "Fidelity traversal work limit",
                    ));
                    break;
                }
                if node.deleted {
                    continue;
                }
                if let Some(failure) = node
                    .unavailable
                    .or(node.meaning_unavailable)
                    .or(node.donor_unavailable)
                    .filter(|failure| failure.reported())
                {
                    let condition = AuthorCondition::new(
                        if failure.definite() {
                            failure.code()
                        } else {
                            "author.fidelity_unknown"
                        },
                        failure.definite(),
                        0,
                        failure.reason,
                    );
                    conditions.push(condition.affected(
                        crate::EffectiveJson {
                            model: &model,
                            node: index,
                        },
                        node.needed_ref.unwrap_or(node.source),
                        &self.catalogue.limits,
                        &mut budget,
                    ));
                }
                if let Some(key) = node.key
                    && let Some(parent) = node.parent
                    && let Some(role) = model.nodes[parent].role
                {
                    let object = model.nodes[parent]
                        .source
                        .get("type")
                        .present()
                        .and_then(|value| value.text().present())
                        == Some("object");
                    if !role.supports(key, object) || key == "sdfContext" {
                        opaque = true;
                        let condition = AuthorCondition::new(
                            "author.fidelity_unknown",
                            false,
                            0,
                            if key == "sdfContext" {
                                "Context content is retained; authoring fidelity is not yet established"
                            } else {
                                "Opaque content is retained; meaning after containment changes is unknown"
                            },
                        );
                        conditions.push(condition.affected(
                            crate::EffectiveJson {
                                model: &model,
                                node: index,
                            },
                            node.source,
                            &self.catalogue.limits,
                            &mut budget,
                        ));
                    }
                }
            }
            known_fidelity = fidelity == Fidelity::Established;
            if opaque {
                fidelity = Fidelity::Undetermined;
            }
        }
        let checking = if let Some(assessment) = self.checking {
            match self.catalogue.check_with_budget(
                CheckRequest::new()
                    .root(generated)
                    .assessment(assessment)
                    .strict_warnings(self.strict),
                &mut budget,
            ) {
                Ok(report) => Some(report::Checking::from_report(
                    report,
                    &mut conditions,
                    known_reference_errors,
                )),
                Err(reason) => {
                    conditions.push(AuthorCondition::new(
                        "author.generation_limit",
                        false,
                        0,
                        reason,
                    ));
                    Some(report::Checking::unavailable(
                        assessment,
                        generated,
                        self.strict,
                    ))
                }
            }
        } else {
            None
        };
        let finished = generation == Generation::Complete
            && fidelity == Fidelity::Established
            && checking.as_ref().is_none_or(|check| check.complete());
        let errors = conditions.errors - conditions.reference_errors
            + checking.as_ref().map_or(0, |check| check.errors())
            + known_reference_errors.saturating_sub(
                checking
                    .as_ref()
                    .map_or(0, |check| check.reference_errors()),
            );
        let warnings = checking.as_ref().map_or(0, |check| check.warnings());
        let notices = checking.as_ref().map_or(0, |check| check.notices());
        let outcome =
            crate::report::author_outcome(finished, errors, warnings, notices, self.strict);
        Ok(AuthorResult {
            request: self,
            generated,
            generation,
            fidelity,
            known_fidelity,
            opaque,
            conditions,
            checking,
            scope,
            outcome,
        })
    }
}

pub struct AuthorResult {
    request: AuthorRequest,
    generated: SourceId,
    generation: Generation,
    fidelity: Fidelity,
    known_fidelity: bool,
    opaque: bool,
    conditions: AuthorConditions,
    checking: Option<report::Checking>,
    scope: AuthorScope,
    outcome: Outcome,
}
// Source IDs have the shared finite ceiling, including the reserved generated root.
struct AuthorScope {
    dependencies: [bool; 257],
    assessed: [bool; 257],
    unavailable: [bool; 257],
}
impl AuthorScope {
    fn new(generated: SourceId) -> Self {
        let mut scope = Self {
            dependencies: [false; 257],
            assessed: [false; 257],
            unavailable: [false; 257],
        };
        scope.assessed[generated.get() as usize] = true;
        scope
    }
}
impl AuthorResult {
    pub fn catalogue(&self) -> &Catalogue {
        &self.request.catalogue
    }
    pub fn into_catalogue(self) -> Catalogue {
        self.request.catalogue
    }
    pub fn request(&self) -> &AuthorRequest {
        &self.request
    }
    pub fn generated_source(&self) -> SourceId {
        self.generated
    }
    pub fn generated_document(&self) -> Option<&[u8]> {
        let source = self.catalogue().source(self.generated)?;
        (!source.bytes.is_empty()).then_some(source.raw_bytes())
    }
    pub fn generation(&self) -> Generation {
        self.generation
    }
    pub fn fidelity(&self) -> Fidelity {
        self.fidelity
    }
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
    pub fn requested_coverage(&self) -> crate::RequestedCoverage {
        crate::report::author_coverage(self.outcome.complete())
    }
    pub fn checking(&self) -> AuthorChecking<'_> {
        AuthorChecking::new(self.checking.as_ref())
    }
    pub fn diagnostics(&self) -> crate::Diagnostics {
        report::diagnostics(self)
    }
    pub fn document_deliverable(&self) -> bool {
        self.generation == Generation::Complete
            && self.fidelity == Fidelity::Established
            && self.outcome.disposition() == crate::Disposition::Passed
    }
    pub fn to_json(&self) -> String {
        report::to_json(self)
    }
    pub fn to_human(&self) -> String {
        self.to_human_with_style(crate::HumanStyle::Plain)
    }
    /// Render with explicit presentation, without consulting the process environment.
    pub fn to_human_with_style(&self, style: crate::HumanStyle) -> String {
        report::to_human(self, style)
    }
}
struct AuthorCondition {
    code: &'static str,
    error: bool,
    object: usize,
    message: &'static str,
    affected: Option<AuthorAffectedLocation>,
}
struct AuthorAffectedLocation {
    root: SourceId,
    effective_pointer: Option<String>,
    source: SourceId,
    source_pointer: Option<String>,
    span: [u32; 2],
}
struct AuthorConditions {
    retained: Vec<AuthorCondition>,
    omitted: u32,
    count_omitted: u32,
    bytes_omitted: u32,
    reference_count_omitted: u32,
    reference_bytes_omitted: u32,
    mixed_reference_omitted: u32,
    presentation_omitted: u32,
    errors: u32,
    reference_errors: u32,
    limitations: u32,
    unknown_fidelity: bool,
    remaining: usize,
    ceiling: usize,
}
impl AuthorConditions {
    fn new(limits: &crate::Limits) -> Self {
        Self {
            retained: Vec::new(),
            omitted: 0,
            count_omitted: 0,
            bytes_omitted: 0,
            reference_count_omitted: 0,
            reference_bytes_omitted: 0,
            mixed_reference_omitted: 0,
            presentation_omitted: 0,
            errors: 0,
            reference_errors: 0,
            limitations: 0,
            unknown_fidelity: false,
            remaining: limits.detail_bytes,
            ceiling: limits.conditions,
        }
    }
    fn push(&mut self, condition: AuthorCondition) {
        let reference_error = condition.error && condition.code.starts_with("reference.");
        self.errors += u32::from(condition.error);
        self.reference_errors += u32::from(reference_error);
        self.limitations += u32::from(!condition.error);
        self.unknown_fidelity |= condition.code == "author.fidelity_unknown";
        let storage = std::mem::size_of::<AuthorCondition>()
            + condition.affected.as_ref().map_or(0, |location| {
                location.effective_pointer.as_ref().map_or(0, String::len)
                    + location.source_pointer.as_ref().map_or(0, String::len)
            });
        if self.retained.len() >= self.ceiling {
            self.count_omitted += 1;
            self.reference_count_omitted += u32::from(reference_error);
            self.omitted += 1;
        } else if self.remaining < storage {
            self.bytes_omitted += 1;
            self.reference_bytes_omitted += u32::from(reference_error);
            self.omitted += 1;
        } else {
            self.remaining -= storage;
            self.retained.reserve_exact(1);
            self.retained.push(condition);
        }
    }
    fn deduplicate_references(
        &mut self,
        mut checked: Option<&mut crate::report::AuthorReferences<'_>>,
        checked_errors: u32,
        known_errors: u32,
    ) {
        if checked.is_none() && checked_errors != known_errors {
            let mut references = 0;
            self.retained.retain(|condition| {
                let reference = condition.error && condition.code.starts_with("reference.");
                references += u32::from(reference);
                !reference
            });
            let hidden = self.reference_count_omitted + self.reference_bytes_omitted;
            let duplicates = checked_errors.min(hidden + references);
            self.omitted += references;
            self.omitted -= duplicates;
            self.count_omitted -= self.reference_count_omitted;
            self.bytes_omitted -= self.reference_bytes_omitted;
            // The remaining identities cannot be separated within the location
            // traversal allowance. Preserve their count without guessing detail.
            self.presentation_omitted = hidden + references - duplicates;
            return;
        }
        let mut retained_duplicates = 0;
        self.retained.retain(|condition| {
            let duplicate = condition.error
                && condition.code.starts_with("reference.")
                && (checked_errors == known_errors
                    || condition.affected.as_ref().is_some_and(|location| {
                        checked.as_deref_mut().is_some_and(|checked| {
                            checked.take(location.source, location.span, condition.code)
                        })
                    }));
            retained_duplicates += u32::from(duplicate);
            !duplicate
        });
        // Both passes examine the same immutable generated root. A later
        // bounded pass can establish a subset of its known reference errors.
        // The semantic counts still identify overlap when all detail is absent.
        let hidden = self.reference_count_omitted + self.reference_bytes_omitted;
        let duplicates = if checked_errors == known_errors {
            hidden
        } else {
            checked_errors
                .saturating_sub(retained_duplicates)
                .min(hidden)
        };
        self.omitted -= duplicates;
        if duplicates == 0 {
            return;
        }
        let count = self.reference_count_omitted;
        let bytes = self.reference_bytes_omitted;
        self.count_omitted -= count;
        self.bytes_omitted -= bytes;
        // Where omitted identities cannot distinguish two established causes,
        // retain only certain per-cause counts and disclose the combined cause.
        let count_certain = count.saturating_sub(duplicates);
        let bytes_certain = bytes.saturating_sub(duplicates);
        self.count_omitted += count_certain;
        self.bytes_omitted += bytes_certain;
        self.mixed_reference_omitted = hidden - duplicates - count_certain - bytes_certain;
    }
    fn iter(&self) -> std::slice::Iter<'_, AuthorCondition> {
        self.retained.iter()
    }
}
impl AuthorCondition {
    fn new(code: &'static str, error: bool, object: usize, message: &'static str) -> Self {
        Self {
            code,
            error,
            object,
            message,
            affected: None,
        }
    }
    fn affected(
        mut self,
        effective: crate::EffectiveJson<'_>,
        source: JsonView<'_>,
        limits: &crate::Limits,
        budget: &mut OperationBudget,
    ) -> Self {
        let cap = limits.pointer_bytes;
        let retained = crate::allowance::charge(&mut budget.allocation, cap * 2 + 256)
            && crate::allowance::charge(&mut budget.work, cap * 8 + 512);
        let source_span = source.span();
        self.affected = Some(AuthorAffectedLocation {
            root: effective.root_document(),
            effective_pointer: retained
                .then(|| effective.pointer(cap as u32).present())
                .flatten(),
            source: source.source_id(),
            source_pointer: retained
                .then(|| source.pointer(cap as u32).present())
                .flatten(),
            span: [source_span.start as u32, source_span.end as u32],
        });
        self
    }
}
fn name_field(name: &str, prefix: bool) -> Result<(), &'static str> {
    if name.len() > 1024 || name.contains(':') || name.is_empty() && prefix {
        Err("bounded exact name without colon required")
    } else {
        Ok(())
    }
}
fn text<'a>(map: JsonView<'a>, key: &str) -> Result<&'a str, &'static str> {
    map.get(key).require()?.text().require()
}
fn contribution<'a>(
    root: JsonView<'a>,
    catalogue: &'a Catalogue,
    budget: &mut OperationBudget,
) -> Result<Option<&'a str>, &'static str> {
    if !matches!(
        crate::structure::Structure::preflight(catalogue, root, budget),
        crate::structure::FeatureContext::Clear(_)
    ) {
        return Err("Namespace contribution context is unavailable");
    }
    let prefix = match field(root, "defaultNamespace", budget)? {
        Some(prefix) => prefix.text().require()?,
        None => return Ok(None),
    };
    let map =
        field(root, "namespace", budget)?.ok_or("Namespace contribution map is unavailable")?;
    let uri = field(map, prefix, budget)?
        .ok_or("Namespace contribution binding is unavailable")?
        .text()
        .require()?;
    if !crate::allowance::charge(&mut budget.work, uri.len() * 8 + 1) {
        return Err("Namespace contribution work limit");
    }
    if crate::uri::valid_uri(uri) {
        Ok(Some(uri))
    } else {
        Err("Namespace contribution URI is unavailable")
    }
}
fn field<'a>(
    map: JsonView<'a>,
    key: &str,
    budget: &mut OperationBudget,
) -> Result<Option<JsonView<'a>>, &'static str> {
    if map.kind() != JsonKind::Map {
        return Err("author source selector map is unavailable");
    }
    for child in map.children() {
        let name = child.member_name().require()?;
        if !crate::allowance::charge(&mut budget.work, name.len() + key.len() + 1) {
            return Err("author selector work limit");
        }
        if name == key {
            return Ok(Some(child));
        }
    }
    Ok(None)
}
fn select<'a>(
    mut current: JsonView<'a>,
    pointer: &str,
    budget: &mut OperationBudget,
) -> Result<Option<(JsonView<'a>, Position)>, &'static str> {
    let mut position = Position::Vocabulary(Role::Document);
    if pointer.is_empty() {
        return Ok(Some((current, position)));
    }
    for token in pointer[1..].split('/') {
        if !crate::allowance::charge(&mut budget.work, token.len() * 8 + 1) {
            return Err("author selector work limit");
        }
        let token = token.replace("~1", "/").replace("~0", "~");
        let Some(next) = field(current, &token, budget)? else {
            return Ok(None);
        };
        position = position.child(&token, false);
        current = next;
    }
    Ok(Some((current, position)))
}
fn escape_pointer(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}
fn fragment(pointer: &str) -> String {
    let mut result = String::new();
    for byte in pointer.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'/'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
                    | b'?'
            )
        {
            result.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(result, "%{byte:02X}").expect("String write");
        }
    }
    result
}
fn emit_document(
    request: &AuthorRequest,
    targets: &[Option<String>],
    budget: &mut OperationBudget,
) -> Result<Vec<u8>, &'static str> {
    if !crate::allowance::charge(
        &mut budget.nodes,
        8 + request.namespaces.len() + request.objects.len() * 2,
    ) {
        return Err("generated structural node ceiling reached");
    }
    let retained: usize = request
        .catalogue
        .retained_sources()
        .map(|s| s.bytes.len())
        .sum();
    let cap = request.catalogue.limits.document_bytes.min(
        request
            .catalogue
            .limits
            .catalogue_bytes
            .saturating_sub(retained),
    );
    let mut out = JsonWriter::new(cap, budget, NodePolicy::Author);
    out.emit("{\n  \"namespace\": {")?;
    for (index, (prefix, uri)) in request.namespaces.iter().enumerate() {
        if index > 0 {
            out.emit(",")?;
        }
        out.emit("\n    ")?;
        out.quoted(prefix)?;
        out.emit(": ")?;
        out.quoted(uri)?;
    }
    if !request.namespaces.is_empty() {
        out.emit("\n  ")?;
    }
    out.emit("}")?;
    if let Some(default) = &request.default {
        out.emit(",\n  \"defaultNamespace\": ")?;
        out.quoted(default)?;
    }
    if let Some(info) = &request.info {
        out.emit(",\n  \"info\": ")?;
        out.value(info.view(), 1)?;
    }
    out.emit(",\n  \"sdfThing\": {\n    ")?;
    out.quoted(&request.thing)?;
    out.emit(": {\n")?;
    if let Some(qualities) = &request.qualities {
        for quality in qualities.view().children() {
            out.emit("      ")?;
            out.quoted(quality.member_name().require()?)?;
            out.emit(": ")?;
            out.value(quality, 3)?;
            out.emit(",\n")?;
        }
    }
    out.emit("      \"sdfObject\": {")?;
    for (index, object) in request.objects.iter().enumerate() {
        if index > 0 {
            out.emit(",")?;
        }
        out.emit("\n        ")?;
        out.quoted(&object.name)?;
        out.emit(": {\n          \"sdfRef\": ")?;
        out.quoted(
            targets[index]
                .as_deref()
                .ok_or("object binding unavailable")?,
        )?;
        if let Some(patch) = &object.refinement {
            for quality in patch.view().children() {
                out.emit(",\n          ")?;
                out.quoted(quality.member_name().require()?)?;
                out.emit(": ")?;
                out.value(quality, 5)?;
            }
        }
        out.emit("\n        }")?;
    }
    out.emit("\n      }")?;
    if request.objects.iter().any(|o| o.required) {
        out.emit(",\n      \"sdfRequired\": [")?;
        for (index, object) in request.objects.iter().filter(|o| o.required).enumerate() {
            if index > 0 {
                out.emit(", ")?;
            }
            if object.name.contains('#') {
                out.quoted(&format!(
                    "#{}",
                    fragment(&format!(
                        "/sdfThing/{}/sdfObject/{}",
                        escape_pointer(&request.thing),
                        escape_pointer(&object.name)
                    ))
                ))?;
            } else {
                out.quoted(&object.name)?;
            }
        }
        out.emit("]")?;
    }
    out.emit("\n    }\n  }\n}\n")?;
    Ok(out.into_bytes())
}

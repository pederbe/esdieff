//! Coverage observations made during the shared, budgeted structure traversal.
//! Follows ADR 0003: Check findings and completeness.
use super::*;
mod formal;
mod meaning;

pub(super) fn meaning_fixed_headers() -> usize {
    meaning::fixed_headers()
}

impl<'a, 'view> Checker<'a, 'view, '_> {
    pub(super) fn coverage_meaning(
        &mut self,
        json: CheckView<'a, 'view>,
        role: Role,
        object: bool,
    ) -> bool {
        if !self.result.coverage.full_requested
            || !role.data_description_role()
            || json.effective().is_none()
        {
            return true;
        }
        let Some(observations) = meaning::scalar(json, role, object, self.budget) else {
            self.record(json, Rule::WorkLimit);
            return false;
        };
        for (member, status) in observations.scalar {
            self.result.coverage.observe(member, status);
        }
        let CheckView::Effective { model, node } = json else {
            unreachable!()
        };
        let compound = meaning::compound(
            crate::EffectiveJson { model, node },
            &observations,
            self.budget,
            self.number_work
                .as_mut()
                .expect("Full model numeric counter"),
            self.equality.as_mut().expect("Full model equality counter"),
            &self.catalogue.limits,
        );
        let special = meaning::special(
            crate::EffectiveJson { model, node },
            &observations,
            &self.catalogue.limits,
            self.budget,
        );
        let equality = self.equality.as_ref().expect("Full model equality counter");
        self.result.temporal_use |= equality.temporal_use;
        self.result.format_use |= equality.format_use;
        self.result.abstract_use |= equality.abstract_use;
        let Some(compound) = compound else {
            self.record(json, Rule::WorkLimit);
            return false;
        };
        for (member, status) in compound {
            self.result.coverage.observe(member, status);
        }
        let Some(special) = special else {
            self.record(json, Rule::WorkLimit);
            return false;
        };
        for (member, status) in special.rows {
            self.result.coverage.observe(member, status);
        }
        if let Some((node, condition)) = special.pattern_condition {
            self.record(
                CheckView::Effective { model, node },
                Rule::PatternMeaning(condition),
            );
        }
        true
    }
    pub(super) fn coverage_record(
        &mut self,
        json: CheckView<'a, 'view>,
        rule: Rule,
    ) -> Option<Requirement> {
        let key = json.member_name().present().unwrap_or("");
        let parent = match json {
            CheckView::Source(source) => source
                .n()
                .parent
                .and_then(|parent| source.source.parsed.nodes[parent].key.as_deref()),
            CheckView::Effective { model, node } => model.nodes[node]
                .parent
                .and_then(|parent| model.nodes[parent].key),
        }
        .unwrap_or("");
        let owner = match rule {
            Rule::Required(_) => Some(Requirement::M020),
            Rule::NameColon => Some(Requirement::M004),
            Rule::QualityNameSyntax => Some(Requirement::M005),
            Rule::BlockShape => match key {
                "info" => Some(Requirement::M009),
                "namespace" => Some(Requirement::M014),
                _ => None,
            },
            Rule::NamespaceUri => Some(Requirement::M014),
            Rule::NamespaceDefault => Some(Requirement::M015),
            Rule::GroupShape | Rule::DefinitionShape => Some(
                match if matches!(key, "properties" | "sdfChoice") {
                    key
                } else {
                    parent
                } {
                    "properties" => Requirement::M038,
                    "sdfChoice" => Requirement::M030,
                    _ => Requirement::M002,
                },
            ),
            Rule::RoleUnavailable => Some(Requirement::M003),
            Rule::GroupingCount | Rule::NumberUnavailable | Rule::DataCount => {
                Some(Requirement::M037)
            }
            Rule::PositiveStep => Some(Requirement::M045),
            Rule::DataNumber | Rule::DataNumberUnavailable => Some(
                if matches!(key, "minLength" | "maxLength" | "minItems" | "maxItems") {
                    Requirement::M037
                } else if matches!(key, "const" | "default") {
                    Requirement::M035
                } else {
                    Requirement::M036
                },
            ),
            Rule::DataString => Some(match key {
                "type" => Requirement::M034,
                "format" | "pattern" => Requirement::M039,
                "sdfType" => Requirement::M025,
                "unit" | "contentFormat" => Requirement::M022,
                _ => Requirement::M021,
            }),
            Rule::DataBoolean => Some(match key {
                "nullable" => Requirement::M029,
                "readable" | "writable" | "observable" => Requirement::M031,
                _ => Requirement::M039,
            }),
            Rule::DataEnum | Rule::DataEnumChoice => Some(Requirement::M030),
            Rule::DataRequired => Some(Requirement::M038),
            Rule::DataMap => Some(match key {
                "items" => Requirement::M040,
                "properties" => Requirement::M038,
                "sdfChoice" => Requirement::M030,
                _ => Requirement::M022,
            }),
            Rule::DataShape => Some(Requirement::M002),
            Rule::MetadataString => Some(match key {
                "description" | "label" | "$comment" => Requirement::M021,
                "modified" => Requirement::M012,
                _ => Requirement::M009,
            }),
            Rule::FeatureContextUnavailable
            | Rule::CriticalUnsupported
            | Rule::FeatureDeclarationUnavailable => Some(Requirement::M013),
            Rule::ModifiedSyntax
            | Rule::ModifiedCalendar
            | Rule::ModifiedClock
            | Rule::ModifiedExceptionalSlot
            | Rule::ModifiedLeapSecond
            | Rule::ModifiedKnowledgeUnavailable
            | Rule::ModifiedUnavailable
            | Rule::ModifiedWorkLimit => Some(Requirement::M012),
            Rule::Pattern(_) => Some(Requirement::M047),
            Rule::PatternMeaning(_) => Some(Requirement::M048),
            Rule::FormalUint => Some(Requirement::M037),
            Rule::FormalUnavailable(owner) => Some(owner),
            Rule::Unit(_) => Some(Requirement::M023),
            Rule::ContentFormat(_) => Some(Requirement::M024),
            _ => None,
        };
        match rule {
            Rule::DataNumberUnavailable | Rule::NumberUnavailable => {
                self.result.coverage.initial(4, Undetermined)
            }
            Rule::RoleUnavailable => self.result.coverage.initial(1, Undetermined),
            Rule::WorkLimit => self.coverage_unknown(if json.effective().is_some() {
                Role::Data
            } else {
                Role::Document
            }),
            Rule::FeatureContextUnavailable
            | Rule::CriticalUnsupported
            | Rule::FeatureDeclarationUnavailable => self.result.coverage.initial(6, Undetermined),
            _ => {}
        }
        if let Some(owner) = owner {
            self.result.coverage.observe(
                owner,
                if rule.severity() == "incomplete" {
                    Undetermined
                } else {
                    Assessed
                },
            );
        }
        owner
    }
    pub(super) fn coverage_role(&mut self, role: Role) {
        let coverage = &mut self.result.coverage;
        coverage.initial(1, Assessed);
        coverage.initial(6, Assessed);
        coverage.observe(Requirement::M005, Assessed);
        coverage.observe(Requirement::M033, Assessed);
        match role {
            Role::Document => {
                coverage.initial(2, Assessed);
                coverage.observe(Requirement::M007, Assessed);
                coverage.observe(Requirement::M010, Assessed);
            }
            Role::Information => coverage.observe(Requirement::M009, Assessed),
            Role::Definition(_) => {
                coverage.observe(Requirement::M002, Assessed);
                coverage.observe(Requirement::M003, Assessed);
                coverage.observe(Requirement::M004, Assessed);
                if role == Role::Definition(Class::Property) {
                    coverage.observe(Requirement::M031, Assessed);
                }
            }
            _ => {}
        }
        if role.data_description_role() {
            coverage.initial(4, Assessed);
            if role.full_data() {
                coverage.observe(Requirement::M029, Assessed);
            }
        }
    }
    pub(super) fn coverage_unknown(&mut self, role: Role) {
        let coverage = &mut self.result.coverage;
        coverage.initial(1, Undetermined);
        coverage.initial(6, Undetermined);
        if role == Role::Document {
            for area in [2, 3, 4, 5] {
                coverage.initial(area, Undetermined);
            }
            if coverage.full_requested {
                for state in &mut coverage.requirements[1..] {
                    *state = Undetermined;
                }
            }
        } else if role == Role::Information {
            coverage.initial(2, Undetermined);
            for member in [
                Requirement::M009,
                Requirement::M010,
                Requirement::M011,
                Requirement::M012,
                Requirement::M013,
                Requirement::M021,
            ] {
                coverage.observe(member, Undetermined);
            }
        } else {
            coverage.initial(3, Undetermined);
            coverage.initial(4, Undetermined);
            coverage.initial(5, Undetermined);
            for member in [
                Requirement::M002,
                Requirement::M003,
                Requirement::M004,
                Requirement::M005,
                Requirement::M006,
                Requirement::M011,
                Requirement::M016,
                Requirement::M017,
            ] {
                coverage.observe(member, Undetermined);
            }
            if coverage.full_requested {
                for state in &mut coverage.requirements[17..] {
                    *state = Undetermined;
                }
            }
        }
    }
    pub(super) fn coverage_field(
        &mut self,
        json: CheckView<'a, 'view>,
        key: &str,
        role: Role,
        object: bool,
    ) {
        let coverage = &mut self.result.coverage;
        let vocabulary = role.vocabulary(key, object);
        if vocabulary != Vocabulary::Supported && key != "sdfRequiredInputData" {
            coverage.observe(Requirement::M033, Assessed);
            if vocabulary.undeclared() {
                coverage.observe(Requirement::M028, Undetermined);
            }
        }
        if role == Role::Document {
            match key {
                "info" => coverage.observe(Requirement::M009, Assessed),
                "namespace" => coverage.observe(Requirement::M014, Assessed),
                "defaultNamespace" => coverage.observe(Requirement::M015, Assessed),
                _ => {}
            }
        }
        if role.metadata(key) {
            let member = match key {
                "$comment" => {
                    coverage.observe(Requirement::M011, Assessed);
                    Requirement::M021
                }
                "description" | "label" => Requirement::M021,
                "modified" => Requirement::M012,
                _ => Requirement::M009,
            };
            coverage.observe(member, Assessed);
        }
        if role == Role::Information && key == "features" {
            coverage.observe(Requirement::M013, Assessed);
        }
        if role.reference_context() && key == "sdfRef" {
            for member in [
                Requirement::M006,
                Requirement::M016,
                Requirement::M017,
                Requirement::M018,
                Requirement::M019,
            ] {
                coverage.observe(member, Assessed);
            }
        }
        if !role.data_description_role() {
            return;
        }
        if vocabulary != Vocabulary::Supported {
            if role == Role::DirectItem && vocabulary == Vocabulary::OutsideRole {
                coverage.observe(Requirement::M041, Undetermined);
            }
            return;
        }
        match key {
            "unit" | "contentFormat" => {
                coverage.observe(Requirement::M022, Assessed);
            }
            "type" => {
                coverage.observe(Requirement::M034, Assessed);
                if json.effective().is_some() && json.kind() == JsonKind::String {
                    if !matches!(
                        json.text(),
                        Field::Present(
                            "number" | "integer" | "string" | "boolean" | "array" | "object"
                        )
                    ) {
                        coverage.observe(Requirement::M028, Undetermined);
                    }
                    if role == Role::DirectItem && matches!(json.text(), Field::Present("array")) {
                        coverage.observe(Requirement::M041, Undetermined);
                    }
                }
            }
            "sdfType" => {
                coverage.observe(
                    Requirement::M025,
                    if json.kind() == JsonKind::String
                        && !matches!(json.text(), Field::Present("byte-string" | "unix-time"))
                    {
                        Unassessed
                    } else {
                        Assessed
                    },
                );
                if json.effective().is_some() {
                    match json.text() {
                        Field::Present("byte-string" | "unix-time") => {}
                        Field::Present(_) => coverage.observe(Requirement::M028, Undetermined),
                        _ => {}
                    }
                }
            }
            "const" | "default" if json.effective().is_some() => {
                if key == "default" {
                    coverage.observe(Requirement::M044, Assessed);
                }
            }
            "pattern" => {
                coverage.observe(Requirement::M039, Assessed);
            }
            "format" => {
                coverage.observe(Requirement::M039, Assessed);
                if json.effective().is_some()
                    && json.kind() == JsonKind::String
                    && !matches!(
                        json.text(),
                        Field::Present(
                            "date-time" | "date" | "time" | "uri" | "uri-reference" | "uuid"
                        )
                    )
                {
                    coverage.observe(Requirement::M028, Undetermined);
                }
            }
            "uniqueItems" => {
                coverage.observe(Requirement::M039, Assessed);
            }
            "properties" | "required" => {
                coverage.observe(Requirement::M038, Assessed);
            }
            "items" => {
                coverage.observe(Requirement::M040, Assessed);
            }
            "sdfChoice" | "enum" => {
                coverage.initial(5, Assessed);
                coverage.observe(Requirement::M030, Assessed);
            }
            _ => {}
        }
    }
}

//! Final effective coordinates use the shared original operation allowance.
//! Follows ADR 0029: Content-format declarations and recorded registry guidance.
use super::lookup::{self, CODINGS, MEDIA, NUMERIC, Names};
use super::{AREAS, State, grammar, table};
use crate::registry::{CONTENT_FORMAT_MEANING, CONTENT_FORMAT_RANGE};

#[derive(Clone, Copy)]
pub(crate) enum Rule {
    Syntax,
    Range,
    Work,
    RegistryWork,
    Known(u8),
    Unknown,
    Withheld,
    Identity,
    Parameters,
    MeaningUnsupported,
}
impl Rule {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Syntax => "content_format.syntax",
            Self::Range => "content_format.number_range",
            Self::Work | Self::RegistryWork => "operation.limit",
            Self::Known(_) => "content_format.registry_known",
            Self::Unknown => "content_format.registration_unknown",
            Self::Withheld => "content_format.registry_knowledge_withheld",
            Self::Identity => "content_format.interpretation_unavailable",
            Self::Parameters => "content_format.parameter_semantics_unassessed",
            Self::MeaningUnsupported => "content_format.model_meaning_unassessed",
        }
    }
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Syntax => "The complete contentFormat text does not follow the RFC 9193 grammar",
            Self::Range => {
                "The decimal contentFormat has valid Section 6 spelling but lies outside the Section 3 identifier range 0 through 65535"
            }
            Self::Work | Self::RegistryWork => {
                "Content-format assessment reached its shared finite work limit; established earlier findings remain available"
            }
            Self::Known(4) => {
                "The recorded dated edition contains this positive temporary registration; this does not establish its current status"
            }
            Self::Known(3) => {
                "The recorded dated edition contains this positive registration marked obsolete and deprecated; this does not establish its current status"
            }
            Self::Known(1) => {
                "The recorded dated edition contains this positive registration marked obsolete; this does not establish its current status"
            }
            Self::Known(2) => {
                "The recorded dated edition contains this positive registration marked deprecated; this does not establish its current status"
            }
            Self::Known(_) => {
                "The recorded dated edition contains this positive registration; this does not establish its current status"
            }
            Self::Unknown => {
                "The spelling is absent from the relevant dated positive table; current registration is unknown"
            }
            Self::Withheld => {
                "The caller withheld the recorded evidence needed for this registration query"
            }
            Self::Identity => {
                "The explicit identity content-coding token is reserved; its declaration meaning is outside this inventory"
            }
            Self::Parameters => {
                "Parameter spelling was checked; parameter meaning and duplicate-parameter meaning are outside this inventory"
            }
            Self::MeaningUnsupported => {
                "The declaration needs media or parameter meaning outside the supported model capability"
            }
        }
    }
    pub(crate) fn citation(self) -> Option<&'static str> {
        match self {
            Self::Syntax => Some("RFC 9193 Section 6"),
            Self::Range => Some("RFC 9193 Section 3"),
            _ => None,
        }
    }
    pub(crate) fn severity(self) -> &'static str {
        match self {
            Self::Syntax | Self::Range => "error",
            Self::Known(_) | Self::Parameters => "notice",
            _ => "incomplete",
        }
    }
    pub(crate) fn basis_kind(self) -> &'static str {
        match self {
            Self::Syntax => "rfc",
            Self::Range | Self::Identity => "interpretation",
            Self::MeaningUnsupported => "interpretation",
            Self::Work | Self::RegistryWork | Self::Withheld => "operational",
            _ => "advice",
        }
    }
    pub(crate) fn basis_id(self) -> Option<&'static str> {
        match self {
            Self::Syntax => None,
            Self::Range => Some("I-CF-RANGE-1"),
            Self::Work => Some("content-format-establishment"),
            Self::RegistryWork => Some("content-format-registry-establishment"),
            Self::Known(_) | Self::Unknown => Some(table::EXTRACTION_POLICY),
            Self::Withheld => Some("recorded-registry-policy"),
            Self::Identity => Some("content-format-identity-meaning"),
            Self::Parameters => Some("content-format-inventory-exclusions"),
            Self::MeaningUnsupported => Some("content-format-model-meaning/1"),
        }
    }
    pub(crate) fn area(self) -> &'static str {
        match self {
            Self::Syntax | Self::Range | Self::Work => AREAS[0],
            _ => AREAS[1],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Grammar,
    Registry,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stop {
    Model,
    Node,
    Member,
    Classification,
    Lookup,
    Identity,
    Ancestry,
    Alternative,
}
#[cfg(test)]
std::thread_local! {
    // Test-only coverage of real public-operation stop paths, with no new
    // production state, retained population, allocator or scheduling behavior.
    static LAST_STOP: std::cell::Cell<Option<(Phase, Stop)>> = const { std::cell::Cell::new(None) };
}

struct Scanner<'a, 'b> {
    state: &'b mut State,
    registry: &'b mut crate::registry::State,
    budget: &'b mut crate::allowance::OperationBudget,
    structure: &'b mut crate::structure::Structure<'a>,
    catalogue: &'a crate::Catalogue,
    detail: usize,
    phase: Phase,
}

// Eight existing fixed common descriptor slots cover these simultaneous groups.
// Tuples include the compiled field padding; borrowed data has no new heap owner.
fn temporary_inventory() -> [usize; 8] {
    use std::mem::size_of;
    [
        size_of::<(Scanner<'_, '_>, [&usize; 3])>(),
        size_of::<(
            std::slice::Iter<'_, crate::EffectiveModel<'_>>,
            (usize, usize),
            Option<usize>,
            &crate::EffectiveModel<'_>,
            &[crate::EffectiveModel<'_>],
            usize,
        )>(),
        size_of::<(
            [Option<usize>; 3],
            [usize; 3],
            [&crate::effective::Node<'_>; 3],
            [bool; 3],
            [&usize; 3],
        )>(),
        size_of::<(
            Option<grammar::Declaration<'_>>,
            crate::JsonView<'_>,
            &str,
            bool,
            Result<Option<u8>, ()>,
            &crate::EffectiveModel<'_>,
            usize,
        )>(),
        size_of::<([usize; 8], &[u8], &str, table::Numeric, [u8; 2], [bool; 2])>(),
        size_of::<(
            std::iter::Skip<std::str::Split<'_, char>>,
            &str,
            bool,
            Result<Option<u8>, ()>,
        )>()
        .max(super::meaning::fixed_headers()),
        size_of::<(
            [usize; 3],
            &[(&str, u8)],
            &str,
            u8,
            [std::str::Bytes<'_>; 2],
            std::cmp::Ordering,
            [&usize; 3],
        )>(),
        size_of::<(crate::structure::Finding<'_>, [usize; 2], Rule)>(),
    ]
}
pub(crate) fn temporary_storage_supported() -> bool {
    let slot = 128
        + std::mem::size_of::<usize>()
        + std::mem::size_of::<String>()
        + std::mem::size_of::<Vec<serde_json::Value>>();
    std::mem::align_of::<Scanner<'_, '_>>() <= 8
        && std::mem::align_of::<grammar::Declaration<'_>>() <= 8
        && temporary_inventory().iter().all(|&bytes| bytes <= slot)
}
impl<'a> Scanner<'a, '_> {
    fn charge(&mut self, amount: usize) -> bool {
        crate::allowance::charge(&mut self.budget.work, amount)
    }
    fn defer(&mut self) {
        if self.phase == Phase::Grammar {
            self.state.defer();
        } else {
            self.state.statuses[1] = 2;
        }
    }
    fn stop(&mut self, model: &crate::EffectiveModel<'a>, node: Option<usize>, site: Stop) -> bool {
        self.defer();
        self.state.reasons |= 2;
        #[cfg(test)]
        LAST_STOP.set(Some((self.phase, site)));
        let _ = site;
        if let Some(node) = node {
            self.record(
                model,
                node,
                if self.phase == Phase::Grammar {
                    Rule::Work
                } else {
                    Rule::RegistryWork
                },
            );
        }
        false
    }
    fn query(&mut self, registry: u8) -> bool {
        self.state.assessed(1);
        self.state.needed |= registry;
        self.state.increment(2);
        if self.registry.policy == crate::RecordedRegistryPolicy::WithholdRecorded {
            self.registry.withheld |= registry;
            self.state.withheld |= registry;
            self.state.reasons |= 1;
            false
        } else {
            true
        }
    }
    fn numeric(&mut self, number: u16, bytes: usize, meaning: bool) -> Result<Option<u8>, ()> {
        let found = lookup::numeric(number, bytes, self.registry, &mut self.budget.work);
        self.state.used |= self.registry.consulted & NUMERIC;
        let Some(row) = found? else {
            return Ok(None);
        };
        if meaning {
            // The canonical tuple's complete static fields fit128
            // bytes, independently checked against all captured rows.
            if !self.charge(129) {
                return Err(());
            }
            self.registry.interpretations |= CONTENT_FORMAT_MEANING;
            self.structure.coverage.observe(
                crate::inventory::Requirement::M024,
                if super::meaning::numeric(row) {
                    crate::AssessmentStatus::Assessed
                } else {
                    crate::AssessmentStatus::Undetermined
                },
            );
        }
        Ok(Some(u8::from(row.temporary) * 4))
    }
    fn named(&mut self, text: &str, names: Names) -> Result<Option<u8>, ()> {
        let found = lookup::named(text, names, self.registry, &mut self.budget.work);
        self.state.used |= self.registry.consulted & names.bit();
        found
    }
    fn result(
        &mut self,
        model: &crate::EffectiveModel<'a>,
        node: usize,
        result: Result<Option<u8>, ()>,
        identity: bool,
    ) -> bool {
        let rule = match result {
            Ok(Some(flags)) => Rule::Known(flags),
            Ok(None) => {
                self.state.statuses[1] = 2;
                self.state.increment(3);
                if identity {
                    self.state.reasons |= 4;
                    Rule::Identity
                } else {
                    Rule::Unknown
                }
            }
            Err(()) => {
                return self.stop(model, Some(node), Stop::Lookup);
            }
        };
        self.record(model, node, rule);
        true
    }
    fn withheld(&mut self, model: &crate::EffectiveModel<'a>, node: usize) {
        self.state.statuses[1] = 2;
        self.state.increment(3);
        self.record(model, node, Rule::Withheld);
    }
    fn record(&mut self, model: &crate::EffectiveModel<'a>, node: usize, rule: Rule) {
        if matches!(rule, Rule::Parameters | Rule::MeaningUnsupported) {
            self.structure.coverage.observe(
                crate::inventory::Requirement::M024,
                crate::AssessmentStatus::Unassessed,
            );
        }
        self.structure.total += 1;
        if rule.severity() == "error" {
            self.structure.errors += 1;
        }
        if rule.severity() == "notice" {
            self.structure.notices += 1;
        }
        if self.structure.findings.len() == self.catalogue.limits.conditions {
            self.structure.count_omitted = true;
            return;
        }
        if self.structure.findings.len() == self.structure.findings.capacity() {
            let old = self.structure.findings.capacity();
            let new = (old.max(4) * 2).min(self.catalogue.limits.conditions);
            if !crate::allowance::charge(
                &mut self.detail,
                (new - old) * std::mem::size_of::<crate::structure::Finding<'_>>(),
            ) {
                self.structure.bytes_omitted = true;
                return;
            }
            self.structure.findings.reserve_exact(new - old);
        }
        self.structure.findings.push(crate::structure::Finding {
            json: model.nodes[node].source,
            effective: Some((model.source, node)),
            rule: crate::structure::Rule::ContentFormat(rule),
            proof: None,
            replacement: None,
            coverage_requirement: Some(crate::inventory::Requirement::M024),
        });
    }
    fn declaration(&mut self, model: &crate::EffectiveModel<'a>, node: usize) -> bool {
        if self.phase == Phase::Grammar {
            self.state.assessed(0);
            self.state.increment(0);
        }
        let json = model.nodes[node].source;
        if json.kind() != crate::JsonKind::String {
            return true;
        }
        let Some(text) = json.text().present() else {
            self.defer();
            return true;
        };
        let admitted = text
            .len()
            .checked_mul(32)
            .and_then(|n| n.checked_add(1))
            .is_some_and(|n| self.charge(n));
        if !admitted {
            return self.stop(model, Some(node), Stop::Classification);
        }
        let declaration = grammar::classify(text);
        if self.phase == Phase::Grammar {
            self.state.increment(1);
            match declaration {
                None => self.record(model, node, Rule::Syntax),
                Some(grammar::Declaration::Number(None)) => {
                    self.registry.interpretations |= CONTENT_FORMAT_RANGE;
                    self.record(model, node, Rule::Range);
                }
                Some(grammar::Declaration::Number(Some(_))) => {
                    self.state.needed |= NUMERIC;
                    self.state.assessed(1);
                }
                Some(grammar::Declaration::Media {
                    codings,
                    parameters,
                    ..
                }) => {
                    self.state.needed |= MEDIA | (u8::from(!codings.is_empty()) * CODINGS);
                    self.state.assessed(1);
                    if !parameters.is_empty() && !self.structure.coverage.full_requested {
                        self.record(model, node, Rule::Parameters);
                    }
                }
            }
            return true;
        }
        let meaning = if self.structure.coverage.full_requested {
            let Some(needed) =
                (crate::EffectiveJson { model, node }).model_meaning_needed(&mut self.budget.work)
            else {
                return self.stop(model, Some(node), Stop::Ancestry);
            };
            needed
        } else {
            false
        };
        match declaration {
            None | Some(grammar::Declaration::Number(None)) => {}
            Some(grammar::Declaration::Number(Some(number))) => {
                if self.query(NUMERIC) {
                    let result = self.numeric(number, text.len(), meaning);
                    if !self.result(model, node, result, false) {
                        return false;
                    }
                } else {
                    self.withheld(model, node);
                }
            }
            Some(grammar::Declaration::Media {
                name,
                codings,
                parameters,
            }) => {
                if meaning {
                    // Charge every compared media/parameter byte before
                    // consulting the bounded normative rule set.
                    if !name
                        .len()
                        .checked_add(parameters.len())
                        .and_then(|n| n.checked_mul(8))
                        .and_then(|n| n.checked_add(64))
                        .is_some_and(|n| self.charge(n))
                    {
                        return self.stop(model, Some(node), Stop::Classification);
                    }
                    self.registry.interpretations |= CONTENT_FORMAT_MEANING;
                    if super::meaning::media(name, parameters) {
                        self.structure.coverage.observe(
                            crate::inventory::Requirement::M024,
                            crate::AssessmentStatus::Assessed,
                        );
                    } else {
                        self.record(model, node, Rule::MeaningUnsupported);
                    }
                }
                if self.query(MEDIA) {
                    let result = self.named(name, Names::Media);
                    if !self.result(model, node, result, false) {
                        return false;
                    }
                } else {
                    self.withheld(model, node);
                }
                for coding in codings.split('@').skip(1) {
                    if self.query(CODINGS) {
                        let result = self.named(coding, Names::Codings);
                        // Reserved identity is separate from positive membership.
                        if !self.charge(1 + coding.len() + 8) {
                            return self.stop(model, Some(node), Stop::Identity);
                        }
                        let identity = coding.eq_ignore_ascii_case(table::RESERVED_CODINGS[0]);
                        if !self.result(model, node, result, identity) {
                            return false;
                        }
                    } else {
                        self.withheld(model, node);
                    }
                }
            }
        }
        true
    }
    fn tree(&mut self, model: &crate::EffectiveModel<'a>, root: usize) -> bool {
        let mut current = Some(root);
        while let Some(id) = current {
            if !self.charge(1 + model.nodes[id].key.map_or(0, str::len) + 9) {
                return self.stop(model, Some(id), Stop::Node);
            }
            let node = &model.nodes[id];
            let required = model.required_quality(id);
            let skip = node.deleted
                || required
                || node.sample_skipped
                || node.donor_unavailable.is_some()
                || node.unavailable.is_some();
            if !node.deleted
                && !required
                && (node.unavailable.is_some()
                    || node.meaning_unavailable.is_some()
                    || node.donor_unavailable.is_some())
            {
                self.defer();
            }
            let choice = node.key == Some("sdfChoice");
            if !skip
                && node.role.is_some_and(|role| role.full_data())
                && node.source.kind() == crate::JsonKind::Map
            {
                let mut next = node.first;
                let mut found = None;
                let mut reliable = true;
                while let Some(child) = next {
                    let quality = &model.nodes[child];
                    next = quality.next;
                    if !self.charge(1 + quality.key.map_or(0, str::len) + 13) {
                        return self.stop(model, Some(id), Stop::Member);
                    }
                    if quality.deleted {
                        continue;
                    }
                    if quality.key.is_none() {
                        reliable = false;
                    }
                    if quality.key == Some("contentFormat") {
                        if quality.unavailable.is_some()
                            || quality.meaning_unavailable.is_some()
                            || quality.donor_unavailable.is_some()
                        {
                            reliable = false;
                        } else {
                            found = Some(child);
                        }
                    }
                }
                if !reliable {
                    self.defer();
                } else if let Some(child) = found
                    && !self.declaration(model, child)
                {
                    return false;
                }
            }
            if !skip
                && !choice
                && let Some(child) = node.first
            {
                current = Some(child);
                continue;
            }
            let mut up = id;
            loop {
                if !self.charge(1) {
                    return self.stop(model, Some(id), Stop::Ancestry);
                }
                if up == root {
                    current = None;
                    break;
                }
                let ancestor = &model.nodes[up];
                if let Some(next) = ancestor.next {
                    current = Some(next);
                    break;
                }
                up = ancestor.parent.expect("reachable effective ancestry");
            }
        }
        true
    }
    fn population(&mut self, models: &[crate::EffectiveModel<'a>]) -> bool {
        for model in models {
            if !self.charge(1) {
                return self.stop(model, model.selected, Stop::Model);
            }
            if !model.complete_apart_from_required() || model.dependency_unknown {
                self.defer();
            }
            let Some(root) = model.selected else {
                self.defer();
                continue;
            };
            if !self.tree(model, root) {
                return false;
            }
            let mut cursor = (0, 0);
            loop {
                match model.next_assessment_alternative(&mut cursor, &mut self.budget.work) {
                    Ok(Some(branch)) => {
                        if !self.tree(model, branch) {
                            return false;
                        }
                    }
                    Ok(None) => break,
                    Err(()) => {
                        return self.stop(model, Some(root), Stop::Alternative);
                    }
                }
            }
        }
        true
    }
}

pub(crate) fn scan<'a>(
    catalogue: &'a crate::Catalogue,
    models: &[crate::EffectiveModel<'a>],
    state: &mut State,
    registry: &mut crate::registry::State,
    structure: &mut crate::structure::Structure<'a>,
    budget: &mut crate::allowance::OperationBudget,
    scope_incomplete: bool,
) {
    let retained =
        structure.findings.capacity() * std::mem::size_of::<crate::structure::Finding<'_>>();
    let mut detail = catalogue
        .limits
        .detail_bytes
        .checked_sub(retained)
        .expect("retained finding capacity was precharged");
    // Unit replacement strings remain live. The shared detail allowance cannot
    // be renewed when the next explicit module begins.
    for finding in &structure.findings {
        if !crate::allowance::charge(&mut budget.work, 1) {
            state.defer();
            return;
        }
        detail = detail
            .checked_sub(finding.replacement.as_ref().map_or(0, String::capacity))
            .expect("retained replacements were precharged");
    }
    let mut scanner = Scanner {
        state,
        registry,
        budget,
        structure,
        catalogue,
        detail,
        phase: Phase::Grammar,
    };
    if scope_incomplete {
        scanner.state.defer();
    }
    // Operational completion is independent of monotonic semantic unknowns.
    // A complete first pass with no reliable queries needs no registry replay.
    if !scanner.population(models) || scanner.state.needed == 0 {
        return;
    }
    scanner.phase = Phase::Registry;
    scanner.population(models);
}

#[cfg(test)]
mod temporary_tests {
    #[test]
    fn public_operations_cover_every_registry_phase_stop_without_erasing_syntax() {
        use super::*;
        use crate::{
            Assessment, AssessmentStatus, Catalogue, CheckRequest, Field, Limits, SourceInput,
        };
        let bytes = br#"{"info":{},"sdfData":{"A":{"contentFormat":"application/json;p=x@gzip@identity"},"B":{"contentFormat":"65536"},"C":{"contentFormat":"text/plain;"},"D":{"contentFormat":"60","sdfChoice":{"One":{},"Two":{"contentFormat":"application/json@zstd"}}}}}"#;
        let other = br#"{"info":{},"sdfData":{"Other":{"contentFormat":"60"}}}"#;
        let expected = [
            Stop::Model,
            Stop::Node,
            Stop::Member,
            Stop::Classification,
            Stop::Lookup,
            Stop::Identity,
            Stop::Ancestry,
            Stop::Alternative,
        ];
        let mut seen = [None; 8];
        for work in 1..25000 {
            LAST_STOP.set(None);
            let catalogue = Catalogue::with_limits(
                &[
                    SourceInput::supplied("authored", bytes),
                    SourceInput::supplied("other", other),
                ],
                Limits::default().decoding_work(work),
            )
            .unwrap();
            let report = catalogue
                .check(CheckRequest::new().assessment(Assessment::ExpandedContentFormat))
                .unwrap();
            if let Some((Phase::Registry, site)) = LAST_STOP.get() {
                assert!(
                    matches!(
                        report.assessment_area_status(AREAS[0]),
                        Field::Present(AssessmentStatus::Assessed)
                    ),
                    "{site:?} at{work}"
                );
                assert!(matches!(
                    report.assessment_area_status(AREAS[1]),
                    Field::Present(AssessmentStatus::Undetermined)
                ));
                assert_eq!(
                    report.outcome().errors(),
                    2,
                    "later spelling and range errors survive {site:?}"
                );
                let index = expected.iter().position(|&item| item == site).unwrap();
                seen[index].get_or_insert(work);
                if seen.iter().all(Option::is_some) {
                    break;
                }
            }
        }
        println!("actual public registry-phase stop boundaries: {expected:?} ->{seen:?}");
        assert!(
            seen.iter().all(Option::is_some),
            "every actual stop class exercised"
        );
    }

    #[test]
    fn actual_two_phase_temporary_inventory_fits_existing_common_slots() {
        use super::*;
        use std::mem::{size_of, size_of_val};
        assert!(temporary_storage_supported());
        crate::registry::qualify().unwrap();
        let slot =
            128 + size_of::<usize>() + size_of::<String>() + size_of::<Vec<serde_json::Value>>();
        let inventory = temporary_inventory();
        println!(
            "two-phase Scanner={} Declaration={} OptionalDeclaration={} cursor={} SplitSkip={} inventory={inventory:?} total={} original_slots={} slot={slot} State={} OptionState={} Report={}",
            size_of::<Scanner<'_, '_>>(),
            size_of::<grammar::Declaration<'_>>(),
            size_of::<Option<grammar::Declaration<'_>>>(),
            size_of::<(usize, usize)>(),
            size_of::<std::iter::Skip<std::str::Split<'_, char>>>(),
            inventory.iter().sum::<usize>(),
            8 * slot,
            size_of::<State>(),
            size_of::<Option<State>>(),
            size_of::<crate::Report<'_>>()
        );
        assert_eq!(size_of::<State>(), 24);
        assert_eq!(size_of::<Option<State>>(), 28);
        let catalogue = crate::Catalogue::from_inputs(&[crate::SourceInput::supplied(
            "fixture",
            br#"{"info":{}}"#,
        )])
        .unwrap();
        let mut state = State::default();
        let mut registry = crate::registry::State::default();
        let mut budget = crate::allowance::OperationBudget::new(&catalogue.limits);
        let mut structure = crate::structure::Structure::default();
        let scanner = Scanner {
            state: &mut state,
            registry: &mut registry,
            budget: &mut budget,
            structure: &mut structure,
            catalogue: &catalogue,
            detail: 0,
            phase: Phase::Registry,
        };
        let text = "application/json;p=\"@\"@gzip@deflate";
        let declaration = grammar::classify(text).unwrap();
        let cursor = (0usize, 0usize);
        let codings = "@gzip@deflate".split('@').skip(1);
        let indices = [0usize; 3];
        // Keep the actual scanner, borrowed classification, cursor, coding
        // iterator and lookup scalars live together. No changed walk executes.
        assert!(scanner.phase == Phase::Registry);
        std::hint::black_box((&scanner, &declaration, &cursor, &codings, &indices));
        assert!(
            size_of_val(&scanner)
                + size_of_val(&declaration)
                + size_of_val(&cursor)
                + size_of_val(&codings)
                + size_of_val(&indices)
                <= 8 * slot
        );
        assert!(crate::source::common_report_supported());
    }
}

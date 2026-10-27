//! Full checks of Recorded extension vocabulary at each effective coordinate.
//! Follows ADR 0038: Recorded extension vocabulary, with the per-coordinate checking
//! and recorded evidence of ADR 0027: Unit names and recorded registry guidance and
//! the `uint` domain of ADR 0032: Formal model admission.
use crate::AssessmentStatus::{Assessed, Undetermined};
use crate::coordinate_scan::Visitor;
use crate::formal::Verdict;
use crate::inventory::Requirement;
use crate::registry::Facts;
use crate::role::Vocabulary;
use crate::{Field, JsonKind};

/// The registry knowledge bit of the recorded OMNA SDF Quality Names evidence.
const RECORD: u8 = crate::registry::bits(crate::registry::RECORDED_EXTENSION_RECORDS);
pub(crate) static FACTS: [Facts; 1] = [Facts {
    registry: "omna.sdf-quality-names",
    snapshot: "omna.sdf-quality-names/2026-10-16/58b2e81f1a27e35f",
    scope: "oma-quality-names",
    extraction_policy: "esdieff.recorded-extension-vocabulary/1",
    digest: "58b2e81f1a27e35f2156f79c3361dc4fcd57d353831d3e62d466dec021392656",
    raw_digest: "98098ac43cd36acca64e73e95b78bc3caca10f9c9d174f431c935d413423a62f",
    source_updated: "2026-10-15",
    retrieved_at: "2026-10-16T08:38:45Z",
    entries_count: 1,
}];

struct Scanner<'a, 'b> {
    registry: &'b mut crate::registry::State,
    budget: &'b mut crate::allowance::OperationBudget,
    structure: &'b mut crate::structure::Structure<'a>,
    catalogue: &'a crate::Catalogue,
    numbers: crate::NumberWork,
    detail: usize,
}
impl<'a> Scanner<'a, '_> {
    fn observe(&mut self, status: crate::AssessmentStatus) {
        self.structure.coverage.observe(Requirement::M028, status);
    }
    /// An `oma:id` value outside the recorded `uint` domain is an error at this effective
    /// coordinate, and its conclusion is assessed.
    fn excluded(&mut self, model: &crate::EffectiveModel<'a>, node: usize) {
        self.observe(Assessed);
        let rule = crate::structure::Rule::RecordedValueExcluded;
        if !crate::coordinate_scan::admit(
            self.structure,
            self.catalogue.limits.conditions,
            &mut self.detail,
            rule.severity(),
        ) {
            return;
        }
        self.structure.findings.push(crate::structure::Finding {
            json: model.nodes[node].source,
            effective: Some((model.source, node)),
            rule,
            proof: None,
            replacement: None,
            coverage_requirement: Some(Requirement::M028),
        });
    }
    fn value(&mut self, model: &crate::EffectiveModel<'a>, node: usize) {
        let quality = &model.nodes[node];
        if quality.unavailable.is_some()
            || quality.meaning_unavailable.is_some()
            || quality.donor_unavailable.is_some()
        {
            self.defer();
            return;
        }
        if self.registry.policy == crate::RecordedRegistryPolicy::WithholdRecorded {
            self.registry.withheld |= RECORD;
            self.defer();
            return;
        }
        let verdict = match quality.source.number() {
            Field::Present(number) => {
                crate::formal::uint(number, &mut self.budget.work, &mut self.numbers)
            }
            _ if quality.source.kind() == JsonKind::Number => Verdict::Unavailable,
            _ => Verdict::Excluded,
        };
        if verdict != Verdict::Unavailable {
            self.registry.consulted |= RECORD;
        }
        match verdict {
            Verdict::Admitted => self.observe(Assessed),
            Verdict::Excluded => self.excluded(model, node),
            Verdict::Unavailable => self.defer(),
        }
    }
}
impl<'a> Visitor<'a> for Scanner<'a, '_> {
    fn work(&mut self) -> &mut usize {
        &mut self.budget.work
    }
    fn defer(&mut self) {
        self.observe(Undetermined);
    }
    fn exhausted(&mut self, _: &crate::EffectiveModel<'a>, _: usize) {
        self.defer();
    }
    /// Checks a Recorded extension quality at a position its registration names.
    fn visit(&mut self, model: &crate::EffectiveModel<'a>, id: usize, _: bool) -> bool {
        let node = &model.nodes[id];
        if !node.deleted
            && let Some(key) = node.key
            && node
                .parent
                .and_then(|parent| model.nodes[parent].role)
                .is_some_and(|role| role.vocabulary(key, false) == Vocabulary::Recorded)
        {
            self.value(model, id);
        }
        true
    }
}

/// Checks every Recorded extension quality of the final effective coordinates, including
/// copied and overriding values at each consuming coordinate and final alternatives.
pub(crate) fn scan<'a>(
    catalogue: &'a crate::Catalogue,
    models: &[crate::EffectiveModel<'a>],
    registry: &mut crate::registry::State,
    structure: &mut crate::structure::Structure<'a>,
    budget: &mut crate::allowance::OperationBudget,
    scope_incomplete: bool,
) {
    // Retained findings and unit replacement strings already use the separate detail
    // allowance, which this scan cannot renew.
    let retained =
        structure.findings.capacity() * std::mem::size_of::<crate::structure::Finding<'_>>();
    let mut detail = catalogue
        .limits
        .detail_bytes
        .checked_sub(retained)
        .expect("retained finding capacity was precharged");
    for finding in &structure.findings {
        if !crate::allowance::charge(&mut budget.work, 1) {
            structure.coverage.observe(Requirement::M028, Undetermined);
            return;
        }
        detail = detail
            .checked_sub(finding.replacement.as_ref().map_or(0, String::capacity))
            .expect("retained replacements were precharged");
    }
    let mut scanner = Scanner {
        registry,
        budget,
        structure,
        catalogue,
        numbers: crate::NumberWork::from_remaining(catalogue.numeric_remaining),
        detail,
    };
    crate::coordinate_scan::scan(&mut scanner, models, scope_incomplete);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Class;
    use crate::role::Role;
    use sha2::{Digest, Sha256};

    #[test]
    fn recorded_vocabulary_matches_the_captured_registrations_and_canonical_bytes() {
        let xml = include_str!("../tests/fixtures/recorded-extension/iana-sdf.xml");
        let text = include_str!("../tests/fixtures/recorded-extension/omna-sdf-quality-names.txt");
        let canonical = include_bytes!(
            "../tests/fixtures/recorded-extension/omna-sdf-quality-names.records.tsv"
        );
        let digest = |bytes: &[u8]| {
            Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        assert_eq!(
            digest(xml.as_bytes()),
            "0fdb79625d83a034b53ef15a007bb07a0e695bb58a60996866641acb5c06333e"
        );
        assert_eq!(
            digest(text.as_bytes()),
            "42c8be5e61ce56b45529160ec4e51d9cdaa4780cd5099948209f8c61c5b80b2f"
        );
        // The IANA prefix registry holds one record, `oma`, referring to the OMNA registry.
        let prefixes = xml
            .split_once(r#"<registry id="sdf-quality-name-prefixes">"#)
            .unwrap()
            .1
            .split_once("</registry>")
            .unwrap()
            .0;
        assert_eq!(prefixes.matches("<record").count(), 1);
        assert!(prefixes.contains(r#"<record date="2026-05-12">"#));
        assert!(prefixes.contains("<value>oma</value>"));
        assert!(prefixes.contains(
            r#"data="https://www.openmobilealliance.org/specifications/registries/sdfqualitynames""#
        ));
        // The OMNA table lists one Quality Name, and its CDDL extends three sockets.
        let lines: Vec<_> = text.lines().collect();
        let table = lines
            .iter()
            .position(|line| *line == "Applicable for")
            .unwrap();
        assert_eq!(lines[table + 1], "id");
        assert_eq!(
            lines[table + 3],
            "sdfObject, sdfProperty, sdfAction, sdfData"
        );
        let mut sockets = Vec::new();
        for line in &lines {
            let Some((socket, rest)) = line.split_once(" //= (") else {
                continue;
            };
            assert_eq!(rest, r#""oma:id" => uint)"#);
            sockets.push(socket);
        }
        assert_eq!(
            sockets,
            [
                "$$SDF-EXTENSION-OBJECT",
                "$$SDF-EXTENSION-DATA",
                "$$SDF-EXTENSION-ACTION"
            ]
        );
        let record = format!("oma:id\t{}\tuint\n", sockets.join(","));
        assert_eq!(record.as_bytes(), canonical);
        let facts = &FACTS[0];
        assert_eq!(digest(canonical), facts.digest);
        assert_eq!(
            facts.snapshot,
            format!("{}/2026-10-16/{}", facts.registry, &facts.digest[..16])
        );
        assert_eq!(facts.entries_count, 1);
        assert!(facts.retrieved_at.starts_with("2026-10-16"));
        assert_eq!(
            crate::registry::facts(crate::registry::RECORDED_EXTENSION_RECORDS.start)
                .unwrap()
                .registry,
            facts.registry
        );
        // RFC 9880 places the object socket in sdfObject, the action socket in sdfAction and
        // the data socket in every full data description.
        for role in [
            Role::Definition(Class::Object),
            Role::Definition(Class::Action),
            Role::Definition(Class::Property),
            Role::Definition(Class::Data),
            Role::Data,
        ] {
            assert!(role.vocabulary("oma:id", false) == Vocabulary::Recorded);
            assert!(role.vocabulary("oma:name", false) == Vocabulary::Qualified);
        }
        for role in [
            Role::Document,
            Role::Information,
            Role::Definition(Class::Thing),
            Role::Definition(Class::Event),
            Role::DirectItem,
        ] {
            assert!(role.vocabulary("oma:id", false) == Vocabulary::Qualified);
        }
    }
}

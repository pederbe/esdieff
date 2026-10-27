//! Full-role unit observations and dated positive membership.
//! Follows ADR 0027: Unit names and recorded registry guidance.
use crate::registry::Facts;
use crate::{AssessmentStatus, Field};
use serde_json::{Map, Value};
pub(crate) const AREAS: [&str; 2] = ["unit-spelling/1", "unit-registry-guidance/1"];
const UNAVAILABLE_REASONS: [(u8, &str); 3] = [
    (1, "caller_withheld"),
    (2, "operation_limited"),
    (4, "index_meaning_unavailable"),
];
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Counter {
    KnownPrimary,
    KnownSecondary,
    Unknown,
    Unavailable,
    Legacy,
    UriNames,
}
impl Counter {
    const ALL: [Self; 6] = [
        Self::KnownPrimary,
        Self::KnownSecondary,
        Self::Unknown,
        Self::Unavailable,
        Self::Legacy,
        Self::UriNames,
    ];
    fn index(self) -> usize {
        match self {
            Self::KnownPrimary => 0,
            Self::KnownSecondary => 1,
            Self::Unknown => 2,
            Self::Unavailable => 3,
            Self::Legacy => 4,
            Self::UriNames => 5,
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::KnownPrimary => "known_primary",
            Self::KnownSecondary => "known_secondary",
            Self::Unknown => "unknown",
            Self::Unavailable => "unavailable",
            Self::Legacy => "legacy",
            Self::UriNames => "uri_names",
        }
    }
}
pub(crate) const RAW_DIGEST: &str =
    "3eff03640c5a5d2abd09e330488aeb9fae0b2766ea69b1114295bbec5c44e754";
pub(crate) static FACTS: [Facts; 2] = [
    Facts {
        registry: "iana.senml-units",
        snapshot: "iana.senml-units/2026-10-05/6246352c590e933b",
        scope: "senml-units",
        extraction_policy: "esdieff.unit-section-positives/1",
        digest: "6246352c590e933b72c25ef9f363171e40fea347bb7e23d08b04f9436894ed34",
        raw_digest: RAW_DIGEST,
        source_updated: "2026-02-02",
        retrieved_at: "2026-10-05T16:49:51Z",
        entries_count: 67,
    },
    Facts {
        registry: "iana.secondary-units",
        snapshot: "iana.secondary-units/2026-10-05/de5c5c009ffbc61a",
        scope: "secondary-units",
        extraction_policy: "esdieff.unit-section-positives/1",
        digest: "de5c5c009ffbc61aa66e71ee55065f95108709575906c6ec7cd64a2bbc7e173b",
        raw_digest: RAW_DIGEST,
        source_updated: "2026-02-02",
        retrieved_at: "2026-10-05T16:49:51Z",
        entries_count: 39,
    },
];
#[derive(Default)]
pub(crate) struct State {
    pub(crate) counts: [u32; 6],
    pub(crate) applicable: bool,
    pub(crate) spelling_unknown: bool,
    pub(crate) guidance_unknown: bool,
    pub(crate) reasons: u8,
}
impl State {
    pub(crate) fn count(&self, counter: Counter) -> u32 {
        self.counts[counter.index()]
    }
    pub(crate) fn unavailable_reasons(&self) -> impl Iterator<Item = &'static str> + '_ {
        UNAVAILABLE_REASONS
            .iter()
            .filter_map(|&(bit, reason)| (self.reasons & bit != 0).then_some(reason))
    }
    pub(crate) fn status(&self, area: usize) -> AssessmentStatus {
        if if area == 0 {
            self.spelling_unknown
        } else {
            self.guidance_unknown
        } {
            AssessmentStatus::Undetermined
        } else if self.applicable {
            AssessmentStatus::Assessed
        } else {
            AssessmentStatus::NotApplicable
        }
    }
    pub(crate) fn wire(&self) -> Value {
        let mut counts = Map::new();
        for counter in Counter::ALL {
            counts.insert(counter.key().to_owned(), Value::from(self.count(counter)));
        }
        let mut map = Map::new();
        for (key, value) in [
            (
                "state",
                if self.spelling_unknown || self.guidance_unknown {
                    "partial"
                } else {
                    "complete"
                },
            ),
            ("spelling_status", self.status(0).as_str()),
            ("guidance_status", self.status(1).as_str()),
        ] {
            map.insert(key.to_owned(), Value::String(value.to_owned()));
        }
        map.insert("counts".to_owned(), Value::Object(counts));
        let mut reasons = Vec::with_capacity(4);
        for reason in self.unavailable_reasons() {
            reasons.push(Value::String(reason.to_owned()));
        }
        map.insert("unavailable_reasons".to_owned(), Value::Array(reasons));
        Value::Object(map)
    }
}
/// Borrowed counts of final effective Full unit qualities, never retained detail.
pub struct UnitGuidance<'a> {
    pub(crate) state: &'a State,
    pub(crate) registry: &'a crate::registry::State,
}
impl UnitGuidance<'_> {
    pub fn complete(&self) -> bool {
        !self.state.spelling_unknown && !self.state.guidance_unknown
    }
    pub fn known_primary(&self) -> u32 {
        self.state.count(Counter::KnownPrimary)
    }
    pub fn known_secondary(&self) -> u32 {
        self.state.count(Counter::KnownSecondary)
    }
    pub fn unknown(&self) -> u32 {
        self.state.count(Counter::Unknown)
    }
    pub fn unavailable(&self) -> u32 {
        self.state.count(Counter::Unavailable)
    }
    pub fn legacy(&self) -> u32 {
        self.state.count(Counter::Legacy)
    }
    pub fn uri_names(&self) -> u32 {
        self.state.count(Counter::UriNames)
    }
    /// The common raw XML edition, independently of which section was consulted.
    pub fn source_edition(&self) -> Field<UnitRegistrySourceEditionView> {
        if self.registry.consulted & 3 != 0 {
            Field::Present(UnitRegistrySourceEditionView)
        } else if self.registry.withheld & 3 != 0 {
            Field::Unavailable("caller withheld recorded evidence")
        } else {
            Field::Absent
        }
    }
}
pub struct UnitRegistrySourceEditionView;
impl UnitRegistrySourceEditionView {
    pub fn id(&self) -> &'static str {
        "iana.senml-units/2026-10-05/3eff03640c5a5d2a"
    }
    pub fn raw_digest(&self) -> &'static str {
        RAW_DIGEST
    }
    pub fn source_updated(&self) -> &'static str {
        "2026-02-02"
    }
    pub fn retrieved_at(&self) -> &'static str {
        "2026-10-05T16:49:51Z"
    }
    pub fn source_uri(&self) -> &'static str {
        "https://www.iana.org/assignments/senml/senml.xml"
    }
}
pub(crate) fn unavailable_wire() -> Value {
    let mut map = Map::new();
    for (key, value) in [
        ("state", "unavailable"),
        ("spelling_status", "undetermined"),
        ("guidance_status", "undetermined"),
        ("reason", "owner_limit"),
    ] {
        map.insert(key.to_owned(), Value::String(value.to_owned()));
    }
    map.insert("counts".to_owned(), Value::Null);
    map.insert("unavailable_reasons".to_owned(), Value::Array(Vec::new()));
    Value::Object(map)
}

mod table;
#[derive(Clone, Copy)]
pub(crate) enum Rule {
    Forbidden,
    IndexSyntax,
    UrnComponents,
    UrnDelimiterAdvice,
    UriSyntax,
    Unknown,
    Secondary,
    Legacy,
    UriName,
    MarkerCase,
    MarkerEncoding,
    IndexMeaning,
    Work,
}
impl Rule {
    pub(crate) fn code(self) -> &'static str {
        self.descriptor().code
    }
    pub(crate) fn message(self) -> &'static str {
        self.descriptor().message
    }
    pub(crate) fn citation(self) -> Option<&'static str> {
        self.descriptor().citation
    }
    pub(crate) fn severity(self) -> &'static str {
        self.descriptor().severity
    }
    pub(crate) fn basis_kind(self) -> &'static str {
        self.descriptor().basis_kind
    }
    pub(crate) fn basis_id(self) -> Option<&'static str> {
        self.descriptor().basis_id
    }
    pub(crate) fn area(self) -> &'static str {
        self.descriptor().area
    }
    fn descriptor(self) -> &'static RuleDescriptor {
        match self {
            Self::Forbidden => {
                &const {
                    RuleDescriptor::rfc(
                        "unit.urn_forbidden",
                        "Use the represented unit name directly: percent-decode its index once, preserving case and scale, if that is the intended unit; otherwise use a suitable definition URI outside this subnamespace. Suffixes are not part of the unit symbol",
                        "RFC 9880 Section 4.7",
                    )
                }
            }
            Self::IndexSyntax => {
                &const {
                    RuleDescriptor::rfc(
                        "unit.index_syntax",
                        "A unit index must use pchar spelling; encode literal slash as %2F and keep optional URN components separate",
                        "RFC 9880 Section 7.3",
                    )
                }
            }
            Self::UrnComponents => {
                &const {
                    RuleDescriptor::rfc(
                        "unit.urn_components_syntax",
                        "Optional URN components must follow RFC 8141 grammar: nonempty r or q components begin with pchar, with r before q",
                        "RFC 8141 Section 2",
                    )
                }
            }
            Self::UrnDelimiterAdvice => {
                &const {
                    RuleDescriptor::advice(
                        "unit.urn_delimiter_advice",
                        "RFC 8141 recommends treating a top-level question mark outside ?+ or ?= components as a URN syntax error; no represented unit symbol was assumed",
                        "RFC 8141 Section 2",
                        AREAS[0],
                    )
                }
            }
            Self::UriSyntax => {
                &const {
                    RuleDescriptor::rfc(
                        "unit.uri_syntax",
                        "A colon-bearing unit name must be a syntactically valid full URI",
                        "RFC 9880 Sections 1.2.4 and 4.7; RFC 3986 Section 3",
                    )
                }
            }
            Self::Unknown => {
                &const {
                    RuleDescriptor::advice(
                        "unit.registration_unknown",
                        "This exact name has no positive match in the recorded SenML and secondary tables; current registration remains unknown. Consider a registered unit or an appropriate definition URI",
                        "RFC 9880 Section 4.7; RFC 8428 Section 12.1; RFC 8798 Section 3",
                        AREAS[1],
                    )
                }
            }
            Self::Secondary => {
                &const {
                    RuleDescriptor::advice(
                        "unit.secondary",
                        "The name has a positive match in the recorded Secondary Units table; prefer a suitable SenML unit when available, without silently changing scale",
                        "RFC 9880 Section 4.7; RFC 8428 Section 12.1; RFC 8798 Section 3",
                        AREAS[1],
                    )
                }
            }
            Self::Legacy => {
                &const {
                    RuleDescriptor::advice(
                        "unit.legacy",
                        "The recorded SenML entry is marked for legacy use; prefer a suitable nonlegacy unit without silently renaming or scaling this declaration",
                        "RFC 9880 Section 4.7; RFC 8428 Section 12.1; RFC 8798 Section 3",
                        AREAS[1],
                    )
                }
            }
            Self::UriName => {
                &const {
                    RuleDescriptor::advice(
                        "unit.uri_name",
                        "A URI unit identifies a unit definition when no suitable registered name is available; URI syntax alone does not establish its definition or assignment",
                        "RFC 9880 Section 4.7; RFC 8428 Section 12.1; RFC 8798 Section 3",
                        AREAS[0],
                    )
                }
            }
            Self::MarkerCase => {
                &const {
                    RuleDescriptor::interpretation(
                        "unit.marker_case_unresolved",
                        "Unit namespace marker case is unresolved because the ietf and params namespace case rules conflict; no marker or symbol normalization was applied",
                        "unit-marker-identity/1",
                    )
                }
            }
            Self::MarkerEncoding => {
                &const {
                    RuleDescriptor::interpretation(
                        "unit.marker_encoding_unresolved",
                        "Encoded namespace markers do not establish unit subnamespace identity; no NSS equivalence or marker decoding was assumed",
                        "unit-marker-identity/1",
                    )
                }
            }
            Self::IndexMeaning => {
                &const {
                    RuleDescriptor::interpretation(
                        "unit.index_meaning_unavailable",
                        "The unit index has no established nonempty UTF-8 symbol meaning; no registration or spelling conclusion was invented",
                        "unit-index-mapping/1",
                    )
                }
            }
            Self::Work => &RuleDescriptor {
                code: "operation.limit",
                message: "Unit assessment reached its shared finite work limit; established earlier findings and counts remain available",
                citation: None,
                severity: "incomplete",
                basis_kind: "operational",
                basis_id: Some("unit-establishment"),
                area: AREAS[0],
            },
        }
    }
}
struct RuleDescriptor {
    code: &'static str,
    message: &'static str,
    citation: Option<&'static str>,
    severity: &'static str,
    basis_kind: &'static str,
    basis_id: Option<&'static str>,
    area: &'static str,
}
impl RuleDescriptor {
    const fn rfc(code: &'static str, message: &'static str, citation: &'static str) -> Self {
        Self {
            code,
            message,
            citation: Some(citation),
            severity: "error",
            basis_kind: "rfc",
            basis_id: None,
            area: AREAS[0],
        }
    }
    const fn advice(
        code: &'static str,
        message: &'static str,
        citation: &'static str,
        area: &'static str,
    ) -> Self {
        Self {
            code,
            message,
            citation: Some(citation),
            severity: "notice",
            basis_kind: "advice",
            basis_id: Some(code),
            area,
        }
    }
    const fn interpretation(
        code: &'static str,
        message: &'static str,
        basis_id: &'static str,
    ) -> Self {
        Self {
            code,
            message,
            citation: None,
            severity: "incomplete",
            basis_kind: "interpretation",
            basis_id: Some(basis_id),
            area: AREAS[0],
        }
    }
}
impl State {
    fn increment(&mut self, counter: Counter) {
        let index = counter.index();
        self.counts[index] = self.counts[index]
            .checked_add(1)
            .expect("one precharged observation below work ceiling");
    }
    fn defer(&mut self) {
        self.spelling_unknown = true;
        self.guidance_unknown = true;
    }
    fn unavailable(&mut self, reason: u8) {
        self.increment(Counter::Unavailable);
        self.guidance_unknown = true;
        self.reasons |= reason;
    }
}
struct Scanner<'a, 'b> {
    state: &'b mut State,
    registry: &'b mut crate::registry::State,
    budget: &'b mut crate::allowance::OperationBudget,
    structure: &'b mut crate::structure::Structure<'a>,
    catalogue: &'a crate::Catalogue,
    detail: usize,
}
impl<'a> Scanner<'a, '_> {
    fn charge(&mut self, amount: usize) -> bool {
        crate::allowance::charge(&mut self.budget.work, amount)
    }
    fn record(&mut self, model: &crate::EffectiveModel<'a>, node: usize, rule: Rule) {
        if !crate::coordinate_scan::admit(
            self.structure,
            self.catalogue.limits.conditions,
            &mut self.detail,
            rule.severity(),
        ) {
            return;
        }
        let replacement = if matches!(rule, Rule::Forbidden) {
            let text = model.nodes[node]
                .source
                .text()
                .present()
                .expect("established text");
            let index = text
                .splitn(5, ':')
                .nth(4)
                .expect("recognized unit index")
                .split(['?', '#'])
                .next()
                .expect("index");
            if crate::allowance::charge(&mut self.detail, index.len()) {
                let mut value = Vec::with_capacity(index.len());
                // UTF-8 and the complete copy pass were already precharged by
                // unit's finite text coefficient; allocation is optional detail.
                for byte in decoded_bytes(index, true) {
                    value.push(byte);
                }
                Some(String::from_utf8(value).expect("validated UTF-8"))
            } else {
                self.structure.bytes_omitted = true;
                None
            }
        } else {
            None
        };
        self.structure.findings.push(crate::structure::Finding {
            json: model.nodes[node].source,
            effective: Some((model.source, node)),
            rule: crate::structure::Rule::Unit(rule),
            proof: None,
            replacement,
            coverage_requirement: Some(crate::inventory::Requirement::M023),
        });
    }
    fn lookup(&mut self, text: &str, encoded: bool) -> Result<(Counter, bool), ()> {
        lookup(text, encoded, self.registry, &mut self.budget.work)
            .map_err(|unfinished| self.state.unavailable(unfinished as u8))
    }
    fn unit(&mut self, model: &crate::EffectiveModel<'a>, node: usize) -> bool {
        self.state.applicable = true;
        let Some(text) = model.nodes[node].source.text().present() else {
            return true;
        };
        // The fixed multiple covers generic grammar, marker/suffix classification,
        // pchar validation and streaming UTF-8/index passes, before examining text.
        if !self.charge(1 + text.len() * 32) {
            self.state.defer();
            self.record(model, node, Rule::Work);
            return false;
        }
        let mut index = text;
        let mut encoded = false;
        if text.contains(':') {
            if !crate::uri::valid_uri(text) {
                self.record(model, node, Rule::UriSyntax);
                return true;
            }
            self.state.increment(Counter::UriNames);
            let (scheme, rest) = text.split_once(':').expect("colon");
            let Some((nid, nss)) = rest.split_once(':') else {
                self.record(model, node, Rule::UriName);
                return true;
            };
            if !scheme.eq_ignore_ascii_case("urn") || !nid.eq_ignore_ascii_case("ietf") {
                self.record(model, node, Rule::UriName);
                return true;
            }
            let assigned = nss.split(['?', '#']).next().expect("assigned name");
            let mut parts = assigned.splitn(3, ':');
            let params = parts.next().unwrap_or("");
            let unit = parts.next().unwrap_or("");
            if params.eq_ignore_ascii_case("params") && unit.eq_ignore_ascii_case("unit") {
                if params != "params" || unit != "unit" {
                    self.registry.interpretations |= 2;
                    self.state.defer();
                    self.record(model, node, Rule::MarkerCase);
                    return true;
                }
            } else if (params.contains('%')
                && marker_candidate(params, "params")
                && marker_candidate(unit, "unit"))
                || (unit.contains('%')
                    && marker_candidate(params, "params")
                    && marker_candidate(unit, "unit"))
                || assigned.contains('%')
                    && decoded_bytes(assigned, true)
                        .map(|b| b.to_ascii_lowercase())
                        .take(12)
                        .eq(b"params:unit:".iter().copied())
            {
                self.registry.interpretations |= 2;
                self.state.defer();
                self.record(model, node, Rule::MarkerEncoding);
                return true;
            } else {
                self.record(model, node, Rule::UriName);
                return true;
            }
            index = parts.next().unwrap_or("");
            encoded = true;
            if !index
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@%".contains(&b))
            {
                self.record(model, node, Rule::IndexSyntax);
                return true;
            }
            let head = nss.split('#').next().unwrap_or(nss);
            if head
                .split_once('?')
                .is_some_and(|(_, suffix)| !suffix.starts_with('+') && !suffix.starts_with('='))
            {
                self.record(model, node, Rule::UrnDelimiterAdvice);
                return true;
            }
            if !urn_suffix(nss) {
                self.record(model, node, Rule::UrnComponents);
                return true;
            }
            self.registry.interpretations |= 1;
            let Some(colon) = index_meaning(index) else {
                self.state.defer();
                self.state.unavailable(4);
                self.record(model, node, Rule::IndexMeaning);
                return true;
            };
            if !colon {
                self.record(model, node, Rule::Forbidden);
            }
        }
        match self.lookup(index, encoded) {
            Ok((outcome, legacy)) => {
                self.state.increment(outcome);
                if legacy {
                    self.state.increment(Counter::Legacy);
                    self.record(model, node, Rule::Legacy);
                }
                if outcome == Counter::KnownSecondary {
                    self.record(model, node, Rule::Secondary);
                }
                if outcome == Counter::Unknown {
                    self.record(model, node, Rule::Unknown);
                }
                true
            }
            Err(()) => {
                if self.state.reasons & 2 != 0 {
                    self.record(model, node, Rule::Work);
                    false
                } else {
                    true
                }
            }
        }
    }
}
impl<'a> crate::coordinate_scan::Visitor<'a> for Scanner<'a, '_> {
    fn work(&mut self) -> &mut usize {
        &mut self.budget.work
    }
    fn defer(&mut self) {
        self.state.defer();
    }
    fn exhausted(&mut self, model: &crate::EffectiveModel<'a>, node: usize) {
        self.state.defer();
        self.record(model, node, Rule::Work);
    }
    /// Checks the one available `unit` of a full data description.
    fn visit(&mut self, model: &crate::EffectiveModel<'a>, id: usize, skip: bool) -> bool {
        let node = &model.nodes[id];
        if skip
            || !node.role.is_some_and(|r| r.full_data())
            || node.source.kind() != crate::JsonKind::Map
        {
            return true;
        }
        let mut next = node.first;
        let mut found = None;
        let mut reliable = true;
        while let Some(child) = next {
            let quality = &model.nodes[child];
            next = quality.next;
            if !self.charge(1 + quality.key.map_or(0, str::len) + 4) {
                self.state.defer();
                self.record(model, id, Rule::Work);
                return false;
            }
            if quality.deleted {
                continue;
            }
            if quality.key.is_none() {
                reliable = false;
            }
            if quality.key == Some("unit") {
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
            self.state.defer();
            true
        } else if let Some(child) = found {
            self.unit(model, child)
        } else {
            true
        }
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
    // Retained findings have a separate optional allowance; all semantic work
    // uses the original operation budget, with no source/replay refill.
    let retained =
        structure.findings.capacity() * std::mem::size_of::<crate::structure::Finding<'_>>();
    let mut scanner = Scanner {
        state,
        registry,
        budget,
        structure,
        catalogue,
        detail: catalogue
            .limits
            .detail_bytes
            .checked_sub(retained)
            .expect("retained finding capacity was precharged"),
    };
    crate::coordinate_scan::scan(&mut scanner, models, scope_incomplete);
}
/// Why a needed lookup is unfinished, as its `unavailable_reasons` bit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Unfinished {
    Withheld = 1,
    Limited = 2,
}

/// Look up an exact unit name in the recorded SenML section, then in the Secondary Units
/// section, returning the section's counter and the entry's legacy flag. A miss in both is
/// `Counter::Unknown`. Each row is charged before it counts as consulted, and withholding
/// policy refuses the lookup. Model checking and revision comparison share this lookup.
pub(crate) fn lookup(
    text: &str,
    encoded: bool,
    registry: &mut crate::registry::State,
    work: &mut usize,
) -> Result<(Counter, bool), Unfinished> {
    if registry.policy == crate::RecordedRegistryPolicy::WithholdRecorded {
        registry.withheld |= 1;
        return Err(Unfinished::Withheld);
    }
    for (index, (counter, table)) in [
        (Counter::KnownPrimary, table::PRIMARY),
        (Counter::KnownSecondary, table::SECONDARY),
    ]
    .into_iter()
    .enumerate()
    {
        for (symbol, legacy) in table {
            if !crate::allowance::charge(work, 1 + text.len() + symbol.len()) {
                return Err(Unfinished::Limited);
            }
            registry.consulted |= 1 << index;
            if decoded_bytes(text, encoded).eq(symbol.bytes()) {
                return Ok((counter, *legacy));
            }
        }
    }
    Ok((Counter::Unknown, false))
}
fn decoded_bytes(text: &str, encoded: bool) -> impl Iterator<Item = u8> + '_ {
    let mut bytes = text.bytes();
    std::iter::from_fn(move || {
        let byte = bytes.next()?;
        if encoded && byte == b'%' {
            Some(
                hex(bytes.next().expect("validated triplet")) * 16
                    + hex(bytes.next().expect("validated triplet")),
            )
        } else {
            Some(byte)
        }
    })
}
fn hex(byte: u8) -> u8 {
    if byte.is_ascii_digit() {
        byte - b'0'
    } else {
        byte.to_ascii_lowercase() - b'a' + 10
    }
}
fn marker_candidate(text: &str, expected: &str) -> bool {
    decoded_bytes(text, true)
        .map(|b| b.to_ascii_lowercase())
        .eq(expected.bytes())
}
fn index_meaning(text: &str) -> Option<bool> {
    if text.is_empty() {
        return None;
    }
    let mut colon = false;
    let mut character = [0u8; 4];
    let mut have = 0;
    let mut need = 0;
    for byte in decoded_bytes(text, true) {
        colon |= byte == b':';
        if have == 0 {
            need = match byte {
                0..=0x7f => 1,
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => return None,
            };
        }
        character[have] = byte;
        have += 1;
        if have == need {
            std::str::from_utf8(&character[..have]).ok()?;
            have = 0;
        }
    }
    (have == 0).then_some(colon)
}
fn urn_suffix(text: &str) -> bool {
    // Generic URI grammar already validates all component characters/escapes.
    // RFC 8141 permits nonempty r then q, or q, followed by an optional fragment.
    let head = text.split('#').next().unwrap_or(text);
    let Some((_, suffix)) = head.split_once('?') else {
        return true;
    };
    if let Some(r) = suffix.strip_prefix('+') {
        if let Some((r, q)) = r.split_once("?=") {
            component_first(r) && component_first(q)
        } else {
            component_first(r)
        }
    } else if let Some(q) = suffix.strip_prefix('=') {
        component_first(q)
    } else {
        false
    }
}
fn component_first(text: &str) -> bool {
    text.as_bytes()
        .first()
        .is_some_and(|b| *b != b'/' && *b != b'?')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiled_tables_match_every_captured_xml_row_flag_and_canonical_byte() {
        let xml = include_str!("../tests/fixtures/registry/senml.xml");
        let mut section = None;
        let mut symbol = None;
        let mut legacy = false;
        let mut rows = [Vec::new(), Vec::new()];
        for line in xml.lines() {
            let line = line.trim();
            match line {
                "<registry id=\"senml-units\">" => section = Some(0),
                "<registry id=\"secondary-units\">" => section = Some(1),
                "</registry>" => section = None,
                _ => {}
            }
            let Some(index) = section else { continue };
            if line.starts_with("<record") {
                symbol = None;
                legacy = false;
            }
            if let Some(value) = line
                .strip_prefix("<value>")
                .and_then(|s| s.strip_suffix("</value>"))
            {
                assert!(symbol.is_none());
                symbol = Some(value);
            }
            if index == 0 && line.starts_with("<description>") && line.contains('*') {
                legacy = true;
            }
            if line == "</record>" {
                rows[index].push((symbol.take().unwrap(), legacy));
            }
        }
        assert_eq!(rows[0].as_slice(), table::PRIMARY);
        assert_eq!(rows[1].as_slice(), table::SECONDARY);
        for (table, bytes) in [
            (
                table::PRIMARY,
                include_bytes!("../tests/fixtures/registry/senml-units.records.tsv").as_slice(),
            ),
            (
                table::SECONDARY,
                include_bytes!("../tests/fixtures/registry/secondary-units.records.tsv").as_slice(),
            ),
        ] {
            use std::fmt::Write;
            let mut canonical = String::new();
            for (symbol, legacy) in table {
                writeln!(canonical, "{symbol}\t{legacy}").unwrap();
            }
            assert_eq!(canonical.as_bytes(), bytes);
        }
        assert_eq!(
            (
                table::PRIMARY.len(),
                table::SECONDARY.len(),
                table::PRIMARY.iter().filter(|r| r.1).count()
            ),
            (67, 39, 9)
        );
        let mut positive = 0;
        for (index, table) in [table::PRIMARY, table::SECONDARY].into_iter().enumerate() {
            for (symbol, legacy) in table {
                let text =
                    serde_json::json!({"info":{},"sdfData":{"Value":{"unit":symbol}}}).to_string();
                let cat = crate::Catalogue::from_inputs(&[crate::SourceInput::supplied(
                    "fixture",
                    text.as_bytes(),
                )])
                .unwrap();
                let report = cat
                    .check(crate::CheckRequest::new().assessment(crate::Assessment::ExpandedUnits))
                    .unwrap();
                let units = report.unit_guidance().require().unwrap();
                assert_eq!(units.state.counts[index], 1, "{symbol}");
                assert_eq!(units.legacy(), u32::from(*legacy), "{symbol}");
                assert_eq!(report.outcome().errors(), 0);
                positive += 1;
            }
        }
        println!(
            "compiled table and public caller parity: {positive} positives, 67 primary, 39 secondary, nine legacy; canonical bytes630/398"
        );
    }
}

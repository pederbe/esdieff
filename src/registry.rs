//! Recorded request evidence, shared by explicit registry assessments.
//! Follows ADR 0027: Unit names and recorded registry guidance.
use crate::Field;
use serde_json::{Map, Value};
use std::ops::Range;

pub(crate) const UNIT_RESERVE: usize = 21448;
pub(crate) const CONTENT_RESERVE: usize = 19116;
pub(crate) const FULL_RESERVE: usize = 31448;
// Four registry interpretations can coexist with civil, byte, Unix, both UUID,
// copied reference relocation and undeclared extension interpretations appended
// by the Full report producer.
const MAX_FULL_INTERPRETATIONS: usize = 11;
// The longest appended identifier, `undeclared-extensions-ignorable/1`, has 33
// bytes.
const MAX_INTERPRETATION_BYTES: usize = 40;
/// The interpretation bit of content-format identifiers outside the registrable range.
pub(crate) const CONTENT_FORMAT_RANGE: u8 = 4;
/// The interpretation bit of the content-format model meaning rules.
pub(crate) const CONTENT_FORMAT_MEANING: u8 = 8;
const INTERPRETATIONS: [(u8, &str); 4] = [
    (1, "unit-index-mapping/1"),
    (2, "unit-marker-identity/1"),
    (CONTENT_FORMAT_RANGE, "I-CF-RANGE-1"),
    (CONTENT_FORMAT_MEANING, "content-format-model-meaning/1"),
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum RecordedRegistryPolicy {
    #[default]
    UseRecorded,
    WithholdRecorded,
}
impl RecordedRegistryPolicy {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::UseRecorded => "use_recorded",
            Self::WithholdRecorded => "withhold_recorded",
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Facts {
    pub(crate) registry: &'static str,
    pub(crate) snapshot: &'static str,
    pub(crate) scope: &'static str,
    pub(crate) extraction_policy: &'static str,
    pub(crate) digest: &'static str,
    pub(crate) raw_digest: &'static str,
    pub(crate) source_updated: &'static str,
    pub(crate) retrieved_at: &'static str,
    pub(crate) entries_count: u32,
}
/// Each owner's record indexes, in report order: two unit sections, three content-format
/// registries and the recorded extension vocabulary.
pub(crate) const UNIT_RECORDS: Range<usize> = 0..crate::units::FACTS.len();
pub(crate) const CONTENT_RECORDS: Range<usize> =
    UNIT_RECORDS.end..UNIT_RECORDS.end + crate::content_format::FACTS.len();
pub(crate) const RECORDED_EXTENSION_RECORDS: Range<usize> =
    CONTENT_RECORDS.end..CONTENT_RECORDS.end + crate::recorded_extension::FACTS.len();
pub(crate) const RECORDS: usize = RECORDED_EXTENSION_RECORDS.end;
/// The `requested`, `consulted` and `withheld` bits of one owner's records.
pub(crate) const fn bits(records: Range<usize>) -> u8 {
    ((1u16 << records.end) - (1u16 << records.start)) as u8
}
pub(crate) fn facts(index: usize) -> Option<&'static Facts> {
    if index < CONTENT_RECORDS.start {
        crate::units::FACTS.get(index)
    } else if index < RECORDED_EXTENSION_RECORDS.start {
        crate::content_format::FACTS.get(index - CONTENT_RECORDS.start)
    } else {
        crate::recorded_extension::FACTS.get(index - RECORDED_EXTENSION_RECORDS.start)
    }
}
#[derive(Clone, Copy, Default)]
pub(crate) struct State {
    pub(crate) consulted: u8,
    pub(crate) withheld: u8,
    pub(crate) interpretations: u8,
    pub(crate) owner_failed: bool,
    pub(crate) requested: u8,
    pub(crate) policy: RecordedRegistryPolicy,
}
impl State {
    pub(crate) fn view(&self, id: &str) -> Field<RegistryKnowledgeView> {
        let Some((index, facts)) = (0..RECORDS)
            .filter_map(|i| facts(i).map(|f| (i, f)))
            .find(|(_, f)| f.registry == id)
        else {
            return Field::Absent;
        };
        if self.owner_failed {
            return if self.requested & (1 << index) != 0 {
                Field::Unavailable("registry metadata owner limit")
            } else {
                Field::Absent
            };
        }
        if self.consulted & (1 << index) != 0 {
            Field::Present(RegistryKnowledgeView {
                facts,
                available: true,
            })
        } else if self.withheld & (1 << index) != 0 {
            Field::Present(RegistryKnowledgeView {
                facts,
                available: false,
            })
        } else {
            Field::Absent
        }
    }
    pub(crate) fn wire(&self, full: bool) -> Value {
        let mut records = Vec::with_capacity(if full { 8 } else { 4 });
        for index in 0..RECORDS {
            let Some(facts) = facts(index) else { continue };
            if let Field::Present(view) = self.view(facts.registry) {
                records.push(view.wire());
            }
        }
        Value::Array(records)
    }
    pub(crate) fn interpretation_wire(&self) -> Value {
        let mut ids = Vec::with_capacity(4);
        for id in self.interpretation_ids() {
            ids.push(Value::String(id.to_owned()));
        }
        Value::Array(ids)
    }
    pub(crate) fn interpretation_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        INTERPRETATIONS
            .iter()
            .filter_map(|&(bit, id)| (self.interpretations & bit != 0).then_some(id))
    }
    pub(crate) fn write_human(&self, output: &mut crate::human::HumanText) {
        use std::fmt::Write;
        for index in 0..RECORDS {
            let Some(facts) = facts(index) else { continue };
            let Field::Present(view) = self.view(facts.registry) else {
                continue;
            };
            output.heading("Registry");
            if view.available {
                writeln!(
                    output,
                    " {}: available; snapshot {}; scope {}; entries_count {}",
                    facts.registry, facts.snapshot, facts.scope, facts.entries_count
                )
                .expect("String write");
                output.heading("Registry extraction_policy");
                writeln!(
                    output,
                    " {}; digest {}; raw_digest {}; source_updated {}; retrieved_at {}",
                    facts.extraction_policy,
                    facts.digest,
                    facts.raw_digest,
                    facts.source_updated,
                    facts.retrieved_at
                )
                .expect("String write");
            } else {
                writeln!(
                    output,
                    " {}: unavailable; snapshot null; reason caller_withheld",
                    facts.registry
                )
                .expect("String write");
            }
        }
        for id in self.interpretation_ids() {
            output.heading("Interpretation:");
            writeln!(output, " {id}").expect("String write");
        }
    }
}
/// A borrowed observation of admitted evidence for this request. An available
/// record identifies consultation, independently of match or lookup completion.
#[derive(Clone, Copy)]
pub struct RegistryKnowledgeView {
    facts: &'static Facts,
    available: bool,
}
impl RegistryKnowledgeView {
    pub fn registry(&self) -> &'static str {
        self.facts.registry
    }
    pub fn state(&self) -> &'static str {
        if self.available {
            "available"
        } else {
            "unavailable"
        }
    }
    pub fn snapshot(&self) -> Option<&'static str> {
        self.available.then_some(self.facts.snapshot)
    }
    pub fn reason(&self) -> Option<&'static str> {
        (!self.available).then_some("caller_withheld")
    }
    pub fn scope(&self) -> Option<&'static str> {
        self.available.then_some(self.facts.scope)
    }
    pub fn extraction_policy(&self) -> Option<&'static str> {
        self.available.then_some(self.facts.extraction_policy)
    }
    pub fn digest(&self) -> Option<&'static str> {
        self.available.then_some(self.facts.digest)
    }
    pub fn raw_digest(&self) -> Option<&'static str> {
        self.available.then_some(self.facts.raw_digest)
    }
    pub fn source_updated(&self) -> Option<&'static str> {
        self.available.then_some(self.facts.source_updated)
    }
    pub fn retrieved_at(&self) -> Option<&'static str> {
        self.available.then_some(self.facts.retrieved_at)
    }
    pub fn entries_count(&self) -> Option<u32> {
        self.available.then_some(self.facts.entries_count)
    }
    fn wire(self) -> Value {
        let mut map = Map::new();
        for (key, value) in [("registry", self.registry()), ("state", self.state())] {
            map.insert(key.to_owned(), Value::String(value.to_owned()));
        }
        map.insert(
            "snapshot".to_owned(),
            self.snapshot()
                .map_or(Value::Null, |s| Value::String(s.to_owned())),
        );
        if self.available {
            for (key, value) in [
                ("scope", self.facts.scope),
                ("extraction_policy", self.facts.extraction_policy),
                ("digest", self.facts.digest),
                ("raw_digest", self.facts.raw_digest),
                ("source_updated", self.facts.source_updated),
                ("retrieved_at", self.facts.retrieved_at),
            ] {
                map.insert(key.to_owned(), Value::String(value.to_owned()));
            }
            map.insert(
                "entries_count".to_owned(),
                Value::from(self.facts.entries_count),
            );
        } else {
            map.insert(
                "reason".to_owned(),
                Value::String("caller_withheld".to_owned()),
            );
        }
        Value::Object(map)
    }
}

/// Guard the qualified allocation model before any new semantic work. The map
/// bound includes old/new dense and hash storage, controls, descriptor and staged
/// bucket, following the fixed serde_json/IndexMap/hashbrown source inventory.
pub(crate) fn qualify() -> Result<(), &'static str> {
    debug_assert_eq!(CONTENT_RESERVE, 19116);
    use std::mem::{align_of, size_of};
    let word = size_of::<usize>();
    let string = size_of::<String>();
    let value = size_of::<Value>();
    let map = size_of::<Map<String, Value>>();
    if word > 8
        || !crate::content_format::temporary_storage_supported()
        || string > 24
        || value > 128
        || map > 256
        || align_of::<Value>() > 8
        || align_of::<Map<String, Value>>() > 8
        || align_of::<crate::Report<'_>>() > 8
        || size_of::<State>() > 16
        || size_of::<Option<crate::units::State>>() > 128
        || size_of::<Option<crate::content_format::State>>() + align_of::<crate::Report<'_>>() - 1
            + size_of::<Field<RegistryKnowledgeView>>()
            > 64
    {
        return Err("registry report backend exceeds qualified fixed envelope");
    }
    let bucket = word + string + value;
    let indices = (8 + 16) * word + (8 + 16) + 2 * 16;
    let growth = (7 + 14) * bucket + indices + map + bucket;
    let btree = 16 + 11 * (string + value) + map + string + value;
    if growth > 4096 || btree > 4096 {
        return Err("registry map growth exceeds qualified fixed envelope");
    }
    // At the ninth interpretation, growth keeps the old8 and new16 backing
    // briefly live, and the tenth and eleventh fit the new16. The registry
    // record array has capacity8. Pay all three, plus every interpretation's
    // at-most40-byte text, from the existing combined array share: registry8
    // and interpretations4 at 128 bytes each.
    // This is a compiled-backend bound, not an unused-fourth-slot argument.
    let combined_arrays =
        (8 + 16 + 8) * value + MAX_FULL_INTERPRETATIONS * MAX_INTERPRETATION_BYTES;
    if combined_arrays > (8 + 4) * 128
        || INTERPRETATIONS
            .iter()
            .any(|(_, id)| id.len() > MAX_INTERPRETATION_BYTES)
    {
        return Err("combined Full interpretation growth exceeds qualified fixed envelope");
    }
    for index in 0..RECORDS {
        let Some(facts) = facts(index) else { continue };
        qualify_facts(facts)?;
    }
    Ok(())
}

fn qualify_facts(facts: &Facts) -> Result<(), &'static str> {
    for (text, cap) in [
        (facts.registry, 32),
        (facts.snapshot, 64),
        (facts.scope, 32),
        (facts.extraction_policy, 64),
        (facts.digest, 64),
        (facts.raw_digest, 64),
        (facts.source_updated, 10),
        (facts.retrieved_at, 20),
    ] {
        if text.len() > cap
            || !text
                .bytes()
                .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\')
        {
            return Err("registry literals exceed qualified fixed envelope");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("../tests/fixtures/registry/content-producer-facts.rs");
    #[test]
    fn fixed_layout_growth_and_two_generations_are_qualified_before_scanning() {
        qualify().unwrap();
        let shared = 5usize.checked_mul(4096).unwrap() + 1536 + 2 * 4 * 128 + 2 * 128 + 2 * 24;
        let unit = 4 * 4096 + 2560 + 4 * 128 + 24;
        let content = 3 * 4096 + 2560;
        // Full alone adds the recorded extension record: one map and its text.
        let recorded = 4096 + 512;
        assert_eq!(
            (shared, unit, content, recorded),
            (23344, 19480, 14848, 4608)
        );
        assert_eq!(2 * (shared + unit) + 144, 4 * UNIT_RESERVE);
        assert_eq!(2 * (shared + content) + 80, 4 * CONTENT_RESERVE);
        assert_eq!(
            2 * (shared + 512 + unit + content + recorded) + 208,
            4 * FULL_RESERVE
        );
        // Exercise the actual six-record, ten-field maximum graph, independently
        // of content semantics. Clone once to model the conservative fallback.
        let mut records = Vec::with_capacity(8);
        for _ in 0..RECORDS {
            let mut record = Map::new();
            for (key, width) in [
                ("registry", 32),
                ("snapshot", 64),
                ("state", 11),
                ("scope", 32),
                ("extraction_policy", 64),
                ("digest", 64),
                ("raw_digest", 64),
                ("source_updated", 10),
                ("retrieved_at", 20),
            ] {
                record.insert(key.to_owned(), Value::String("x".repeat(width)));
                assert!(record.len() <= 10);
            }
            record.insert("entries_count".to_owned(), Value::from(u32::MAX));
            records.push(Value::Object(record));
        }
        assert_eq!(records.capacity(), 8);
        let first = Value::Array(records);
        let second = first.clone();
        assert!(second.as_array().unwrap().capacity() <= 8);
        let record_wire = serde_json::to_string(&first).unwrap().len();
        assert!(record_wire <= RECORDS * 530);
        assert!(record_wire + 1536 + 2560 <= 7680);
        println!("synthetic_all_caps_six_record_bytes={record_wire}; full_wire_ceiling=7680");
        drop(second);
        drop(first);
        let mut actual = Vec::with_capacity(8);
        for facts in crate::units::FACTS
            .iter()
            .chain(CONTENT_PRODUCER_FACTS.iter())
            .chain(crate::recorded_extension::FACTS.iter())
        {
            qualify_facts(facts).unwrap();
            actual.push(
                RegistryKnowledgeView {
                    facts,
                    available: true,
                }
                .wire(),
            );
        }
        let actual = Value::Array(actual);
        assert_eq!(actual.as_array().unwrap().capacity(), 8);
        assert_eq!(actual.as_array().unwrap().len(), RECORDS);
        let actual_second = actual.clone();
        assert!(actual_second.as_array().unwrap().capacity() <= 8);
        for record in actual.as_array().unwrap() {
            let width = serde_json::to_string(record).unwrap().len();
            assert!(width <= 530);
            assert_eq!(record.as_object().unwrap().len(), 10);
            println!(
                "actual fixed record {}: bytes={width}, fields=10",
                record["registry"]
            );
        }
        println!(
            "finite_literal_six_record_bytes={}",
            serde_json::to_string(&actual).unwrap().len()
        );
        drop(actual_second);
        let captured: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/registry/content-producer-records.json"
        ))
        .unwrap();
        for (produced, expected) in actual
            .as_array()
            .unwrap()
            .iter()
            .skip(2)
            .zip(captured.as_array().unwrap())
        {
            assert_eq!(produced.as_object().unwrap().len(), 10);
            assert_eq!(expected.as_object().unwrap().len(), 10);
            for (key, value) in expected.as_object().unwrap() {
                assert_eq!(&produced[key], value);
            }
        }
        drop(captured);
        drop(actual);
        let arbitrary: Value = serde_json::from_str("123456789012345678901234567890").unwrap();
        let encoded_number = arbitrary.to_string();
        let mut order = Map::new();
        order.insert("z".to_owned(), Value::Null);
        order.insert("a".to_owned(), Value::Null);
        println!(
            "word={} String={} Value={} Map={} alignment={}/{} map_growth={} btree={} owners={}/{} indices={} full_DTO={} exact_number={} order={}",
            std::mem::size_of::<usize>(),
            std::mem::size_of::<String>(),
            std::mem::size_of::<Value>(),
            std::mem::size_of::<Map<String, Value>>(),
            std::mem::align_of::<Value>(),
            std::mem::align_of::<Map<String, Value>>(),
            (7 + 14) * (8 + 24 + std::mem::size_of::<Value>())
                + 248
                + std::mem::size_of::<Map<String, Value>>()
                + 8
                + 24
                + std::mem::size_of::<Value>(),
            16 + 11 * (24 + std::mem::size_of::<Value>())
                + std::mem::size_of::<Map<String, Value>>()
                + 24
                + std::mem::size_of::<Value>(),
            std::mem::size_of::<Option<crate::units::State>>(),
            std::mem::size_of::<State>(),
            248,
            2 * (shared + 512 + unit + content) + 208,
            encoded_number == "123456789012345678901234567890",
            order.keys().cloned().collect::<Vec<_>>().join(",")
        );
        println!(
            "integrated Rule={} Finding={} Structure={} ProofScope={} ProofWitness={} ProofEvaluation={} Report={} CheckRequest={}; finding_capacity_charge=size_of_Finding; advisory_DTO=optional_precharged",
            std::mem::size_of::<crate::structure::Rule>(),
            std::mem::size_of::<crate::structure::Finding<'_>>(),
            std::mem::size_of::<crate::structure::Structure<'_>>(),
            std::mem::size_of::<crate::proof::Scope>(),
            std::mem::size_of::<crate::proof::Witness<'_>>(),
            std::mem::size_of::<crate::proof::Evaluation<'_>>(),
            std::mem::size_of::<crate::Report<'_>>(),
            std::mem::size_of::<crate::CheckRequest>(),
        );
    }
}

//! Sample-value checking of values Esdieff generates itself, such as a candidate Breaking
//! witness or a Generated value, against a selected Effective data description from any
//! effective model. Admission follows ADR 0016: Selected samples with explicit admission and
//! coverage, except that running out of the allowance leaves a check undetermined, following
//! ADR 0039: Client-side revision comparison. A check spends the allowance it is lent without
//! refill: revision comparison lends one operation's allowance to all its checks, following
//! ADR 0013: Shared workflow and report guarantees, while generation lends each check the
//! allowance of one Sample-value checking request, following ADR 0041: Generated values settled
//! by Sample-value checking. The public Sample workflow keeps its supplied Sample source.
use super::{Admission, description_root, evaluate_branches};
use crate::Catalogue;
use crate::allowance::OperationBudget;
use crate::equality::EqualityWork;
use crate::finding::Finding;
use crate::value::{BorrowedValue, ValueContext};
use crate::{
    EffectiveDataDescription, ExactNumber, JsonValue, NumberWork, SampleValueView, Source,
};
use std::cmp::Ordering;

/// A value Esdieff built itself, such as a candidate Breaking witness or a Generated value,
/// held as the typed Sample projection that Sample-value checking reads if it were supplied.
pub(crate) struct CheckedValue(Source);
impl CheckedValue {
    pub(crate) fn new(value: JsonValue) -> Self {
        Self(Source::generated_sample(value))
    }
    pub(crate) fn to_json(&self) -> &str {
        self.0.text().expect("generated JSON text")
    }
}

/// The work, allocation, numeric and equality counters that generated-value checks spend.
/// Revision comparison lends one to all its checks, beside its own composition and
/// comparisons, while generation builds each description's values on one and lends each check
/// a fresh one. No check replenishes any counter.
pub(crate) struct ValueAllowance {
    pub(crate) budget: OperationBudget,
    numbers: usize,
    equality_pairs: usize,
}
impl ValueAllowance {
    /// The counters Sample-value checking would lend for one request against `catalogue`.
    pub(crate) fn new(catalogue: &Catalogue) -> Self {
        Self::lend(
            OperationBudget::new(&catalogue.limits),
            catalogue.numeric_remaining,
            catalogue.limits.equality_pairs,
        )
    }
    /// Counters with these remainders, lent to whatever spends them.
    pub(crate) fn lend(budget: OperationBudget, numbers: usize, equality_pairs: usize) -> Self {
        Self {
            budget,
            numbers,
            equality_pairs,
        }
    }
    /// Compare exact numbers on the numeric allowance that the checks share.
    pub(crate) fn compare(
        &mut self,
        a: ExactNumber<'_>,
        b: ExactNumber<'_>,
    ) -> Result<Ordering, &'static str> {
        let mut numbers = NumberWork::from_remaining(self.numbers);
        let order = a.compare(b, &mut numbers);
        self.numbers = numbers.remaining();
        order
    }
    /// Whether `value` is an exact integer multiple of `step`, on the same numeric allowance.
    pub(crate) fn multiple(
        &mut self,
        value: ExactNumber<'_>,
        step: ExactNumber<'_>,
    ) -> Result<bool, &'static str> {
        let mut numbers = NumberWork::from_remaining(self.numbers);
        let multiple =
            crate::numeric::multiple(value, step, &mut numbers, &mut self.budget.allocation);
        self.numbers = numbers.remaining();
        multiple
    }
}

/// The value interpretations that Sample-value checking relied on, as the bits it records:
/// the selected civil-time representation, the `sdfType` conventions and the UUID
/// representations.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ValueInterpretations {
    pub(crate) temporal: u8,
    pub(crate) abstract_use: u8,
    pub(crate) format: u8,
}
impl ValueInterpretations {
    /// Their identifiers, in the order a report's `baseline.interpretations` lists them.
    pub(crate) fn ids(self) -> impl Iterator<Item = &'static str> {
        [
            (
                self.temporal & crate::civil::CONVENTION_USED,
                "ADR0018-selected-representation",
            ),
            (self.abstract_use & 1, "byte-string-json/1"),
            (self.abstract_use & 2, "unix-time-posix-coordinate/1"),
            (
                self.format & crate::formats::UUID_TEXT_USED,
                "uuid-bare-json/1",
            ),
            (
                self.format & crate::formats::UUID_VALUE_USED,
                "uuid-opaque-value/1",
            ),
        ]
        .into_iter()
        .filter_map(|(used, id)| (used != 0).then_some(id))
    }
}
impl std::ops::BitOrAssign for ValueInterpretations {
    fn bitor_assign(&mut self, other: Self) {
        self.temporal |= other.temporal;
        self.abstract_use |= other.abstract_use;
        self.format |= other.format;
    }
}

/// The admission of one checked value, such as a Generated value or the value of a Trace
/// entry, and the sample findings that establish it.
pub(crate) struct ValueAdmission {
    pub(crate) admission: Admission,
    pub(crate) findings: Vec<Finding>,
    /// The allowance ran out during this check, which alone leaves it undetermined.
    pub(crate) exhausted: bool,
    /// The value interpretations the check relied on.
    pub(crate) interpretations: ValueInterpretations,
}

/// Check a value Esdieff built against `description`, as `check_value` does.
pub(crate) fn check_generated(
    description: EffectiveDataDescription<'_>,
    value: &CheckedValue,
    allowance: &mut ValueAllowance,
) -> Result<ValueAdmission, &'static str> {
    check_value(description, value.0.sample_value().require()?, allowance)
}

/// Check `sample` against `description` on `allowance`. The sample may be any value within its
/// source, such as the value of a Trace entry, and the description may belong to any effective
/// model but must be a root that Sample-value checking can select, or the check is refused
/// before it spends anything.
pub(crate) fn check_value(
    description: EffectiveDataDescription<'_>,
    sample: SampleValueView<'_>,
    allowance: &mut ValueAllowance,
) -> Result<ValueAdmission, &'static str> {
    let json = description.json();
    description_root(json)?;
    let mut numbers = NumberWork::from_remaining(allowance.numbers);
    let mut equality = EqualityWork::new(allowance.equality_pairs);
    let mut evaluation = evaluate_branches(
        &ValueContext {
            description: json,
            root: BorrowedValue::Sample(sample),
        },
        &mut allowance.budget,
        &mut numbers,
        &mut equality,
    );
    allowance.numbers = numbers.remaining();
    allowance.equality_pairs = equality.remaining();
    // A refused charge empties its counter, so an exactly spent counter counts as exhausted.
    let exhausted = numbers.exhausted()
        || allowance.budget.work == 0
        || allowance.budget.allocation == 0
        || allowance.equality_pairs == 0;
    if exhausted {
        // Unfinished work never yields a definite admission, even beside an established
        // rejection, so a candidate witness stays unsettled.
        evaluation.admission = Admission::Undetermined;
        evaluation
            .findings
            .retain(|finding| finding.basis.incomplete());
    }
    let interpretations = ValueInterpretations {
        temporal: evaluation.temporal_use,
        abstract_use: equality.abstract_use,
        format: if equality.format_established {
            equality.format_use
        } else {
            0
        },
    };
    Ok(ValueAdmission {
        admission: evaluation.admission,
        findings: evaluation.findings,
        exhausted,
        interpretations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Admission, Catalogue, Class, EffectiveDefinition, EffectiveModel, EffectiveRequest,
        JsonBuilder, JsonValue, SampleKind, SampleRequest, SourceInput,
    };

    const THERMOSTAT: &[u8] = br#"{"sdfObject":{"thermostat":{"sdfProperty":{
        "setpoint":{"type":"number","minimum":5,"maximum":1e1}
    }}}}"#;

    fn number(lexeme: &str) -> JsonValue {
        JsonBuilder::new().number(lexeme).unwrap()
    }
    fn text(value: &str) -> JsonValue {
        JsonBuilder::new().text(value).unwrap()
    }
    fn map(entries: Vec<(&str, JsonValue)>) -> JsonValue {
        JsonBuilder::new().map(entries).unwrap()
    }
    fn array(lexemes: &[&str]) -> JsonValue {
        JsonBuilder::new()
            .array(lexemes.iter().map(|lexeme| number(lexeme)).collect())
            .unwrap()
    }

    #[derive(Clone, Copy)]
    enum Root {
        Data,
        Input,
        Output,
    }
    /// One value, the selected description it is checked against, and its known admission.
    struct Case {
        path: &'static [(Class, &'static str)],
        root: Root,
        value: fn() -> JsonValue,
        expected: Admission,
    }
    fn select<'a>(model: &'a EffectiveModel<'a>, case: &Case) -> EffectiveDataDescription<'a> {
        let found = definition(model, case.path);
        match case.root {
            Root::Data => found.data_description(),
            Root::Input => found.input(),
            Root::Output => found.output(),
        }
        .present()
        .expect("selected description")
    }
    /// Check each case's value as a generated value and as a supplied typed Sample against
    /// the same description of `document` within `shared` documents.
    fn agree(document: &[u8], shared: &[(&str, &[u8])], cases: &[Case]) {
        for case in cases {
            let mut inputs = vec![SourceInput::supplied("model", document)];
            inputs.extend(
                shared
                    .iter()
                    .map(|(key, bytes)| SourceInput::supplied(key, bytes)),
            );
            inputs.push(SourceInput::typed("sample", (case.value)()));
            let catalogue = Catalogue::take_inputs(inputs).unwrap();
            let model = whole_document(&catalogue);
            let description = select(&model, case);
            let report = catalogue
                .check_sample(
                    SampleRequest::from_description(description)
                        .unwrap()
                        .sample(catalogue.source_id("sample").unwrap()),
                )
                .unwrap();
            let supplied = report.sample_result().unwrap().admission();
            let mut allowance = ValueAllowance::new(&catalogue);
            let value = CheckedValue::new((case.value)());
            let generated = check_generated(description, &value, &mut allowance)
                .unwrap()
                .admission;
            let shown = (case.value)().to_json().to_owned();
            assert_eq!(generated, supplied, "{:?} {shown}", case.path);
            assert_eq!(generated, case.expected, "{:?} {shown}", case.path);
        }
    }

    /// Ordinary Sample-value checking of `value` supplied as a typed Sample source.
    fn supplied(model: &[u8], kind: SampleKind, pointer: &str, value: JsonValue) -> Admission {
        let catalogue = Catalogue::take_inputs(vec![
            SourceInput::supplied("model", model),
            SourceInput::typed("sample", value),
        ])
        .unwrap();
        let request = SampleRequest::new(catalogue.source_id("model").unwrap(), kind, pointer)
            .sample(catalogue.source_id("sample").unwrap());
        let report = catalogue.check_sample(request).unwrap();
        report.sample_result().unwrap().admission()
    }

    fn whole_document(catalogue: &Catalogue) -> EffectiveModel<'_> {
        catalogue
            .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
            .unwrap()
    }

    /// The named definition reached through each `(class, name)` step of `path`.
    fn definition<'a>(
        model: &'a EffectiveModel<'a>,
        path: &[(Class, &str)],
    ) -> EffectiveDefinition<'a> {
        let (class, name) = path[0];
        let mut found = match model.definition(class, name) {
            crate::Lookup::Found(definition) => definition,
            _ => panic!("definition {name}"),
        };
        for (class, name) in &path[1..] {
            found = found
                .children(*class)
                .present()
                .and_then(|mut rows| rows.find(|(row, _)| row == name))
                .and_then(|(_, row)| row.present())
                .expect("nested definition");
        }
        found
    }

    /// Compose the whole `model` document on the caller's allowance, as an operation would.
    fn compose<'a>(catalogue: &'a Catalogue, budget: &mut OperationBudget) -> EffectiveModel<'a> {
        let id = catalogue.source_id("model").unwrap();
        catalogue
            .compose_gated(EffectiveRequest::new(id), budget, false)
            .unwrap()
    }

    #[test]
    fn a_generated_value_has_the_admission_of_the_same_supplied_sample() {
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", THERMOSTAT)]).unwrap();
        let model = whole_document(&catalogue);
        let setpoint = definition(
            &model,
            &[(Class::Object, "thermostat"), (Class::Property, "setpoint")],
        )
        .data_description()
        .present()
        .unwrap();
        let mut allowance = ValueAllowance::new(&catalogue);
        for (lexeme, expected) in [
            ("10", Admission::Admitted),
            ("1e1", Admission::Admitted),
            ("5", Admission::Admitted),
            ("10.5", Admission::Rejected),
            ("4.999", Admission::Rejected),
        ] {
            let value = CheckedValue::new(number(lexeme));
            let checked = check_generated(setpoint, &value, &mut allowance).unwrap();
            assert_eq!(checked.admission, expected, "generated {lexeme}");
            assert_eq!(
                supplied(
                    THERMOSTAT,
                    SampleKind::Property,
                    "/sdfObject/thermostat/sdfProperty/setpoint",
                    number(lexeme),
                ),
                expected,
                "supplied {lexeme}"
            );
        }
    }
    #[test]
    fn one_allowance_checks_a_value_against_two_model_revisions() {
        let earlier = Catalogue::from_inputs(&[SourceInput::supplied(
            "model",
            br#"{"sdfData":{"temperature":{"type":"number","maximum":1e1}}}"#,
        )])
        .unwrap();
        let later = Catalogue::from_inputs(&[SourceInput::supplied(
            "model",
            br#"{"sdfData":{"temperature":{"type":"number","maximum":9.5}}}"#,
        )])
        .unwrap();
        let mut allowance = ValueAllowance::new(&earlier);
        let before = allowance.budget.work;
        let earlier_model = compose(&earlier, &mut allowance.budget);
        let later_model = compose(&later, &mut allowance.budget);
        let composed = allowance.budget.work;
        assert!(composed < before);
        let temperature = |model| {
            definition(model, &[(Class::Data, "temperature")])
                .data_description()
                .present()
                .unwrap()
        };
        let value = CheckedValue::new(number("10"));
        let old = check_generated(temperature(&earlier_model), &value, &mut allowance).unwrap();
        let checked_once = allowance.budget.work;
        let new = check_generated(temperature(&later_model), &value, &mut allowance).unwrap();
        assert_eq!(
            (old.admission, new.admission),
            (Admission::Admitted, Admission::Rejected)
        );
        assert!(allowance.budget.work < checked_once && checked_once < composed);
        assert!(old.findings.is_empty());
        assert_eq!(
            new.findings
                .iter()
                .map(|finding| (finding.code, finding.constraint))
                .collect::<Vec<_>>(),
            [("sample.bound", "maximum")]
        );
    }

    const DEVICE: &[u8] = br##"{"sdfObject":{"device":{
        "sdfProperty":{
            "level":{"type":"integer","minimum":0,"exclusiveMaximum":1800.001},
            "mode":{"type":"string","enum":["eco","comfort"]},
            "name":{"type":"string","nullable":false,"maxLength":4},
            "pinned":{"type":"number","const":1e1,"maximum":20},
            "broken":{"type":"string","pattern":"["},
            "vendor":{"type":"acme:thing"},
            "record":{"type":"object","properties":{
                "known":{"type":"number"},
                "missing":{"sdfRef":"#/sdfData/absent"}
            }}
        },
        "sdfAction":{"configure":{
            "sdfInputData":{"type":"object","required":["target"],
                "properties":{"target":{"type":"number","maximum":1800}}},
            "sdfOutputData":{"type":"boolean"}
        }},
        "sdfEvent":{"alarm":{"sdfOutputData":{"type":"string","format":"date-time"}}}
    }}}"##;

    #[test]
    fn generated_admission_agrees_with_supplied_samples() {
        use Admission::{Admitted, Rejected, Undetermined};
        const LEVEL: &[(Class, &str)] = &[(Class::Object, "device"), (Class::Property, "level")];
        const MODE: &[(Class, &str)] = &[(Class::Object, "device"), (Class::Property, "mode")];
        const NAME: &[(Class, &str)] = &[(Class::Object, "device"), (Class::Property, "name")];
        const PINNED: &[(Class, &str)] = &[(Class::Object, "device"), (Class::Property, "pinned")];
        const BROKEN: &[(Class, &str)] = &[(Class::Object, "device"), (Class::Property, "broken")];
        const VENDOR: &[(Class, &str)] = &[(Class::Object, "device"), (Class::Property, "vendor")];
        const RECORD: &[(Class, &str)] = &[(Class::Object, "device"), (Class::Property, "record")];
        const CONFIGURE: &[(Class, &str)] =
            &[(Class::Object, "device"), (Class::Action, "configure")];
        const ALARM: &[(Class, &str)] = &[(Class::Object, "device"), (Class::Event, "alarm")];
        let case = |path, root, value, expected| Case {
            path,
            root,
            value,
            expected,
        };
        agree(
            DEVICE,
            &[],
            &[
                case(LEVEL, Root::Data, || number("1800"), Admitted),
                case(LEVEL, Root::Data, || number("1e1"), Admitted),
                case(LEVEL, Root::Data, || number("1800.001"), Rejected),
                case(LEVEL, Root::Data, || number("0.5"), Rejected),
                case(LEVEL, Root::Data, || text("1"), Rejected),
                case(MODE, Root::Data, || text("eco"), Admitted),
                case(MODE, Root::Data, || text("away"), Rejected),
                case(NAME, Root::Data, || text("abcd"), Admitted),
                case(NAME, Root::Data, || text("abcde"), Rejected),
                case(
                    NAME,
                    Root::Data,
                    || JsonBuilder::new().null().unwrap(),
                    Rejected,
                ),
                case(PINNED, Root::Data, || number("10.0"), Admitted),
                case(PINNED, Root::Data, || number("11"), Rejected),
                case(BROKEN, Root::Data, || text("a"), Undetermined),
                case(VENDOR, Root::Data, || text("a"), Undetermined),
                case(
                    RECORD,
                    Root::Data,
                    || map(vec![("known", number("1"))]),
                    Admitted,
                ),
                case(
                    RECORD,
                    Root::Data,
                    || map(vec![("known", text("1"))]),
                    Rejected,
                ),
                case(
                    RECORD,
                    Root::Data,
                    || map(vec![("missing", number("1"))]),
                    Undetermined,
                ),
                case(
                    CONFIGURE,
                    Root::Input,
                    || map(vec![("target", number("1800"))]),
                    Admitted,
                ),
                case(
                    CONFIGURE,
                    Root::Input,
                    || map(vec![("target", number("1800.001"))]),
                    Rejected,
                ),
                case(CONFIGURE, Root::Input, || map(Vec::new()), Rejected),
                case(
                    CONFIGURE,
                    Root::Output,
                    || JsonBuilder::new().boolean(true).unwrap(),
                    Admitted,
                ),
                case(CONFIGURE, Root::Output, || text("true"), Rejected),
                case(
                    ALARM,
                    Root::Output,
                    || text("2026-10-16T08:00:00Z"),
                    Admitted,
                ),
                case(
                    ALARM,
                    Root::Output,
                    || text("2026-02-30T08:00:00Z"),
                    Rejected,
                ),
            ],
        );
    }

    const SHARED: &[u8] = br##"{
        "namespace":{"s":"https://example.com/shared"},"defaultNamespace":"s",
        "sdfData":{
            "Temperature":{"type":"number","minimum":-40,"maximum":125,"unit":"Cel"},
            "Readings":{"type":"array","maxItems":2,"uniqueItems":true,
                "items":{"type":"number","maximum":1e1}},
            "Band":{"sdfChoice":{
                "low":{"type":"number","maximum":0},
                "high":{"type":"number","minimum":100}
            }}
        }
    }"##;
    const SENSOR: &[u8] = br##"{
        "namespace":{"s":"https://example.com/shared"},
        "sdfObject":{"sensor":{"sdfProperty":{
            "temperature":{"sdfRef":"s:#/sdfData/Temperature","maximum":85},
            "readings":{"sdfRef":"s:#/sdfData/Readings"},
            "band":{"sdfRef":"s:#/sdfData/Band"},
            "note":{"type":"number","acme:hint":"x"}
        }}}
    }"##;

    #[test]
    fn generated_admission_agrees_through_shared_definitions() {
        use Admission::{Admitted, Rejected};
        const TEMPERATURE: &[(Class, &str)] =
            &[(Class::Object, "sensor"), (Class::Property, "temperature")];
        const READINGS: &[(Class, &str)] =
            &[(Class::Object, "sensor"), (Class::Property, "readings")];
        const BAND: &[(Class, &str)] = &[(Class::Object, "sensor"), (Class::Property, "band")];
        const NOTE: &[(Class, &str)] = &[(Class::Object, "sensor"), (Class::Property, "note")];
        let case = |path, value, expected| Case {
            path,
            root: Root::Data,
            value,
            expected,
        };
        agree(
            SENSOR,
            &[("shared", SHARED)],
            &[
                case(TEMPERATURE, || number("85"), Admitted),
                case(TEMPERATURE, || number("85.5"), Rejected),
                case(TEMPERATURE, || number("-4e1"), Admitted),
                case(TEMPERATURE, || number("-40.001"), Rejected),
                case(READINGS, || array(&["1", "1e1"]), Admitted),
                case(READINGS, || array(&["1e1", "10"]), Rejected),
                case(READINGS, || array(&["1", "2", "3"]), Rejected),
                case(READINGS, || array(&["10.5"]), Rejected),
                case(BAND, || number("-1"), Admitted),
                case(BAND, || number("1e2"), Admitted),
                case(BAND, || number("50"), Rejected),
                case(NOTE, || number("1"), Admitted),
            ],
        );
    }

    #[test]
    fn exhausting_the_allowance_during_a_check_leaves_admission_undetermined() {
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
            "model",
            br#"{"sdfProperty":{"setting":{"type":"object","properties":{
                "label":{"type":"string","maxLength":4},
                "level":{"type":"number","const":1e1}
            }}}}"#,
        )])
        .unwrap();
        let model = whole_document(&catalogue);
        let setting = definition(&model, &[(Class::Property, "setting")])
            .data_description()
            .present()
            .unwrap();
        let admitted = || map(vec![("label", text("eco")), ("level", number("10"))]);
        // The label's type rejects without numeric work while the level still needs it.
        let rejected = || map(vec![("label", number("1")), ("level", number("10"))]);
        type Counter = fn(&mut ValueAllowance) -> &mut usize;
        let counters: [(&str, Counter); 4] = [
            ("work", |allowance| &mut allowance.budget.work),
            ("allocation", |allowance| &mut allowance.budget.allocation),
            ("numbers", |allowance| &mut allowance.numbers),
            ("equality pairs", |allowance| &mut allowance.equality_pairs),
        ];
        for (value, complete) in [
            (admitted as fn() -> JsonValue, Admission::Admitted),
            (rejected, Admission::Rejected),
        ] {
            let value = CheckedValue::new(value());
            for (name, counter) in counters {
                let mut full = ValueAllowance::new(&catalogue);
                let before = *counter(&mut full);
                let checked = check_generated(setting, &value, &mut full).unwrap();
                assert_eq!((checked.admission, checked.exhausted), (complete, false));
                let needed = before - *counter(&mut full);
                assert!(needed > 0, "{name} is charged");
                for short in 0..needed {
                    let mut allowance = ValueAllowance::new(&catalogue);
                    *counter(&mut allowance) = short;
                    let checked = check_generated(setting, &value, &mut allowance).unwrap();
                    assert_eq!(
                        (checked.admission, checked.exhausted),
                        (Admission::Undetermined, true),
                        "{complete:?} with {short} of {needed} {name}"
                    );
                    assert!(
                        checked
                            .findings
                            .iter()
                            .all(|finding| finding.basis.incomplete())
                    );
                }
            }
        }
    }

    #[test]
    fn a_description_sample_checking_cannot_select_is_refused() {
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", DEVICE)]).unwrap();
        let model = whole_document(&catalogue);
        let target = definition(
            &model,
            &[(Class::Object, "device"), (Class::Action, "configure")],
        )
        .input()
        .present()
        .and_then(|input| input.properties().present())
        .and_then(|mut entries| entries.find(|(name, _)| *name == "target"))
        .and_then(|(_, entry)| entry.present())
        .unwrap();
        let mut allowance = ValueAllowance::new(&catalogue);
        let work = allowance.budget.work;
        let value = CheckedValue::new(number("1"));
        assert!(check_generated(target, &value, &mut allowance).is_err());
        assert_eq!(allowance.budget.work, work);
    }
}

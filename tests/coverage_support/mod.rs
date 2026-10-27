//! Shared fixture observations through public domain operations.
#![allow(dead_code)]
use esdieff::{
    AssessmentStatus, BaseType, Catalogue, CheckRequest, Class, DataType, EffectiveDefinition,
    EffectiveRequest, Field, InspectRequest, Lookup, RequiredReferenceForm, SourceInput, View,
};

pub const DEVICE: &[u8] = include_bytes!("../fixtures/coverage/native-1.json");
pub const CONTROLLER: &[u8] = include_bytes!("../fixtures/coverage/native-2.json");
pub const LEVEL: &[u8] = include_bytes!("../fixtures/coverage/native-3.json");
pub const INITIAL_AREAS: [&str; 8] = [
    "document-envelope/1",
    "base-roles-names/1",
    "metadata-namespaces/1",
    "established-composition/1",
    "basic-parameters/1",
    "alternatives-requiredness/1",
    "extensions-policies/1",
    "advisory-four/1",
];
pub mod semantic;
pub const FULL_AREAS: [&str; 13] = [
    "full.document-syntax/1",
    "full.model-roles/1",
    "full.metadata/1",
    "full.namespace-reference/1",
    "full.composition-requiredness/1",
    "full.critical-extensions/1",
    "full.data-grammar/1",
    "full.formal-numbers/1",
    "full.scalar-meaning/1",
    "full.pattern-format/1",
    "full.compound-meaning/1",
    "full.special-type-unit/1",
    "advisory-four/1",
];

/// Validate current counts against every public member of the requested inventory.
pub fn assert_requested_counts(report: &esdieff::Report<'_>) {
    if report.assessment().is_none() {
        assert_wire_counts(&serde_json::from_str(&report.to_json()).unwrap());
        return;
    }
    let mut areas = INITIAL_AREAS.to_vec();
    let inventory = match report.assessment().unwrap() {
        esdieff::Assessment::Initial => "esdieff.initial/1",
        esdieff::Assessment::FullBaseline => {
            areas = FULL_AREAS.to_vec();
            "esdieff.full-model/1"
        }
        esdieff::Assessment::ExpandedModified => {
            areas.push("modified-date-time/1");
            "esdieff.expanded.modified/1"
        }
        esdieff::Assessment::ExpandedUnits => {
            areas.extend(["unit-spelling/1", "unit-registry-guidance/1"]);
            "esdieff.expanded.units/1"
        }
        esdieff::Assessment::ExpandedContentFormat => {
            areas.extend(["content-format-syntax/1", "content-format-registry/1"]);
            "esdieff.expanded.content-format/1"
        }
        esdieff::Assessment::ExpandedPatternSyntax => {
            areas.push("pattern-syntax/1");
            "esdieff.expanded.pattern-syntax/1"
        }
        _ => panic!("unhandled assessment inventory"),
    };
    let mut counts = [0u32; 4];
    for area in &areas {
        counts[match report.assessment_area_status(area).require().unwrap() {
            AssessmentStatus::Assessed => 0,
            AssessmentStatus::NotApplicable => 1,
            AssessmentStatus::Unassessed => 2,
            AssessmentStatus::Undetermined => 3,
            _ => panic!("unhandled assessment status"),
        }] += 1;
    }
    let coverage = report.requested_coverage();
    assert_eq!(coverage.inventory(), Some(inventory));
    assert_eq!(coverage.total(), Some(areas.len() as u32));
    assert_eq!(
        [
            coverage.assessed(),
            coverage.not_applicable(),
            coverage.unassessed(),
            coverage.undetermined()
        ],
        counts
    );
    assert_eq!(report.outcome().incomplete_checks(), counts[2] + counts[3]);
    assert_eq!(report.outcome().complete(), counts[2] + counts[3] == 0);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    for (field, expected) in ["assessed", "not_applicable", "unassessed", "undetermined"]
        .into_iter()
        .zip(counts)
    {
        assert_eq!(wire["coverage"]["requested"][field], expected);
    }
}

pub fn assert_wire_counts(wire: &serde_json::Value) {
    let requested = &wire["coverage"]["requested"];
    let count = |field| requested[field].as_u64().unwrap();
    let unfinished = count("unassessed") + count("undetermined");
    assert_eq!(
        count("assessed") + count("not_applicable") + unfinished,
        count("total")
    );
    assert_eq!(wire["outcome"]["incomplete_checks"], unfinished);
    assert_eq!(wire["outcome"]["complete"], unfinished == 0);
    let rows = wire["coverage"]["rows"].as_array().unwrap();
    if requested["count_unit"] == "inspection" {
        assert!(rows.is_empty());
        return;
    }
    assert_eq!(
        rows.len() as u64 + wire["coverage"]["rows_omitted"].as_u64().unwrap(),
        count("total")
    );
    if wire["coverage"]["rows_omitted"] == 0 {
        for field in ["assessed", "not_applicable", "unassessed", "undetermined"] {
            assert_eq!(
                rows.iter().filter(|row| row["status"] == field).count() as u64,
                count(field)
            );
        }
    }
}

/// The immutable existing inventory identifies its fine-grained membership.
pub fn requirement_binding(inventory: Option<&str>) -> Field<&'static str> {
    match inventory {
        Some("esdieff.full-model/1") => Field::Present("rfc9880.model/1"),
        Some("esdieff.initial/1" | "esdieff.expanded.modified/1" | "esdieff.expanded.units/1") => {
            Field::Absent
        }
        _ => Field::Unavailable("requested inventory binding unavailable"),
    }
}

/// Existing typed and wire consumers recover identity with all new optional sections absent.
pub fn assert_full_binding(report: &esdieff::Report<'_>) {
    let coverage = report.requested_coverage();
    assert_eq!(coverage.inventory(), Some("esdieff.full-model/1"));
    assert_eq!(coverage.total(), Some(13));
    assert_eq!(
        requirement_binding(coverage.inventory()).require().unwrap(),
        "rfc9880.model/1"
    );
    for area in FULL_AREAS {
        assert!(matches!(
            report.assessment_area_status(area),
            Field::Present(_)
        ));
    }
    assert!(matches!(
        report.assessment_area_status("outside-this-inventory/1"),
        Field::Absent
    ));
    let mut wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["request"]["assessment"], "rfc9880/base-1");
    assert_eq!(wire["baseline"]["id"], "rfc9880/base-1");
    let aggregate = semantic::aggregate_projection(&wire);
    wire["request"]
        .as_object_mut()
        .unwrap()
        .remove("requirement_inventory");
    for field in [
        "requirements",
        "requirement_rows",
        "requirement_rows_retained",
        "requirement_rows_omitted",
        "requirement_omissions",
    ] {
        wire["coverage"].as_object_mut().unwrap().remove(field);
    }
    let binding = requirement_binding(wire["request"]["inventory"].as_str())
        .require()
        .unwrap();
    assert_eq!(binding, "rfc9880.model/1");
    assert_eq!(
        wire["coverage"]["requested"]["inventory"],
        wire["request"]["inventory"]
    );
    assert_eq!(
        wire["coverage"]["requested"]["count_unit"],
        "assessment_area"
    );
    assert_eq!(wire["coverage"]["requested"]["total"], 13);
    assert_eq!(wire["outcome"]["complete"], report.outcome().complete());
    assert_eq!(semantic::aggregate_projection(&wire), aggregate);
    // Unknown historical membership cannot become Absent just because a guessed ID is unfamiliar.
    assert!(matches!(requirement_binding(None), Field::Unavailable(_)));
}

pub fn native_catalogue(reverse: bool) -> Catalogue {
    let mut inputs = [
        SourceInput::supplied("device", DEVICE),
        SourceInput::supplied("controller", CONTROLLER),
        SourceInput::supplied("level", LEVEL),
    ];
    if reverse {
        inputs.reverse();
    }
    Catalogue::from_inputs(&inputs).expect("native fixture catalogue")
}

fn child<'a>(
    definition: EffectiveDefinition<'a>,
    class: Class,
    name: &str,
) -> EffectiveDefinition<'a> {
    definition
        .children(class)
        .require()
        .expect("class group")
        .find(|(candidate, _)| *candidate == name)
        .expect("named declaration")
        .1
        .require()
        .expect("declaration")
}

pub fn observe_native_domain(catalogue: &Catalogue) {
    let device_id = catalogue.source_id("device").expect("device identity");
    let model = catalogue
        .effective(EffectiveRequest::new(device_id))
        .expect("effective device");
    let Lookup::Found(device) = model.definition(Class::Thing, "Device") else {
        panic!("Device")
    };
    let ambient = child(device, Class::Object, "ambient");
    assert!(matches!(ambient.description(), Field::Absent));
    let temperature = child(ambient, Class::Property, "temperature");
    let data = temperature
        .data_description()
        .require()
        .expect("temperature description");
    assert_eq!(
        data.data_type().require().unwrap(),
        DataType::Known(BaseType::Number)
    );
    assert_eq!(data.unit().require().unwrap(), "Cel");
    assert_eq!(data.minimum().require().unwrap().lexeme(), "-50");
    assert_eq!(data.maximum().require().unwrap().lexeme(), "100");
    assert!(!temperature.writable().require().unwrap());
    assert!(!data.nullable().require().unwrap());
    let changed = child(ambient, Class::Event, "changed");
    assert_eq!(
        changed
            .output()
            .require()
            .unwrap()
            .maximum()
            .require()
            .unwrap()
            .lexeme(),
        "150"
    );
    let reset = child(ambient, Class::Action, "reset");
    assert!(matches!(reset.input(), Field::Absent));
    let source = catalogue.source(device_id).unwrap();
    assert_eq!(source.raw_bytes(), DEVICE);
    let Lookup::Found(sensor) = source.definition(Class::Object, "Sensor") else {
        panic!("Sensor")
    };
    assert_eq!(
        sensor.description().require().unwrap(),
        "Reusable temperature sensing"
    );
    let Lookup::Found(temperature) = source.definition(Class::Data, "Temperature") else {
        panic!("Temperature")
    };
    assert_eq!(
        temperature
            .data_description()
            .require()
            .unwrap()
            .maximum()
            .require()
            .unwrap()
            .lexeme(),
        "150"
    );

    let required = catalogue
        .inspect(
            InspectRequest::new(device_id, "/sdfThing/Device/sdfObject/ambient")
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    assert!(
        required
            .required_declarations()
            .require()
            .unwrap()
            .complete()
    );
    assert!(
        required
            .required_declarations()
            .require()
            .unwrap()
            .references_known()
            > 0
    );
    let mut temperature_required = false;
    for owner in required.required_declarations().require().unwrap().owners() {
        if let Field::Present(references) = owner.references() {
            for reference in references {
                if matches!(
                    reference.form(),
                    Field::Present(RequiredReferenceForm::DirectName("temperature"))
                ) {
                    assert!(reference.complete());
                    let Lookup::Found(mut targets) = reference.targets() else {
                        panic!("required targets")
                    };
                    let target = targets.next().unwrap();
                    assert_eq!(target.class(), Class::Property);
                    assert_eq!(target.given_name(), "temperature");
                    assert!(targets.next().is_none());
                    temperature_required = true;
                }
            }
        }
    }
    assert!(temperature_required);

    let controller_id = catalogue.source_id("controller").unwrap();
    let level_id = catalogue.source_id("level").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(controller_id))
        .unwrap();
    let Lookup::Found(controller) = model.definition(Class::Object, "Controller") else {
        panic!("Controller")
    };
    let level = child(controller, Class::Property, "level");
    assert!(matches!(level.description(), Field::Absent));
    let data = level.data_description().require().unwrap();
    assert_eq!(data.minimum().require().unwrap().lexeme(), "0");
    assert_eq!(data.maximum().require().unwrap().lexeme(), "80");
    assert!(!data.nullable().require().unwrap());
    let source = catalogue.source(level_id).unwrap();
    assert_eq!(source.raw_bytes(), LEVEL);
    let Lookup::Found(level) = source.definition(Class::Data, "Level") else {
        panic!("Level")
    };
    assert_eq!(level.description().require().unwrap(), "Reusable level");
    assert_eq!(
        level
            .data_description()
            .require()
            .unwrap()
            .maximum()
            .require()
            .unwrap()
            .lexeme(),
        "100"
    );
}

pub fn assert_initial_completion(catalogue: &Catalogue, selected: Option<&str>) {
    let request = selected.map_or_else(CheckRequest::new, |key| {
        CheckRequest::new().root(catalogue.source_id(key).unwrap())
    });
    let report = catalogue.check(request).unwrap();
    assert!(
        report.outcome().complete(),
        "agreed native initial work must complete"
    );
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().disposition(), esdieff::Disposition::Passed);
    let coverage = report.requested_coverage();
    assert_eq!(coverage.inventory(), Some("esdieff.initial/1"));
    assert_eq!(coverage.total(), Some(8));
    assert_eq!(coverage.unassessed(), 0);
    assert_eq!(coverage.undetermined(), 0);
    assert_eq!(coverage.assessed() + coverage.not_applicable(), 8);
    for area in INITIAL_AREAS {
        assert!(
            matches!(
                report.assessment_area_status(area),
                Field::Present(AssessmentStatus::Assessed | AssessmentStatus::NotApplicable)
            ),
            "{area}"
        );
    }
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["request"]["assessment"], "esdieff.initial/1");
    assert_eq!(wire["coverage"]["broader"]["state"], "partial");
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    assert_eq!(
        wire["coverage"]["requested"]["assessed"],
        coverage.assessed()
    );
    assert_eq!(
        wire["coverage"]["requested"]["not_applicable"],
        coverage.not_applicable()
    );
    assert_eq!(
        wire["coverage"]["requested"]["unassessed"],
        coverage.unassessed()
    );
    assert_eq!(
        wire["coverage"]["requested"]["undetermined"],
        coverage.undetermined()
    );
    for area in INITIAL_AREAS {
        let status = report.assessment_area_status(area).require().unwrap();
        let expected = match status {
            AssessmentStatus::Assessed => "assessed",
            AssessmentStatus::NotApplicable => "not_applicable",
            _ => panic!("completed initial area"),
        };
        let row = wire["coverage"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == area)
            .unwrap();
        assert_eq!(row["status"], expected);
    }
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(
        wire["request"]["scope"]["kind"],
        if selected.is_some() {
            "selected"
        } else {
            "catalogue"
        }
    );
    if let Some(key) = selected {
        assert_eq!(
            wire["request"]["scope"]["roots"],
            serde_json::json!([catalogue.source_id(key).unwrap().get()])
        );
    }
    assert!(report.to_human().contains("Broader RFC coverage: partial"));
}

/// Stable semantic signature excludes human wording and optional detail ordering.
pub fn semantic_signature(report: &esdieff::Report<'_>) -> serde_json::Value {
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let diagnostics = report.diagnostics();
    assert_eq!(
        diagnostics.omitted(),
        wire["diagnostics"]["omitted"].as_u64().unwrap() as u32
    );
    assert_eq!(
        diagnostics.truncated(),
        wire["diagnostics"]["truncated"].as_bool().unwrap()
    );
    let typed: Vec<_> = INITIAL_AREAS
        .into_iter()
        .map(|area| {
            let state = match report.assessment_area_status(area) {
                Field::Present(status) => format!("{status:?}"),
                Field::Absent => "absent".into(),
                Field::Unavailable(_) => "unavailable".into(),
                _ => "unavailable".into(),
            };
            (area, state)
        })
        .collect();
    let mut typed_conditions: Vec<_> = diagnostics
        .conditions()
        .map(|condition| {
            (
                condition.code(),
                format!("{:?}", condition.severity()),
                format!("{:?}", condition.basis().kind()),
                condition.basis().id().map(str::to_owned),
                condition.basis().citation().map(str::to_owned),
                condition.assessment_unit(),
            )
        })
        .collect();
    typed_conditions.sort();
    serde_json::json!({"wire":semantic::wire_projection(&wire), "typed_initial_states":typed,
        "typed_conditions":typed_conditions})
}

//! Literal expectations follow the authored constraints and
//! ADR 0016: Selected samples with explicit admission and coverage.
use crate::catalogue_support::{
    Admission, Case, Diagnostic, Disposition, Location, LocationKind, Plan,
};
use LocationKind::{Effective, Sample, Source};

fn location(kind: LocationKind, source: &'static str, pointer: impl Into<String>) -> Location {
    Location::new(kind, source, pointer)
}
fn rejection(
    case: Case,
    code: &'static str,
    sample: &'static str,
    sample_pointer: &'static str,
    constraint: &'static str,
) -> Case {
    case.diagnostic(Diagnostic::constraint(
        code,
        location(Sample, sample, sample_pointer),
        location(Source, "thermostat", constraint),
    ))
}
fn enum_rejection(
    case: Case,
    sample: &'static str,
    sample_pointer: &'static str,
    constraint: &'static str,
    alternatives: usize,
) -> Case {
    case.enum_rejection(
        location(Sample, sample, sample_pointer),
        location(Source, "thermostat", constraint),
        alternatives,
    )
}
fn dependency(case: Case, pointer: &'static str) -> Case {
    case.diagnostic(Diagnostic::unavailable_reference(location(
        Source,
        "thermostat",
        pointer,
    )))
}
fn faulty_model(name: &'static str, rationale: &'static str) -> Case {
    dependency(
        Case {
            disposition: Disposition::Failed,
            complete: false,
            ..Case::model(name, rationale)
        }
        .diagnostic(Diagnostic {
            code: "data.number",
            primary: location(
                Source,
                "thermostat",
                "/sdfObject/Thermostat/sdfProperty/brokenTemperature/maximum",
            ),
            related: vec![],
        }),
        "/sdfObject/Thermostat/sdfProperty/currentTemperature",
    )
}

pub fn plans() -> Vec<Plan> {
    vec![Plan { device: "thermostat", variant: "entry", file: "plan.json", exit: 1, source_keys: &["thermostat", "comfortable", "too-hot"], failed_sources: &[], cases: vec![
            Case::model("Catalogue", "Initial checks the supported supplied target description"),
            Case::model("Thermostat model", "Full checks the supported selected target description").selected("thermostat"),
            Case::sample("Comfortable target", Admission::Admitted, "21.5 is between inclusive 5 and 30"),
            rejection(Case::sample("Too hot target", Admission::Rejected, "35 exceeds the inclusive maximum 30"), "sample.bound", "too-hot", "", "/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum"),
        ] },
        Plan { device: "thermostat", variant: "minimal", file: "minimal-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["thermostat", "lower", "upper", "below", "text", "unavailable"],
        cases: vec![
            Case::model("Minimal Catalogue", "Initial checks the existing supported target description"),
            Case::model("Minimal model", "Full checks the existing supported target description").selected("thermostat"),
            Case::sample("Target lower boundary", Admission::Admitted, "5 equals the inclusive minimum 5"),
            Case::sample("Target upper boundary", Admission::Admitted, "30 equals the inclusive maximum 30"),
            rejection(Case::sample("Target below minimum", Admission::Rejected, "4.5 is less than the inclusive minimum 5"), "sample.bound", "below", "", "/sdfObject/Thermostat/sdfProperty/targetTemperature/minimum"),
            rejection(Case::sample("Target wrong type", Admission::Rejected, "A string is not the declared number type"), "sample.type", "text", "", "/sdfObject/Thermostat/sdfProperty/targetTemperature/type"),
            rejection(Case::sample("Target unavailable", Admission::Rejected, "The existing target explicitly has nullable false"), "sample.null", "unavailable", "", "/sdfObject/Thermostat/sdfProperty/targetTemperature/nullable"),
        ] },
        Plan { device: "thermostat", variant: "rich", file: "rich-plan.json", exit: 1, failed_sources: &[], cases: vec![
            Case::model("Rich Catalogue", "Initial checks authored current/target, configuration and alarm descriptions"),
            Case::model("Rich model", "Full establishes all applicable supported descriptions").selected("thermostat"),
            Case::sample("Fractional current reading", Admission::Admitted, "21.75 lies within inclusive -40 and 80, without a step constraint"),
            Case::sample("Current reading unavailable", Admission::Admitted, "The current reading explicitly permits null"),
            rejection(Case::sample("Target off half-degree step", Admission::Rejected, "21.25 is not a multiple of 0.5, although it is within the bounds"), "sample.multiple", "off-step", "", "/sdfObject/Thermostat/sdfProperty/targetTemperature/multipleOf"),
            Case::sample("Configure heating", Admission::Admitted, "22.5 is within 5 through 30 and a multiple of 0.5, heat is enumerated, optional label is absent"),
            rejection(Case::sample("Configure missing mode", Admission::Rejected, "The configuration requires mode even when targetTemperature is present"), "sample.required", "missing-mode", "", "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/required/1"),
            enum_rejection(Case::sample("Configure unknown mode", Admission::Rejected, "fan is outside the enumerated off/heat/cool modes"), "unknown-mode", "/mode", "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/properties/mode/enum", 3),
            Case::sample("Configure null mode", Admission::Rejected, "A present null meets presence but mode explicitly disallows null")
                .diagnostic(Diagnostic { code: "sample.null", primary: location(Sample, "null-mode", "/mode"), related: vec![
                    location(Effective, "thermostat", "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/properties/mode/enum/0/nullable"),
                    location(Source, "thermostat", "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/properties/mode/nullable")
                ] }),
            rejection(Case::sample("Configure wrong optional label", Admission::Rejected, "A present optional label must be text, unlike an absent label"), "sample.type", "wrong-label", "/label", "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/properties/label/type"),
            Case::sample("Overheating alarm", Admission::Admitted, "35 is within alarm bounds and overheat is an enumerated reason"),
            enum_rejection(Case::sample("Unknown alarm reason", Admission::Rejected, "fire is outside the enumerated overheat/sensor-fault reasons"), "unknown-reason", "/reason", "/sdfObject/Thermostat/sdfEvent/alarm/sdfOutputData/properties/reason/enum", 2),
        ], source_keys: &["thermostat", "reading", "unavailable", "off-step", "configure", "missing-mode", "unknown-mode", "null-mode", "wrong-label", "alarm", "unknown-reason"] },
        Plan { device: "thermostat", variant: "limited", file: "limited-plan.json", exit: 1, failed_sources: &[], cases: vec![
            faulty_model("Limited Catalogue", "A textual maximum is invalid, and the referenced calibration definition is not supplied"),
            faulty_model("Limited model", "Full finds the invalid maximum and retains the unavailable calibration dependency").selected("thermostat"),
            dependency(Case::sample("Current reading needs dependency", Admission::Undetermined, "The selected current reading is wholly imported from an unsupplied namespace"), "/sdfObject/Thermostat/sdfProperty/currentTemperature"),
            dependency(Case::sample("Configure needs calibration", Admission::Undetermined, "21.5 meets the target bound, but present calibration has no supplied description"), "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/properties/calibration"),
            dependency(rejection(Case { complete: false, ..Case::sample("Configure too hot despite missing calibration", Admission::Rejected, "35 independently exceeds target maximum 30 while present calibration remains unfinished") }, "sample.bound", "too-hot-calibration", "/targetTemperature", "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/properties/targetTemperature/maximum"), "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/properties/calibration"),
        ], source_keys: &["thermostat", "reading", "calibration", "too-hot-calibration"] }]
}

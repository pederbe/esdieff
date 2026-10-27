//! Literal expectations for unique zone identifiers and recursive configuration data.
use crate::catalogue_support::{
    Admission, Case, Diagnostic, Disposition, Location, LocationKind, Plan,
};
use LocationKind::{Effective, Sample, Source};

fn constraint(
    case: Case,
    code: &'static str,
    sample: &'static str,
    pointer: &str,
    quality: &str,
) -> Case {
    case.diagnostic(Diagnostic::constraint(
        code,
        Location::new(Sample, sample, pointer),
        Location::new(
            Source,
            "irrigation",
            format!("/sdfObject/Irrigation/{quality}"),
        ),
    ))
}

pub fn plans() -> Vec<Plan> {
    vec![Plan {
        device: "irrigation", variant: "minimal", file: "minimal-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["irrigation", "one-zone", "four-zones", "no-zones", "five-zones", "duplicate-zone", "bad-identifier"],
        cases: vec![
            Case::model("Minimal Catalogue", "Initial covers the supported bounded array of zone identifiers"),
            Case::model("Minimal model", "Full covers array counts, uniqueness and the identifier pattern").selected("irrigation"),
            Case::sample("One zone lower count boundary", Admission::Admitted, "One matching zone identifier meets minItems 1"),
            Case::sample("Four zones upper count boundary", Admission::Admitted, "Four distinct zone-1 through zone-4 identifiers meet maxItems 4 and uniqueItems"),
            constraint(Case::sample("No zones", Admission::Rejected, "An empty array violates minItems 1"), "sample.count", "no-zones", "", "sdfProperty/zoneIds/minItems"),
            constraint(Case::sample("Five zones", Admission::Rejected, "Five elements exceed maxItems 4 independently of the repeated identifier"), "sample.count", "five-zones", "", "sdfProperty/zoneIds/maxItems"),
            Case::sample("Duplicate zone identifier", Admission::Rejected, "Two equal zone-1 strings violate uniqueItems true")
                .diagnostic(Diagnostic {
                    code: "sample.unique",
                    primary: Location::new(Sample, "duplicate-zone", "/1"),
                    related: vec![
                        Location::new(Sample, "duplicate-zone", "/0"),
                        Location::new(Effective, "irrigation", "/sdfObject/Irrigation/sdfProperty/zoneIds/uniqueItems"),
                        Location::new(Source, "irrigation", "/sdfObject/Irrigation/sdfProperty/zoneIds/uniqueItems"),
                    ],
                }),
            constraint(Case::sample("Invalid zone identifier", Admission::Rejected, "garden does not match the anchored zone-[1-4] pattern"), "sample.pattern", "bad-identifier", "/0", "sdfProperty/zoneIds/items/sdfChoice/identifier/pattern"),
        ],
    }, Plan {
        device: "irrigation", variant: "rich", file: "rich-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["irrigation", "configure", "moisture-zero", "moisture-upper", "bad-moisture", "missing-zones", "null-zones", "missing-duration", "bad-feedback", "empty-config", "five-config", "configured", "too-many-configured"],
        cases: vec![
            Case::model("Rich Catalogue", "Initial covers nested zone configuration and structured Action output"),
            Case::model("Rich model", "Full covers every supported nested description, required entry and exact bound").selected("irrigation"),
            Case::sample("Configure without optional feedback", Admission::Admitted, "The required identifier list and one zone with id and duration satisfy all constraints, and absent feedback requests no child work"),
            Case::sample("Configure moisture lower boundary", Admission::Admitted, "Present nested moisture 0 meets the inclusive minimum"),
            Case::sample("Configure moisture upper boundary", Admission::Admitted, "Present nested moisture 100 meets the inclusive maximum"),
            constraint(Case::sample("Configure excessive nested moisture", Admission::Rejected, "101 exceeds the nested moisture maximum independently of the valid other entries"), "sample.bound", "bad-moisture", "/zones/0/feedback/moisture", "sdfAction/configure/sdfInputData/properties/zones/items/sdfChoice/zone/properties/feedback/properties/moisture/maximum"),
            constraint(Case::sample("Configure missing required zones", Admission::Rejected, "The required zones entry is absent from the containing map"), "sample.required", "missing-zones", "", "sdfAction/configure/sdfInputData/required/1"),
            constraint(Case::sample("Configure null zones", Admission::Rejected, "A supplied null meets presence but zones explicitly has nullable false"), "sample.null", "null-zones", "/zones", "sdfAction/configure/sdfInputData/properties/zones/nullable"),
            constraint(Case::sample("Configure missing duration despite default", Admission::Rejected, "The zone requires duration and its documentary default 10 inserts no value"), "sample.required", "missing-duration", "/zones/0", "sdfAction/configure/sdfInputData/properties/zones/items/sdfChoice/zone/required/1"),
            constraint(Case::sample("Configure invalid optional feedback", Admission::Rejected, "Present optional feedback must be an object rather than false"), "sample.type", "bad-feedback", "/zones/0/feedback", "sdfAction/configure/sdfInputData/properties/zones/items/sdfChoice/zone/properties/feedback/type"),
            constraint(Case::sample("Configure empty zone array", Admission::Rejected, "The supplied zones array contains fewer than minItems 1"), "sample.count", "empty-config", "/zones", "sdfAction/configure/sdfInputData/properties/zones/minItems"),
            constraint(Case::sample("Configure excessive zone array", Admission::Rejected, "Five zone records exceed maxItems 4, without asserting uniqueness of projected id fields"), "sample.count", "five-config", "/zones", "sdfAction/configure/sdfInputData/properties/zones/maxItems"),
            Case::sample("Configuration applied output", Admission::Admitted, "true is the required Boolean result and four configured zones meet the output count maximum"),
            constraint(Case::sample("Configuration output above maximum", Admission::Rejected, "The supplied configuredZones 5 exceeds the output maximum 4"), "sample.bound", "too-many-configured", "/configuredZones", "sdfAction/configure/sdfOutputData/properties/configuredZones/maximum"),
        ],
    }, Plan {
        device: "irrigation", variant: "limited", file: "limited-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["irrigation", "configure", "calibration", "missing-zones-calibration"],
        cases: vec![
            dependency(Case { complete: false, disposition: Disposition::Undetermined, ..Case::model("Limited Catalogue", "The optional calibration description needs an unsupplied definition, so whole-model work remains incomplete") }),
            dependency(Case { complete: false, disposition: Disposition::Undetermined, ..Case::model("Limited model", "Full cannot establish every model description without calibration meaning") }).selected("irrigation"),
            Case::sample("Absent optional calibration", Admission::Admitted, "All supplied configuration data is supported and absent optional calibration requests no value or reference work"),
            dependency(Case::sample("Present calibration needs definition", Admission::Undetermined, "The supplied calibration value requires unavailable child meaning despite valid zone data")),
            dependency(constraint(Case { complete: false, ..Case::sample("Missing zones despite incomplete calibration", Admission::Rejected, "The absent required zones independently reject the map while supplied calibration remains unfinished") }, "sample.required", "missing-zones-calibration", "", "sdfAction/configure/sdfInputData/required/1")),
        ],
    }]
}

fn dependency(case: Case) -> Case {
    case.diagnostic(Diagnostic::unavailable_reference(Location::new(
        Source,
        "irrigation",
        "/sdfObject/Irrigation/sdfAction/configure/sdfInputData/properties/calibration",
    )))
}

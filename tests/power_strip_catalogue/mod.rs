//! Literal expectations for outlet state, exact meter readings and supplied composition.
use crate::catalogue_support::{
    Admission, Case, Diagnostic, Disposition, Location, LocationKind, Plan,
};
use LocationKind::{Effective, Sample, Source};

fn outlet(pointer: &str) -> Location {
    Location::new(
        Source,
        "strip",
        format!("/sdfThing/PowerStrip/sdfObject/outlet/{pointer}"),
    )
}

fn imported_constraint(
    code: &'static str,
    sample: &'static str,
    property: &str,
    donor: &str,
) -> Diagnostic {
    Diagnostic {
        code,
        primary: Location::new(Sample, sample, ""),
        related: vec![
            Location::new(
                Effective,
                "strip",
                format!("/sdfThing/PowerStrip/sdfProperty/{property}"),
            ),
            Location::new(Source, "meter", donor),
        ],
    }
}

fn unavailable_model(name: &'static str, rationale: &'static str) -> Case {
    Case {
        complete: false,
        disposition: Disposition::Undetermined,
        ..Case::model(name, rationale)
    }
    .diagnostic(Diagnostic::unavailable_reference(Location::new(
        Source,
        "strip",
        "/sdfThing/PowerStrip/sdfProperty/totalPower",
    )))
}

pub fn plans() -> Vec<Plan> {
    vec![Plan {
        device: "power-strip", variant: "minimal", file: "minimal-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["strip", "on", "off", "text-state", "zero", "outlet-upper", "above-outlet"],
        cases: vec![
            Case::model("Minimal Catalogue", "Initial covers the repeated outlet's supported Boolean state and power description"),
            Case::model("Minimal model", "Full covers the repeated outlet's supported Boolean state and power description").selected("strip"),
            Case::sample("Outlet on", Admission::Admitted, "true is the declared Boolean state"),
            Case::sample("Outlet off", Admission::Admitted, "false is also a Boolean state"),
            Case::sample("Outlet text state", Admission::Rejected, "The text true is not a Boolean state")
                .diagnostic(Diagnostic::constraint("sample.type", Location::new(Sample, "text-state", ""), outlet("sdfProperty/on/type"))),
            Case::sample("Outlet power lower boundary", Admission::Admitted, "0 equals the inclusive power minimum"),
            Case::sample("Outlet power upper boundary", Admission::Admitted, "2400 equals the inclusive power maximum"),
            Case::sample("Outlet power above maximum", Admission::Rejected, "2400.001 exceeds the inclusive maximum 2400")
                .diagnostic(Diagnostic::constraint("sample.bound", Location::new(Sample, "above-outlet", ""), outlet("sdfProperty/power/maximum"))),
        ],
    },
    Plan {
        device: "power-strip", variant: "rich", file: "rich-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["strip", "meter", "fractional-power", "outlet-upper", "ordinary-total", "total-upper", "above-total", "ordinary-energy", "large-fraction", "energy-upper", "above-energy", "off-step", "zero"],
        cases: vec![
            Case::model("Rich Catalogue", "Initial checks both supplied documents, including reusable meter and outlet definitions"),
            Case::model("Rich model", "Full checks the strip with its explicit supplied meter dependency").selected("strip")
                .diagnostic(Diagnostic {
                    code: "unit.secondary",
                    primary: Location::new(Effective, "strip", "/sdfThing/PowerStrip/sdfProperty/energy/unit"),
                    related: vec![Location::new(Source, "meter", "/sdfData/Energy/unit")],
                }),
            Case::sample("Outlet fractional power", Admission::Admitted, "12.345 is an exact nonnegative watt reading below the inherited maximum 3600"),
            Case::sample("Shared power above product bound", Admission::Admitted, "2400 meets shared Power's maximum 3600 independently of the consumer's override"),
            Case::sample("Total power within override", Admission::Admitted, "1200.125 is within the consumer's inclusive 0 through 1800 bound"),
            Case::sample("Total power override boundary", Admission::Admitted, "1800 equals the explicit consumer maximum"),
            Case::sample("Total power just above override", Admission::Rejected, "1800.001 is admitted by the shared maximum 3600 but exceeds the consumer's explicit maximum 1800")
                .diagnostic(Diagnostic::constraint("sample.bound", Location::new(Sample, "above-total", ""), Location::new(Source, "strip", "/sdfThing/PowerStrip/sdfProperty/totalPower/maximum"))),
            Case::sample("Total power rejects shared reading", Admission::Rejected, "The same 2400 value admitted by the shared definition exceeds the product override 1800")
                .diagnostic(Diagnostic::constraint("sample.bound", Location::new(Sample, "outlet-upper", ""), Location::new(Source, "strip", "/sdfThing/PowerStrip/sdfProperty/totalPower/maximum"))),
            Case::sample("Ordinary fractional energy", Admission::Admitted, "42.125 is nonnegative and exactly a multiple of 0.125 watt-hours"),
            Case::sample("Large fractional energy", Admission::Admitted, "9007199254740993.125 is below 9007199254740994 and exactly a multiple of 0.125"),
            Case::sample("Large exact energy boundary", Admission::Admitted, "9007199254740994 equals the inclusive energy maximum and is a multiple of 0.125"),
            Case::sample("Energy above exact boundary", Admission::Rejected, "9007199254740994.125 exceeds the exact maximum despite rounding to the same ordinary machine float")
                .diagnostic(imported_constraint("sample.bound", "above-energy", "energy/maximum", "/sdfData/Energy/maximum")),
            Case::sample("Energy off eighth step", Admission::Rejected, "0.0625 is half of 0.125, not an integer multiple of the declared step")
                .diagnostic(imported_constraint("sample.multiple", "off-step", "energy/multipleOf", "/sdfData/Energy/multipleOf")),
            Case::sample("Shared energy lower boundary", Admission::Admitted, "0 meets the shared Energy minimum and is a multiple of 0.125"),
        ],
    },
    Plan {
        device: "power-strip", variant: "missing-dependency", file: "missing-plan.json", exit: 3, failed_sources: &["meter"],
        source_keys: &["strip", "meter", "on", "ordinary-total"],
        cases: vec![
            unavailable_model("Missing dependency Catalogue", "The explicitly named meter document cannot be acquired, and totalPower needs its shared definition"),
            unavailable_model("Missing dependency model", "Full cannot establish totalPower without the named local dependency").selected("strip"),
            Case::sample("Outlet state survives missing dependency", Admission::Admitted, "The directly authored Boolean state needs no imported definition"),
            Case::sample("Total power needs dependency", Admission::Undetermined, "1200.125 meets the independent maximum but the absent donor's other constraints are unavailable")
                .diagnostic(Diagnostic::unavailable_reference(Location::new(Source, "strip", "/sdfThing/PowerStrip/sdfProperty/totalPower"))),
        ],
    },
    Plan {
        device: "power-strip", variant: "copied-requiredness", file: "copied-plan.json", exit: 0, failed_sources: &[],
        source_keys: &["strip", "meter", "on", "fractional-power"],
        cases: vec![
            Case::model("Copied requiredness Catalogue", "The surviving copied required reference relocates to the importing outlet's own Boolean declaration"),
            Case::model("Copied requiredness model", "Full resolves the copied declaration at the outlet's coordinate in the resolved model").selected("strip"),
            Case::sample("Copied outlet state", Admission::Admitted, "true is the inherited Boolean state that the copied requirement selects"),
            Case::sample("Independent total power", Admission::Admitted, "12.345 meets the independently authored total power description beside the copied declaration"),
        ],
    }]
}

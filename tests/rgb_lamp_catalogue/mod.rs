//! Literal expectations for the authored lamp constraints.
use crate::catalogue_support::{Admission, Case, Diagnostic, Location, LocationKind, Plan};
use LocationKind::{Sample, Source};

fn source(pointer: &str) -> Location {
    Location::new(Source, "lamp", format!("/sdfObject/Lamp/{pointer}"))
}

pub fn plans() -> Vec<Plan> {
    vec![Plan {
        device: "rgb-lamp", variant: "minimal", file: "minimal-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["lamp", "on", "off", "text-on", "lower", "upper", "above"],
        cases: vec![
            Case::model("Minimal Catalogue", "Initial covers the supported Boolean and brightness descriptions"),
            Case::model("Minimal model", "Full covers the supported Boolean and brightness descriptions").selected("lamp"),
            Case::sample("Lamp on", Admission::Admitted, "true is the declared Boolean state"),
            Case::sample("Lamp off", Admission::Admitted, "false is also a Boolean state, rather than an absent value"),
            Case::sample("Text is not on state", Admission::Rejected, "The string on is not a Boolean")
                .diagnostic(Diagnostic::constraint("sample.type", Location::new(Sample, "text-on", ""), source("sdfProperty/on/type"))),
            Case::sample("Brightness lower boundary", Admission::Admitted, "0 equals the inclusive minimum brightness"),
            Case::sample("Brightness upper boundary", Admission::Admitted, "100 equals the inclusive maximum brightness"),
            Case::sample("Brightness above maximum", Admission::Rejected, "101 exceeds the maximum 100")
                .diagnostic(Diagnostic::constraint("sample.bound", Location::new(Sample, "above", ""), source("sdfProperty/brightness/maximum"))),
        ],
    },
    Plan {
        device: "rgb-lamp", variant: "rich", file: "rich-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["lamp", "rgb", "invalid-rgb", "mode", "unknown-mode", "dim-red", "bright-blue", "mixed-scene", "applied", "refused"],
        cases: vec![
            Case::model("Rich Catalogue", "Initial covers RGB data, modes and scene Action descriptions"),
            Case::model("Rich model", "Full completes the supported RGB, Choice and const descriptions").selected("lamp"),
            Case::sample("RGB channel boundaries", Admission::Admitted, "All required channels are integers from 0 through 255"),
            Case::sample("RGB green above maximum", Admission::Rejected, "Green 256 exceeds its maximum 255 in the compound value")
                .diagnostic(Diagnostic::constraint("sample.bound", Location::new(Sample, "invalid-rgb", "/green"), source("sdfProperty/rgb/properties/green/maximum"))),
            Case::sample("Steady mode", Admission::Admitted, "steady is one of the declared steady and pulse modes"),
            Case::sample("Unknown mode", Admission::Rejected, "strobe matches neither enumerated mode")
                .enum_rejection(Location::new(Sample, "unknown-mode", ""), source("sdfProperty/mode/enum"), 2),
            Case::sample("Dim red scene", Admission::Admitted, "red and brightness 20 satisfy the whole dimRed branch, including its upper boundary"),
            Case::sample("Bright blue scene", Admission::Admitted, "blue and brightness 80 satisfy the whole brightBlue branch, including its lower boundary"),
            Case::sample("Mixed scene has no Choice witness", Admission::Rejected, "red fails the brightBlue const while brightness 80 fails the dimRed maximum")
                .diagnostic(Diagnostic::constraint("sample.bound", Location::new(Sample, "mixed-scene", "/brightness"), source("sdfAction/setScene/sdfInputData/sdfChoice/dimRed/properties/brightness/maximum")))
                .diagnostic(Diagnostic::constraint("sample.const", Location::new(Sample, "mixed-scene", "/preset"), source("sdfAction/setScene/sdfInputData/sdfChoice/brightBlue/properties/preset/const"))),
            Case::sample("Scene applied output", Admission::Admitted, "The required applied Boolean equals const true and optional brightness is within bounds"),
            Case::sample("Scene refused output", Admission::Rejected, "false is a Boolean but violates the output's const true")
                .diagnostic(Diagnostic::constraint("sample.const", Location::new(Sample, "refused", "/applied"), source("sdfAction/setScene/sdfOutputData/properties/applied/const"))),
        ],
    },
    Plan {
        device: "rgb-lamp", variant: "limited", file: "limited-plan.json", exit: 3, failed_sources: &[],
        source_keys: &["lamp", "calibration"],
        cases: vec![
            Case { complete: false, disposition: crate::catalogue_support::Disposition::Undetermined,
                ..Case::model("Limited Catalogue", "The supplied model needs an unsupplied brightness calibration definition") }
                .diagnostic(Diagnostic::unavailable_reference(source("sdfProperty/brightness"))),
            Case { complete: false, disposition: crate::catalogue_support::Disposition::Undetermined,
                ..Case::model("Limited model", "Full cannot complete the unsupplied brightness description") }.selected("lamp")
                .diagnostic(Diagnostic::unavailable_reference(source("sdfProperty/brightness"))),
            Case::sample("Brightness needs calibration", Admission::Undetermined, "50 has no established selected constraints without the calibration definition")
                .diagnostic(Diagnostic::unavailable_reference(source("sdfProperty/brightness"))),
        ],
    }]
}

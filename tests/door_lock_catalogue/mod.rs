//! Literal expectations for the authored lock constraints.
use crate::catalogue_support::{
    Admission, Case, Classification, Diagnostic, Disposition, Location, LocationKind, Plan,
};
use LocationKind::{Sample, Source};

fn source(pointer: &str) -> Location {
    Location::new(Source, "lock", format!("/sdfObject/Lock/{pointer}"))
}

fn faulty_model(name: &'static str, rationale: &'static str) -> Case {
    Case {
        complete: false,
        disposition: Disposition::Failed,
        ..Case::model(name, rationale)
    }
    .diagnostic(Diagnostic {
        code: "data.number",
        primary: source("sdfProperty/attempts/maximum"),
        related: vec![],
    })
    .diagnostic(Diagnostic::unavailable_reference(source(
        "sdfProperty/credential",
    )))
}

pub fn plans() -> Vec<Plan> {
    vec![Plan {
        device: "door-lock", variant: "minimal", file: "minimal-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["lock", "locked", "unknown-state", "lower-pin", "upper-pin", "short-pin", "letter-pin"],
        cases: vec![
            Case::model("Minimal Catalogue", "Initial covers the supported lock state and PIN descriptions"),
            Case::model("Minimal model", "Full covers the supported lock state and PIN descriptions").selected("lock"),
            Case::sample("Locked state", Admission::Admitted, "locked is one of the two enumerated states"),
            Case::sample("Unknown lock state", Admission::Rejected, "jammed is not an enumerated state in the minimal model")
                .enum_rejection(Location::new(Sample, "unknown-state", ""), source("sdfProperty/state/enum"), 2),
            Case::sample("PIN lower length boundary", Admission::Admitted, "1234 has exactly four scalar characters and contains only ASCII digits"),
            Case::sample("PIN upper length boundary", Admission::Admitted, "12345678 has exactly eight scalar characters and contains only ASCII digits"),
            Case::sample("PIN too short", Admission::Rejected, "123 has three scalar characters, below the minimum four")
                .diagnostic(Diagnostic::constraint("sample.length", Location::new(Sample, "short-pin", ""), source("sdfProperty/credential/minLength"))),
            Case::sample("PIN has a letter", Admission::Rejected, "12a4 meets the length bounds but fails the anchored digit pattern")
                .diagnostic(Diagnostic::constraint("sample.pattern", Location::new(Sample, "letter-pin", ""), source("sdfProperty/credential/pattern"))),
        ],
    },
    Plan {
        device: "door-lock", variant: "rich", file: "rich-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["lock", "unicode", "short-unicode", "spaced", "missing-operation", "null-credential", "result", "wrong-result", "event", "invalid-date"],
        cases: vec![
            Case::model("Rich Catalogue", "Initial covers lock states, authorization and timestamped Event data"),
            Case::model("Rich model", "Full completes the supported credential, map and civil format descriptions").selected("lock"),
            Case::diff("Minimal to rich revision", Classification::Breaking, Disposition::Passed, "The added jammed state and non-digit credentials break earlier Clients, and both are acknowledged as intended"),
            Case::sample("Authorize Unicode credential", Admission::Admitted, "é, the musical clef, 门 and a are four Unicode scalars with no spaces, and unlock is enumerated"),
            Case::sample("Authorize short Unicode credential", Admission::Rejected, "é, the musical clef and 门 are three Unicode scalars despite occupying nine UTF-8 bytes")
                .diagnostic(Diagnostic::constraint("sample.length", Location::new(Sample, "short-unicode", "/credential"), source("sdfAction/authorize/sdfInputData/properties/credential/minLength"))),
            Case::sample("Authorize spaced credential", Admission::Rejected, "ab c has four scalars but its space violates the anchored no-space pattern")
                .diagnostic(Diagnostic::constraint("sample.pattern", Location::new(Sample, "spaced", "/credential"), source("sdfAction/authorize/sdfInputData/properties/credential/pattern"))),
            Case::sample("Authorize missing operation", Admission::Rejected, "The credential is present, but required operation is absent")
                .diagnostic(Diagnostic::constraint("sample.required", Location::new(Sample, "missing-operation", ""), source("sdfAction/authorize/sdfInputData/required/1"))),
            Case::sample("Authorize null credential", Admission::Rejected, "A present null meets required presence but credential explicitly forbids null")
                .diagnostic(Diagnostic::constraint("sample.null", Location::new(Sample, "null-credential", "/credential"), source("sdfAction/authorize/sdfInputData/properties/credential/nullable"))),
            Case::sample("Authorization result", Admission::Admitted, "accepted is Boolean and unlocked is a declared result state"),
            Case::sample("Authorization result wrong type", Admission::Rejected, "The text true is not a Boolean accepted result")
                .diagnostic(Diagnostic::constraint("sample.type", Location::new(Sample, "wrong-result", "/accepted"), source("sdfAction/authorize/sdfOutputData/properties/accepted/type"))),
            Case::sample("Timestamped locked event", Admission::Admitted, "locked is enumerated and 2024-01-01T12:34:56Z is an established ordinary date-time"),
            Case::sample("Impossible event date", Admission::Rejected, "February 30 is impossible in the Gregorian calendar")
                .diagnostic(Diagnostic::constraint("sample.temporal_calendar", Location::new(Sample, "invalid-date", "/at"), source("sdfEvent/stateChanged/sdfOutputData/properties/at/format"))),
        ],
    },
    Plan {
        device: "door-lock", variant: "limited", file: "limited-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["lock", "locked", "credential"],
        cases: vec![
            faulty_model("Limited Catalogue", "A textual attempts maximum is invalid and the credential definition is unsupplied"),
            faulty_model("Limited model", "Full retains both the definite model finding and unavailable credential meaning").selected("lock"),
            Case::sample("State survives unrelated model faults", Admission::Admitted, "locked meets the selected state's enum without requesting the unrelated faulty or missing descriptions"),
            Case::sample("Credential needs dependency", Admission::Undetermined, "The selected credential depends on a deliberately unsupplied definition")
                .diagnostic(Diagnostic::unavailable_reference(source("sdfProperty/credential"))),
        ],
    }]
}

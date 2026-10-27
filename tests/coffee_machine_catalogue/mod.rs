//! Literal expectations for resource levels, whole recipe witnesses and structured outputs.
use crate::catalogue_support::{
    Admission, Case, Diagnostic, Disposition, Location, LocationKind, Plan,
};
use LocationKind::{Sample, Source};

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
            "coffee",
            format!("/sdfObject/CoffeeMachine/{quality}"),
        ),
    ))
}

pub fn plans() -> Vec<Plan> {
    vec![Plan {
        device: "coffee-machine", variant: "minimal", file: "minimal-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["coffee", "resource-zero", "resource-upper", "resource-above", "missing-beans", "null-beans", "espresso", "unknown-preparation"],
        cases: vec![
            Case::model("Minimal Catalogue", "Initial covers resource percentages and an enumerated preparation"),
            Case::model("Minimal model", "Full covers supported required resource entries, inclusive bounds and preparation alternatives").selected("coffee"),
            Case::sample("Resources lower boundary", Admission::Admitted, "Required water and beans at 0 meet the inclusive lower bounds and optional milk is absent"),
            Case::sample("Resources upper boundary", Admission::Admitted, "Water, beans and optional milk at 100 meet their inclusive upper bounds"),
            constraint(Case::sample("Water above maximum", Admission::Rejected, "Water 101 exceeds the resource percentage maximum 100"), "sample.bound", "resource-above", "/water", "sdfProperty/resources/properties/water/maximum"),
            constraint(Case::sample("Missing required beans", Admission::Rejected, "Beans are absent from the required resource map"), "sample.required", "missing-beans", "", "sdfProperty/resources/required/1"),
            constraint(Case::sample("Present null beans", Admission::Rejected, "Null beans meet presence but violate explicit nullable false"), "sample.null", "null-beans", "/beans", "sdfProperty/resources/properties/beans/nullable"),
            Case::sample("Espresso preparation", Admission::Admitted, "espresso is one of the two declared preparation literals"),
            Case::sample("Unknown preparation", Admission::Rejected, "tea is neither espresso nor latte")
                .enum_rejection(Location::new(Sample, "unknown-preparation", ""), Location::new(Source, "coffee", "/sdfObject/CoffeeMachine/sdfProperty/preparation/enum"), 2),
        ],
    }, Plan {
        device: "coffee-machine", variant: "rich", file: "rich-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["coffee", "brew-espresso", "brew-latte", "mixed-recipe", "excess-servings", "missing-servings", "brew-result", "bad-result", "maintenance", "unknown-maintenance", "bad-maintenance-resource"],
        cases: vec![
            Case::model("Rich Catalogue", "Initial covers preparation choices and structured brew and maintenance data"),
            Case::model("Rich model", "Full covers supported recursive maps, alternatives and resource bounds").selected("coffee"),
            Case::sample("Brew espresso lower boundaries", Admission::Admitted, "One complete espresso branch admits drink espresso with size 25 and outer servings 1"),
            Case::sample("Brew latte upper boundaries", Admission::Admitted, "One complete latte branch admits drink latte with size 300 and outer servings 5"),
            constraint(constraint(Case::sample("Mixed recipe has no Choice witness", Admission::Rejected, "espresso with size 300 violates espresso's maximum 60 and latte's const, so constraints from different branches cannot assemble admission"), "sample.bound", "mixed-recipe", "/recipe/size", "sdfAction/brew/sdfInputData/properties/recipe/sdfChoice/espresso/properties/size/maximum"), "sample.const", "mixed-recipe", "/recipe/drink", "sdfAction/brew/sdfInputData/properties/recipe/sdfChoice/latte/properties/drink/const"),
            constraint(Case::sample("Recipe witness cannot admit excessive servings", Admission::Rejected, "A complete latte child witness does not erase the outer servings 6 above maximum 5"), "sample.bound", "excess-servings", "/servings", "sdfAction/brew/sdfInputData/properties/servings/maximum"),
            constraint(Case::sample("Missing servings despite default", Admission::Rejected, "The required servings entry is absent and documentary default 1 inserts no value"), "sample.required", "missing-servings", "", "sdfAction/brew/sdfInputData/required/1"),
            Case::sample("Structured brew result", Admission::Admitted, "The required result is Boolean and message is nonempty text"),
            constraint(Case::sample("Brew result wrong type", Admission::Rejected, "The result string true is not the declared Boolean"), "sample.type", "bad-result", "/result", "sdfAction/brew/sdfOutputData/properties/result/type"),
            Case::sample("Maintenance clean event", Admission::Admitted, "clean is an enumerated reason and the nested required water and beans readings meet their percentage bounds"),
            Case::sample("Unknown maintenance reason", Admission::Rejected, "polish is outside the clean/refill maintenance reason enumeration")
                .enum_rejection(Location::new(Sample, "unknown-maintenance", "/reason"), Location::new(Source, "coffee", "/sdfObject/CoffeeMachine/sdfEvent/maintenance/sdfOutputData/properties/reason/enum"), 2),
            constraint(Case::sample("Maintenance nested resource above maximum", Admission::Rejected, "Valid clean reason cannot erase the independent nested water 101 rejection"), "sample.bound", "bad-maintenance-resource", "/resources/water", "sdfEvent/maintenance/sdfOutputData/properties/resources/properties/water/maximum"),
        ],
    }, Plan {
        device: "coffee-machine", variant: "limited", file: "limited-plan.json", exit: 1, failed_sources: &[],
        source_keys: &["coffee", "brew-espresso", "calibration", "excess-calibration"],
        cases: vec![
            dependency(Case { complete: false, disposition: Disposition::Undetermined, ..Case::model("Limited Catalogue", "The optional brew calibration has no supplied meaning, leaving model work incomplete") }),
            dependency(Case { complete: false, disposition: Disposition::Undetermined, ..Case::model("Limited model", "Full retains the unavailable optional calibration description") }).selected("coffee"),
            Case::sample("Brew without optional calibration", Admission::Admitted, "The complete espresso witness and valid servings admit supplied data while absent optional calibration requests no child work"),
            dependency(Case::sample("Brew calibration needs definition", Admission::Undetermined, "Valid recipe and servings cannot establish supplied calibration meaning without its definition")),
            dependency(constraint(Case { complete: false, ..Case::sample("Excess servings despite incomplete calibration", Admission::Rejected, "Servings 6 independently violate maximum 5 while present calibration remains unfinished") }, "sample.bound", "excess-calibration", "/servings", "sdfAction/brew/sdfInputData/properties/servings/maximum")),
        ],
    }]
}

fn dependency(case: Case) -> Case {
    case.diagnostic(Diagnostic::unavailable_reference(Location::new(
        Source,
        "coffee",
        "/sdfObject/CoffeeMachine/sdfAction/brew/sdfInputData/properties/calibration",
    )))
}

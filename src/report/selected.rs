//! The report parts that `generate` and `trace` share. Each composes one selected model
//! document once and checks values against it, so both reports name the same scope, the same
//! value interpretations and the same sources, following ADR 0011: Machine-readable report
//! contracts and ADR 0013: Shared workflow and report guarantees.
use crate::effective::EffectiveModel;
use crate::role::SetAside;
use crate::sample::ValueInterpretations;
use crate::{Catalogue, SourceId};
use serde_json::{Value, json};

/// One selected model document, the other sources its composition used or needed, and the
/// interpretations and set-aside extension vocabulary that composing it and checking values
/// against it relied on.
pub(crate) struct SelectedScope {
    document: SourceId,
    dependencies: Vec<SourceId>,
    unavailable: Vec<SourceId>,
    /// Composition relied on copied-reference relocation.
    relocation: bool,
    /// A requested selector names Context, including when its details are omitted.
    pub(crate) context: bool,
    /// The value interpretations the checks relied on.
    pub(crate) interpretations: ValueInterpretations,
    /// The extension vocabulary composition and the checks set aside.
    pub(crate) set_aside: SetAside,
}

impl SelectedScope {
    pub(crate) fn new(document: SourceId) -> Self {
        Self {
            document,
            dependencies: Vec::new(),
            unavailable: Vec::new(),
            relocation: false,
            context: false,
            interpretations: ValueInterpretations::default(),
            set_aside: SetAside::default(),
        }
    }

    pub(crate) fn document(&self) -> SourceId {
        self.document
    }

    /// Record what composing the selected document relied on.
    pub(crate) fn composed(&mut self, model: &EffectiveModel<'_>) {
        for (found, kept) in [
            (&model.dependencies, &mut self.dependencies),
            (&model.unavailable_dependencies, &mut self.unavailable),
        ] {
            for id in found {
                if *id != self.document && !kept.contains(id) {
                    kept.push(*id);
                }
            }
        }
        self.relocation = model.relocation_interpretation;
    }

    /// The request's selected scope: the document as its one root, the other model sources
    /// its composition used as dependencies, both as assessed, and the needed sources it could
    /// not use as unavailable.
    pub(crate) fn scope_json(&self) -> Value {
        let document = self.document.get();
        let dependencies: Vec<u32> = self.dependencies.iter().map(|id| id.get()).collect();
        let mut assessed = vec![document];
        assessed.extend(&dependencies);
        let unavailable: Vec<u32> = self.unavailable.iter().map(|id| id.get()).collect();
        json!({"kind":"selected","roots":[document],"dependencies":dependencies,
            "assessed":assessed,"unavailable_required":unavailable})
    }

    /// The baseline, naming the interpretations composition and the checks relied on.
    pub(crate) fn baseline_json(&self) -> Value {
        let mut interpretations: Vec<&str> = self.interpretations.ids().collect();
        if self.relocation {
            interpretations.push("copied-reference-relocation/1");
        }
        if self.set_aside.relies_on_reading(false) {
            interpretations.push("undeclared-extensions-ignorable/1");
        }
        let mut baseline = json!({"id":"rfc9880/base-1","rfc":"9880","corrections":[],
            "interpretations":interpretations,"deviations":[],"registry_knowledge":[]});
        if self.context {
            baseline["extensions"] = crate::context::support();
        }
        baseline
    }
}

/// The row of every source `catalogue` retains, then the `extra` rows, with the presentation
/// omissions of their labels.
pub(crate) fn sources(
    catalogue: &Catalogue,
    extra: impl IntoIterator<Item = (Value, Option<Value>)>,
) -> (Vec<Value>, Vec<Value>) {
    let mut sources = Vec::new();
    let mut presentation = Vec::new();
    let rows = catalogue
        .retained_sources()
        .map(super::source_wire)
        .chain(extra);
    for (row, omission) in rows {
        sources.push(row);
        presentation.extend(omission);
    }
    (sources, presentation)
}

/// The distinct reasons of `omissions`, in order, as `diagnostics.reasons` lists them.
pub(crate) fn reasons(omissions: &[Value]) -> Vec<Value> {
    let mut reasons: Vec<Value> = Vec::new();
    for omission in omissions {
        if !reasons.contains(&omission["reason"]) {
            reasons.push(omission["reason"].clone());
        }
    }
    reasons
}

/// The report `base` as JSON text, with the already rendered `details` written as the array
/// member `field` at the end of its `result` map.
pub(crate) fn splice(mut base: Value, field: &str, details: &str) -> String {
    let result = base
        .as_object_mut()
        .expect("report map")
        .remove("result")
        .expect("result");
    let mut output = serde_json::to_string(&base).expect("bounded report envelope");
    output.pop();
    output.push_str(",\"result\":");
    let mut result = serde_json::to_string(&result).expect("bounded report result");
    result.pop();
    output.push_str(&result);
    output.push_str(&format!(",\"{field}\":["));
    output.push_str(details);
    output.push_str("]}}");
    output
}

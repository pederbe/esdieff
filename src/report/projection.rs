//! Shared presentation policy, following ADR 0009: Bounded processing and reporting.
//! Operation-specific builders retain responsibility for meaning and exact payload encoding.
use serde_json::{Value, json};

/// A retained-wire precharge. Construction allowance is consumed permanently.
pub(crate) struct Charge(usize);

#[derive(Clone)]
pub(super) struct DetailBudget {
    remaining: usize,
    scratch: usize,
    count_omitted: bool,
    byte_omitted: bool,
}
impl DetailBudget {
    pub(super) fn remaining(&self) -> usize {
        self.remaining
    }
    pub(super) fn fits(&self, wire: usize, scratch: usize) -> bool {
        self.remaining >= wire && self.scratch >= scratch
    }
    pub(super) fn admit(&mut self, wire: usize, scratch: usize) -> Option<Charge> {
        if !self.fits(wire, scratch) {
            return None;
        }
        self.remaining -= wire;
        self.scratch -= scratch;
        Some(Charge(wire))
    }
    pub(super) fn settle(&mut self, charge: Charge, actual: usize) {
        assert!(actual <= charge.0, "detail exceeds its wire precharge");
        self.remaining += charge.0 - actual;
    }
    pub(super) fn summary<'a>(
        &self,
        total: usize,
        retained: usize,
        omissions: &'a [Value],
        detail_omitted: bool,
        extra_reasons: [Option<&'static str>; 3],
    ) -> (usize, bool, Vec<&'a str>) {
        let omitted = total - retained;
        let truncated = omitted > 0
            || !omissions.is_empty()
            || detail_omitted
            || extra_reasons.iter().any(Option::is_some);
        let mut reasons = Vec::new();
        if omitted > 0 {
            if self.count_omitted {
                reasons.push("count_limit");
            }
            if self.byte_omitted {
                reasons.push("byte_limit");
            }
        }
        for reason in omissions
            .iter()
            .map(|value| value["reason"].as_str().expect("omission reason"))
            .chain(extra_reasons.into_iter().flatten())
        {
            if !reasons.contains(&reason) {
                reasons.push(reason);
            }
        }
        debug_assert!(reasons.len() <= 8, "fixed report reason inventory");
        (omitted, truncated, reasons)
    }
    pub(super) fn with_scratch<T>(&mut self, make: impl FnOnce(&mut usize) -> T) -> T {
        make(&mut self.scratch)
    }
    pub(super) fn allow_count(&mut self, retained: usize, cap: usize) -> bool {
        if retained >= cap {
            self.count_omitted = true;
            false
        } else {
            true
        }
    }
    pub(super) fn observe_omitted(&mut self, count: bool, bytes: bool) {
        self.count_omitted |= count;
        self.byte_omitted |= bytes;
    }
    pub(super) fn count_omitted(&self) -> bool {
        self.count_omitted
    }
    pub(super) fn byte_omitted(&self) -> bool {
        self.byte_omitted
    }
}

/// Opaque cumulative allowances carried from checking into a containing report.
#[derive(Clone)]
pub(crate) struct Projection {
    detail: DetailBudget,
    pointers: crate::json::PointerBudget,
}
impl Projection {
    pub(crate) fn new(limits: &crate::Limits, reserve: usize) -> Self {
        Self {
            detail: DetailBudget {
                remaining: limits.detail_bytes.min(limits.report_bytes - reserve),
                scratch: (limits.report_bytes - reserve) * 4,
                count_omitted: false,
                byte_omitted: false,
            },
            pointers: crate::json::PointerBudget::new(limits.pointer_total),
        }
    }
    pub(super) fn parts(&mut self) -> (&mut DetailBudget, &mut crate::json::PointerBudget) {
        (&mut self.detail, &mut self.pointers)
    }
    pub(crate) fn admit_traversal(&mut self, work: usize) -> bool {
        self.pointers.debit(work).is_ok()
    }
}

/// Continue a diagnostic projection without rebuilding allowances or copied omissions.
pub(crate) struct DiagnosticProjection {
    items: Vec<Value>,
    omitted: u64,
    truncated: bool,
    reasons: Vec<Value>,
    omissions: Vec<Value>,
    added_omissions: Vec<Value>,
    projection: Projection,
}
impl DiagnosticProjection {
    pub(crate) fn new(copied: Option<&Value>, projection: Projection) -> Self {
        Self {
            items: Vec::new(),
            omitted: copied
                .and_then(|value| value["omitted"].as_u64())
                .unwrap_or(0),
            truncated: copied.is_some_and(|value| value["truncated"] == true),
            reasons: copied
                .and_then(|value| value["reasons"].as_array())
                .cloned()
                .unwrap_or_default(),
            omissions: copied
                .and_then(|value| value["omissions"].as_array())
                .cloned()
                .unwrap_or_default(),
            added_omissions: Vec::new(),
            projection,
        }
    }
    pub(crate) fn push(&mut self, item: Value) {
        self.items.push(item);
    }
    pub(crate) fn add_omitted(&mut self, count: u64) {
        self.omitted += count;
    }
    pub(crate) fn record(&mut self, field: &'static str, reason: &'static str, count: u64) {
        if count == 0 {
            return;
        }
        self.truncated = true;
        let reason_value = json!(reason);
        if !self.reasons.contains(&reason_value) {
            self.reasons.push(reason_value);
        }
        if let Some(omission) = self
            .added_omissions
            .iter_mut()
            .find(|value| value["field"] == field && value["reason"] == reason)
        {
            omission["omitted_count"] =
                json!(omission["omitted_count"].as_u64().expect("omitted count") + count);
        } else {
            self.added_omissions
                .push(json!({"field":field,"reason":reason,"omitted_count":count}));
        }
    }
    fn omit_item(&mut self, reason: &'static str) {
        self.omitted += 1;
        self.record("diagnostics.items", reason, 1);
    }
    pub(crate) fn allow_item(&mut self, cap: usize) -> bool {
        if self.projection.detail.allow_count(self.items.len(), cap) {
            true
        } else {
            self.omit_item("count_limit");
            false
        }
    }
    pub(crate) fn admit_item(&mut self, wire: usize, scratch: usize) -> Option<Charge> {
        let charge = self.admit(wire, scratch);
        if charge.is_none() {
            self.omit_item("byte_limit");
        }
        charge
    }
    pub(crate) fn admit(&mut self, wire: usize, scratch: usize) -> Option<Charge> {
        self.projection.detail.admit(wire, scratch)
    }
    pub(crate) fn settle(&mut self, charge: Charge, actual: usize) {
        self.projection.detail.settle(charge, actual);
    }
    pub(crate) fn present_pointer(&mut self, pointer: &str, cap: usize) -> bool {
        pointer.len() <= cap && self.projection.pointers.debit(pointer.len() + 1).is_ok()
    }
    /// Origin lookup consumes traversal and pointer construction from the same allowance.
    pub(crate) fn lookup<T>(&mut self, find: impl FnOnce(&mut usize, &mut usize) -> T) -> T {
        let mut work = *self.projection.pointers.counter();
        let mut bytes = work;
        let result = find(&mut work, &mut bytes);
        *self.projection.pointers.counter() = work.min(bytes);
        result
    }
    pub(crate) fn finish(mut self) -> Value {
        self.omissions.extend(self.added_omissions);
        json!({"retained":self.items.len(),"omitted":self.omitted,"truncated":self.truncated,"reasons":self.reasons,"omissions":self.omissions,"items":self.items})
    }
}

pub(super) fn retain(
    items: &mut Vec<Value>,
    budget: &mut DetailBudget,
    cap: usize,
    make: impl FnOnce() -> Value,
) {
    if !budget.allow_count(items.len(), cap) {
        return;
    }
    // These conditions contain bounded literals or fixed-size counts and spans.
    let Some(charge) = budget.admit(512, 32768) else {
        budget.observe_omitted(false, true);
        return;
    };
    let condition = make();
    let size = serde_json::to_string(&condition)
        .expect("condition encoding")
        .len()
        + 1;
    budget.settle(charge, size);
    items.push(condition);
}

pub(crate) fn encode_author(mut value: Value, limit: usize) -> String {
    let mut item_omission = None;
    let mut row_diagnostic_omission = None;
    let mut row_omission = None;
    let mut requirement_diagnostic_omission = None;
    let mut requirement_omission = None;
    loop {
        let text = serde_json::to_string(&value).expect("bounded author DTO");
        if text.len() <= limit {
            return text;
        }
        if let Some(item) = value["diagnostics"]["items"]
            .as_array_mut()
            .expect("conditions")
            .pop()
        {
            drop(item);
            let omitted = value["diagnostics"]["omitted"].as_u64().expect("count") + 1;
            value["diagnostics"]["omitted"] = json!(omitted);
            value["diagnostics"]["retained"] = json!(
                value["diagnostics"]["items"]
                    .as_array()
                    .expect("conditions")
                    .len()
            );
            fallback_omission(
                &mut value["diagnostics"]["omissions"],
                &mut item_omission,
                "diagnostics.items",
            );
            mark_report_limit(&mut value["diagnostics"]);
            continue;
        }
        if value["result"]["checking"]["coverage"]["rows"]
            .as_array_mut()
            .is_some_and(|rows| rows.pop().is_some())
        {
            let coverage = &mut value["result"]["checking"]["coverage"];
            coverage["rows_omitted"] =
                json!(coverage["rows_omitted"].as_u64().expect("row count") + 1);
            fallback_omission(
                &mut coverage["omissions"],
                &mut row_omission,
                "coverage.rows",
            );
            fallback_omission(
                &mut value["diagnostics"]["omissions"],
                &mut row_diagnostic_omission,
                "coverage.rows",
            );
            mark_report_limit(&mut value["diagnostics"]);
            continue;
        }
        if value["result"]["checking"]["coverage"]["requirement_rows"]
            .as_array_mut()
            .is_some_and(|rows| rows.pop().is_some())
        {
            let coverage = &mut value["result"]["checking"]["coverage"];
            coverage["requirement_rows_retained"] = json!(
                coverage["requirement_rows"]
                    .as_array()
                    .expect("requirement rows")
                    .len()
            );
            coverage["requirement_rows_omitted"] = json!(
                coverage["requirement_rows_omitted"]
                    .as_u64()
                    .expect("requirement row count")
                    + 1
            );
            fallback_omission(
                &mut coverage["requirement_omissions"],
                &mut requirement_omission,
                "coverage.requirement_rows",
            );
            fallback_omission(
                &mut value["diagnostics"]["omissions"],
                &mut requirement_diagnostic_omission,
                "coverage.requirement_rows",
            );
            mark_report_limit(&mut value["diagnostics"]);
            continue;
        }
        // Every remaining field belongs to the admitted mandatory envelope.
        let text = serde_json::to_string(&value).expect("mandatory author DTO");
        assert!(text.len() <= limit, "author mandatory reservation");
        return text;
    }
}
fn mark_report_limit(diagnostics: &mut Value) {
    diagnostics["truncated"] = json!(true);
    let reasons = diagnostics["reasons"]
        .as_array_mut()
        .expect("diagnostic reasons");
    if !reasons.contains(&json!("report_limit")) {
        reasons.push(json!("report_limit"));
    }
}
fn fallback_omission(omissions: &mut Value, index: &mut Option<usize>, field: &'static str) {
    let omissions = omissions.as_array_mut().expect("omissions");
    if let Some(index) = *index {
        let count = omissions[index]["omitted_count"]
            .as_u64()
            .expect("fallback count");
        omissions[index]["omitted_count"] = json!(count + 1);
    } else {
        *index = Some(omissions.len());
        omissions.push(json!({"field":field,"reason":"report_limit","omitted_count":1}));
    }
}

/// Headers live beside the already admitted DTO graphs, without new heap owners.
pub(super) fn owner_bytes() -> usize {
    2 * std::mem::size_of::<Projection>() + std::mem::size_of::<DiagnosticProjection>()
}

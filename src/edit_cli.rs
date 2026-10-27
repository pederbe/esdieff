//! Explicit authored-quality editing follows ADR 0034, Authored-quality editing with source
//! substitution.
use crate::{
    cli_style::ColorMode,
    document_cli::{self, Operation, OperationResult},
};
use esdieff::{Assessment, Catalogue, EditRequest, EditResult, HumanStyle, Outcome};
use std::ffi::OsString;

pub(super) fn run(args: &[OsString], color: &mut ColorMode) -> Result<u8, (u8, String)> {
    document_cli::run::<EditRequest>(args, color)
}
impl Operation for EditRequest {
    const NAME: &'static str = "edit";
    // Editing substitutes the original source, so its inputs must keep their identity.
    const UNCHANGED_INPUTS: bool = true;
    type Result = EditResult;
    fn from_plan(catalogue: Catalogue, plan: &[u8]) -> Result<Self, &'static str> {
        EditRequest::from_plan(catalogue, plan)
    }
    fn strict_warnings(self, strict: bool) -> Self {
        EditRequest::strict_warnings(self, strict)
    }
    fn checking(self, assessment: Assessment) -> Result<Self, &'static str> {
        EditRequest::checking(self, assessment)
    }
    fn perform(self) -> Result<EditResult, &'static str> {
        self.edit()
    }
}
impl OperationResult for EditResult {
    fn document_deliverable(&self) -> bool {
        EditResult::document_deliverable(self)
    }
    fn generated_document(&self) -> Option<&[u8]> {
        EditResult::generated_document(self)
    }
    fn to_json(&self) -> String {
        EditResult::to_json(self)
    }
    fn to_human_with_style(&self, style: HumanStyle) -> String {
        EditResult::to_human_with_style(self, style)
    }
    fn outcome(&self) -> &Outcome {
        EditResult::outcome(self)
    }
}

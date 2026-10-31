//! Explicit sdfThing authoring follows ADR 0017, Reference-preserving sdfThing authoring.
use crate::{
    cli_style::ColorMode,
    document_cli::{self, Operation, OperationResult},
};
use esdieff::{Assessment, AuthorRequest, AuthorResult, Catalogue, HumanStyle, Outcome};
use std::ffi::OsString;

pub(super) fn run(args: &[OsString], color: &mut ColorMode) -> Result<u8, (u8, String)> {
    document_cli::run::<AuthorRequest>(args, color)
}
impl Operation for AuthorRequest {
    const NAME: &'static str = "author";
    const UNCHANGED_INPUTS: bool = false;
    type Result = AuthorResult;
    fn from_plan(catalogue: Catalogue, plan: &[u8]) -> Result<Self, &'static str> {
        AuthorRequest::from_plan(catalogue, plan)
    }
    fn strict_warnings(self, strict: bool) -> Self {
        AuthorRequest::strict_warnings(self, strict)
    }
    fn checking(self, assessment: Assessment) -> Result<Self, &'static str> {
        AuthorRequest::checking(self, assessment)
    }
    fn perform(self) -> Result<AuthorResult, &'static str> {
        self.author()
    }
}
impl OperationResult for AuthorResult {
    fn document_deliverable(&self) -> bool {
        AuthorResult::document_deliverable(self)
    }
    fn generated_document(&self) -> Option<&[u8]> {
        AuthorResult::generated_document(self)
    }
    fn to_json(&self) -> String {
        AuthorResult::to_json(self)
    }
    fn to_human_with_style(&self, style: HumanStyle) -> String {
        AuthorResult::to_human_with_style(self, style)
    }
    fn outcome(&self) -> &Outcome {
        AuthorResult::outcome(self)
    }
}

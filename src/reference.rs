//! Selected effective definitions as a bounded Markdown reference, following ADR 0050.
mod render;
use crate::allowance::OperationBudget;
use crate::{Catalogue, Class, Disposition, EffectiveRequest, SourceId};

/// Select one effective Thing or Object from an explicitly supplied document.
/// Use navigation additionally admits other named definitions.
pub struct ReferenceRequest<'a> {
    document: SourceId,
    pointer: &'a str,
    uses: bool,
}
impl<'a> ReferenceRequest<'a> {
    pub fn new(document: SourceId, pointer: &'a str) -> Self {
        Self {
            document,
            pointer,
            uses: false,
        }
    }
    /// Include effective uses of the selected authored definition across supplied models.
    /// This also permits selecting named definitions other than Things and Objects.
    pub fn with_uses(mut self) -> Self {
        self.uses = true;
        self
    }
}

/// A retained processing condition. Documentation itself performs no model assessment.
#[derive(Clone, Debug)]
pub struct ReferenceCondition {
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
    pub(crate) definite: bool,
}
impl ReferenceCondition {
    pub fn code(&self) -> &'static str {
        self.code
    }
    pub fn message(&self) -> &'static str {
        self.message
    }
    pub fn definite(&self) -> bool {
        self.definite
    }
}

/// Presentation losses, separate from unsupported meaning or unavailable examples.
#[derive(Clone, Debug, Default)]
pub struct ReferenceOmissions {
    pub(crate) entries: usize,
    pub(crate) bytes: usize,
    pub(crate) pointers: usize,
    pub(crate) conditions: usize,
    pub(crate) unknown_remainder: bool,
}
impl ReferenceOmissions {
    pub fn entries(&self) -> usize {
        self.entries
    }
    /// Number of optional rows/sections omitted for the byte allowance.
    pub fn byte_limited(&self) -> usize {
        self.bytes
    }
    pub fn pointers(&self) -> usize {
        self.pointers
    }
    pub fn conditions(&self) -> usize {
        self.conditions
    }
    pub fn unknown_remainder(&self) -> bool {
        self.unknown_remainder
    }
    fn any(&self) -> bool {
        self.entries + self.bytes + self.pointers + self.conditions > 0 || self.unknown_remainder
    }
}

/// Owns the rendered document and derived outcome, borrowing its immutable source catalogue.
pub struct ReferenceResult<'a> {
    catalogue: &'a Catalogue,
    document: SourceId,
    markdown: String,
    complete: bool,
    failed: bool,
    conditions: Vec<ReferenceCondition>,
    omissions: ReferenceOmissions,
}
impl ReferenceResult<'_> {
    pub fn catalogue(&self) -> &Catalogue {
        self.catalogue
    }
    pub fn document(&self) -> SourceId {
        self.document
    }
    /// One UTF-8 Markdown document with a final newline and no terminal styling.
    pub fn to_markdown(&self) -> &str {
        &self.markdown
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
    pub fn disposition(&self) -> Disposition {
        if self.failed {
            Disposition::Failed
        } else if self.complete {
            Disposition::Passed
        } else {
            Disposition::Undetermined
        }
    }
    pub fn conditions(&self) -> impl ExactSizeIterator<Item = &ReferenceCondition> {
        self.conditions.iter()
    }
    pub fn omissions(&self) -> &ReferenceOmissions {
        &self.omissions
    }
}
impl Catalogue {
    /// Document one effective definition. An established absent or wrong-class selector is
    /// a request error. Unavailable composition produces a usable domain result instead.
    pub fn reference(
        &self,
        request: ReferenceRequest<'_>,
    ) -> Result<ReferenceResult<'_>, &'static str> {
        let source = self
            .source(request.document)
            .ok_or("unknown document selector")?;
        if !source.role().is_model() {
            return Err("sample source cannot be a model root");
        }
        if request.pointer.len() > self.limits.pointer_bytes.min(8192) {
            return Err("invalid or oversized JSON pointer selector");
        }
        crate::json::validate_pointer(request.pointer)?;
        // Reserve mandatory identity, status, every supplied attempt and omission counts
        // before composition. Even a failed selection retains that envelope.
        let envelope = render::Envelope::new(self, &request)?;
        let mut budget = OperationBudget::new(&self.limits);
        budget.allocation -= envelope.allocation_reserve;
        let (model, required) = self.compose_gated_required(
            EffectiveRequest::new(request.document).pointer(request.pointer),
            &mut budget,
        )?;
        if model.selected.is_some_and(|node| {
            model.nodes[node].role.is_some_and(|role| {
                if request.uses {
                    !matches!(role, crate::role::Role::Definition(_))
                } else {
                    !matches!(
                        role,
                        crate::role::Role::Definition(Class::Thing | Class::Object)
                    )
                }
            })
        }) {
            return Err(
                "reference pointer must select an sdfThing or sdfObject, or a named definition with --uses",
            );
        }
        render::render(self, request, envelope, &model, required.as_ref(), budget)
    }
}

//! Request origins specialize existing generated-source explanations, following ADR 0017.
use super::*;
use crate::Field;
use crate::document_request::RequestLocation;

/// A decoded coordinate in the owned recipe, distinct from every SDF source identity.
pub struct AuthorRequestLocation<'a>(RequestLocation<'a>);
impl<'a> AuthorRequestLocation<'a> {
    pub(crate) fn from_report(location: &'a serde_json::Value) -> Self {
        Self(RequestLocation::from_report(location))
    }
    pub fn key(&self) -> &str {
        self.0.key()
    }
    pub fn pointer(&self) -> Field<&str> {
        self.0.pointer()
    }
    /// Original textual-plan bytes. Typed requests have no original textual span.
    pub fn span(&self) -> Option<std::ops::Range<u32>> {
        self.0.span()
    }
}

impl AuthorResult {
    /// Map a nearest generated-source contributor to its original recipe coordinate.
    /// Reusable-source contributors are absent; a foreign catalogue view is unavailable.
    pub fn request_origin(&self, source: JsonView<'_>) -> Field<AuthorRequestLocation<'_>> {
        let mut work = self.catalogue().limits.work;
        let mut bytes = self.catalogue().limits.pointer_total;
        self.request_origin_with_budget(source, &mut work, &mut bytes)
    }
    pub(super) fn request_origin_with_budget(
        &self,
        source: JsonView<'_>,
        work: &mut usize,
        bytes: &mut usize,
    ) -> Field<AuthorRequestLocation<'_>> {
        let Some(owned) = self.catalogue().source(source.source_id()) else {
            return Field::Unavailable("source belongs to another catalogue");
        };
        if !std::ptr::eq(owned, source.source) {
            return Field::Unavailable("source belongs to another catalogue");
        }
        if source.source_id() != self.generated {
            return Field::Absent;
        }
        let pointer = match source.pointer(self.catalogue().limits.pointer_bytes as u32) {
            Field::Present(pointer) => pointer,
            Field::Unavailable(reason) => return Field::Unavailable(reason),
            Field::Absent => return Field::Absent,
        };
        let name_work = self
            .request
            .objects
            .iter()
            .map(|object| object.name.len() * 4 + 1)
            .sum::<usize>();
        if !crate::allowance::charge(work, pointer.len() * 8 + name_work + 512) {
            return Field::Unavailable("author request origin work limit");
        }
        let Some(logical) = logical_pointer(&self.request, &pointer) else {
            return Field::Absent;
        };
        if logical.len() > self.catalogue().limits.pointer_bytes {
            return Field::Unavailable("author request location limit");
        }
        if !crate::allowance::charge(bytes, logical.len()) {
            return Field::Unavailable("author request location limit");
        }
        let located = self
            .request
            .plan
            .as_ref()
            .and_then(|plan| plan.json().present())
            .and_then(|root| {
                crate::report::select_with_budget(root, &logical, work)
                    .ok()
                    .flatten()
            });
        Field::Present(AuthorRequestLocation(RequestLocation::new(
            &self.request.key,
            logical,
            located,
        )))
    }
}

fn logical_pointer(request: &AuthorRequest, pointer: &str) -> Option<String> {
    if pointer.is_empty() {
        return Some(String::new());
    }
    if let Some(suffix) = pointer.strip_prefix("/namespace/") {
        return Some(format!("/namespace/{suffix}"));
    }
    if pointer == "/defaultNamespace" {
        return Some("/default_namespace".to_owned());
    }
    if pointer == "/info" || pointer.starts_with("/info/") {
        return Some(pointer.to_owned());
    }
    let thing = format!("/sdfThing/{}", escape_pointer(&request.thing));
    let suffix = pointer.strip_prefix(&thing)?;
    if suffix.is_empty() {
        return Some("/thing_name".to_owned());
    }
    if let Some(objects) = suffix.strip_prefix("/sdfObject/") {
        for (index, object) in request.objects.iter().enumerate() {
            let escaped = escape_pointer(&object.name);
            let Some(tail) = objects.strip_prefix(&escaped) else {
                continue;
            };
            if tail.is_empty() {
                return Some(format!("/objects/{index}"));
            }
            if tail == "/sdfRef" {
                return Some(format!("/objects/{index}/pointer"));
            }
            if tail.starts_with('/') {
                return Some(format!("/objects/{index}/refinement{tail}"));
            }
        }
        return None;
    }
    if suffix.starts_with('/')
        && request.qualities.is_some()
        && !matches!(suffix, "/sdfObject" | "/sdfRequired")
        && !suffix.starts_with("/sdfRequired/")
    {
        return Some(format!("/thing_qualities{suffix}"));
    }
    None
}

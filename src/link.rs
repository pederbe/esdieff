//! Recorded draft policy for `sdfType: "link"`, following ADR 0060: Link sdfType draft support.
use crate::allowance::{OperationBudget, charge};
use crate::{EffectiveJson, EffectiveModel, Field};

pub(crate) const DRAFT: &str = "draft-ietf-asdf-sdftype-link-02";
pub(crate) const SHA256: &str = "27c42209d743d020a26c6d5a35b0c70b80e07beeb149db674081c9809929c95d";

/// The recorded draft support and its limits.
pub(crate) fn support() -> serde_json::Value {
    serde_json::json!({
        "id":DRAFT,
        "sha256":SHA256,
        "scope":"Link checking, inspection, selected-value checking, generation, trace replay and client-interaction revision comparison",
        "limitations":["link meaning requires type object", "attribute value case not assessed", "links are not followed"]
    })
}

/// The `baseline.extensions` rows for the recorded drafts a report relied on, or `None`.
pub(crate) fn extensions(context: bool, link: bool) -> Option<serde_json::Value> {
    let mut rows = Vec::new();
    if context {
        rows.extend(
            crate::context::support()
                .as_array()
                .expect("Context support rows")
                .iter()
                .cloned(),
        );
    }
    if link {
        rows.push(support());
    }
    (!rows.is_empty()).then_some(serde_json::Value::Array(rows))
}

/// Whether `json` establishes link meaning: `sdfType: "link"` with `type: "object"` and
/// neither `format` nor `contentFormat`. Other uses of `link` leave its meaning unavailable.
pub(crate) fn established<'a>(json: EffectiveJson<'a>, budget: &mut OperationBudget) -> bool {
    if !charge(&mut budget.work, 64) {
        return false;
    }
    fn text<'a>(field: Field<EffectiveJson<'a>>) -> Option<&'a str> {
        field.present().and_then(|value| value.text().present())
    }
    text(json.lookup("sdfType", budget)) == Some("link")
        && text(json.lookup("type", budget)) == Some("object")
        && matches!(json.lookup("format", budget), Field::Absent)
        && matches!(json.lookup("contentFormat", budget), Field::Absent)
}

/// Section 2 requires the lowercase form of the case-insensitive RFC 8288 attribute names.
pub(crate) fn member_name_admitted(name: &str) -> bool {
    !name.bytes().any(|byte| byte.is_ascii_uppercase())
}

/// Whether any present effective description in `model` names the link convention.
pub(crate) fn used(model: &EffectiveModel<'_>) -> bool {
    (0..model.nodes.len()).any(|node| {
        let json = EffectiveJson { model, node };
        let n = json.n();
        n.key == Some("sdfType") && !n.deleted && json.text().present() == Some("link")
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn member_names_must_be_lowercase() {
        assert!(super::member_name_admitted("href"));
        assert!(super::member_name_admitted("object-instance-id"));
        assert!(super::member_name_admitted("_other"));
        assert!(!super::member_name_admitted("Href"));
        assert!(!super::member_name_admitted("hrefLang"));
        // Only ASCII letters have the RFC 8288 case-insensitive comparison.
        assert!(super::member_name_admitted("ä"));
    }
}

//! Role-aware Full model admission follows ADR0032: Formal model admission.
use super::*;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Literal {
    Base,
    Extension,
    Unavailable,
}

pub(super) fn literal(json: CheckView<'_, '_>, work: &mut usize) -> Literal {
    if !charge_exact(work, 1) {
        return Literal::Unavailable;
    }
    match json.kind() {
        JsonKind::Number => decoded_number(json),
        JsonKind::String => {
            if matches!(json.text(), Field::Present(_)) {
                Literal::Base
            } else {
                Literal::Unavailable
            }
        }
        JsonKind::Boolean | JsonKind::Null => Literal::Base,
        JsonKind::Map => {
            for child in json.children() {
                let name = child.member_name().present();
                if !charge_exact(work, name.map_or(1, |name| name.len() + 1)) || name.is_none() {
                    return Literal::Unavailable;
                }
                // The map alternative explicitly gives its values `any`.
            }
            Literal::Base
        }
        JsonKind::Array => {
            let mut kind = None;
            for child in json.children() {
                if !charge_exact(work, 1) {
                    return Literal::Unavailable;
                }
                let current = child.kind();
                if !matches!(
                    current,
                    JsonKind::Number | JsonKind::String | JsonKind::Boolean
                ) || kind.is_some_and(|previous| previous != current)
                {
                    return Literal::Extension;
                }
                kind = Some(current);
                let unavailable = match current {
                    JsonKind::Number => decoded_number(child) == Literal::Unavailable,
                    JsonKind::String => !matches!(child.text(), Field::Present(_)),
                    _ => false,
                };
                if unavailable {
                    return Literal::Unavailable;
                }
            }
            Literal::Base
        }
    }
}

/// Every decoded JSON number belongs to the imported `number`. Decoding limits
/// leave membership unavailable rather than excluded.
fn decoded_number(json: CheckView<'_, '_>) -> Literal {
    if matches!(json.number(), Field::Present(_)) {
        Literal::Base
    } else {
        Literal::Unavailable
    }
}

impl<'a, 'view> Checker<'a, 'view, '_> {
    pub(in crate::structure) fn formal_field(
        &mut self,
        description: CheckView<'a, 'view>,
        value: CheckView<'a, 'view>,
        key: &str,
        role: Role,
        object: bool,
    ) {
        if !self.result.coverage.full_requested || !role.supports(key, object) {
            return;
        }
        let CheckView::Effective { .. } = value else {
            return;
        };
        let grouping = matches!(role, Role::Definition(Class::Thing | Class::Object))
            && matches!(key, "minItems" | "maxItems");
        let owner = if grouping || role.count_parameter(key) {
            Requirement::M037
        } else if role.numeric_parameter(key) {
            Requirement::M036
        } else if role.full_data() && matches!(key, "const" | "default") {
            Requirement::M035
        } else {
            return;
        };
        let unavailable = |value: CheckView<'_, '_>| matches!(value,CheckView::Effective{model,node} if !model.clear_feature || model.nodes[node].unavailable.is_some() || model.nodes[node].donor_unavailable.is_some() || model.nodes[node].meaning_unavailable.is_some());
        if unavailable(description) || unavailable(value) {
            self.record(value, Rule::FormalUnavailable(owner));
            if grouping {
                self.result
                    .coverage
                    .observe(Requirement::M032, Undetermined);
            }
            return;
        }
        if owner == Requirement::M035 {
            let result = literal(value, &mut self.budget.work);
            self.result.coverage.observe(
                owner,
                if result == Literal::Unavailable {
                    Undetermined
                } else {
                    Assessed
                },
            );
            if result == Literal::Unavailable {
                self.record(value, Rule::FormalUnavailable(owner));
            }
            if result == Literal::Extension {
                let CheckView::Effective { model, node } = value else {
                    unreachable!()
                };
                let Some(needed) = (crate::EffectiveJson { model, node })
                    .model_meaning_needed(&mut self.budget.work)
                else {
                    self.record(value, Rule::WorkLimit);
                    return;
                };
                if needed {
                    self.result
                        .coverage
                        .observe(Requirement::M028, Undetermined);
                    if key == "const" {
                        self.result
                            .coverage
                            .observe(Requirement::M043, Undetermined);
                    }
                }
            }
            return;
        }
        let verdict = match value.number() {
            Field::Present(_) if owner == Requirement::M036 => crate::formal::Verdict::Admitted,
            Field::Present(number) => {
                let numbers = self
                    .number_work
                    .as_mut()
                    .expect("Full original numeric allowance");
                crate::formal::uint(number, &mut self.budget.work, numbers)
            }
            _ if value.kind() == JsonKind::Number => crate::formal::Verdict::Unavailable,
            _ => crate::formal::Verdict::Excluded,
        };
        self.result.coverage.observe(
            owner,
            if verdict == crate::formal::Verdict::Unavailable {
                Undetermined
            } else {
                Assessed
            },
        );
        if grouping {
            self.result.coverage.observe(
                Requirement::M032,
                if verdict == crate::formal::Verdict::Admitted {
                    Assessed
                } else {
                    Undetermined
                },
            );
            if verdict == crate::formal::Verdict::Excluded {
                self.result
                    .coverage
                    .observe(Requirement::M028, Undetermined);
            }
        }
        match verdict {
            crate::formal::Verdict::Unavailable => {
                self.record(value, Rule::FormalUnavailable(owner))
            }
            crate::formal::Verdict::Excluded
                if !grouping
                    && value
                        .number()
                        .present()
                        .is_some_and(|number| number.is_integer() && !number.sign().is_lt()) =>
            {
                self.record(value, Rule::FormalUint);
            }
            _ => {}
        }
    }
}

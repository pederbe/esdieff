//! Fixed RFC 9880 model membership and borrowed requirement descriptions.
//! Coverage follows ADR 0011: Machine-readable report contracts.
use crate::AssessmentStatus;
use serde_json::{Value, json};
pub(crate) const INVENTORY: &str = "rfc9880.model/1";
pub(crate) const FULL_INVENTORY: &str = "esdieff.full-model/1";
pub(crate) const FULL_AREAS: [&str; 13] = [
    "full.document-syntax/1",
    "full.model-roles/1",
    "full.metadata/1",
    "full.namespace-reference/1",
    "full.composition-requiredness/1",
    "full.critical-extensions/1",
    "full.data-grammar/1",
    "full.formal-numbers/1",
    "full.scalar-meaning/1",
    "full.pattern-format/1",
    "full.compound-meaning/1",
    "full.special-type-unit/1",
    "advisory-four/1",
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum Requirement {
    M001,
    M002,
    M003,
    M004,
    M005,
    M006,
    M007,
    M008,
    M009,
    M010,
    M011,
    M012,
    M013,
    M014,
    M015,
    M016,
    M017,
    M018,
    M019,
    M020,
    M021,
    M022,
    M023,
    M024,
    M025,
    M026,
    M027,
    M028,
    M029,
    M030,
    M031,
    M032,
    M033,
    M034,
    M035,
    M036,
    M037,
    M038,
    M039,
    M040,
    M041,
    M042,
    M043,
    M044,
    M045,
    M046,
    M047,
    M048,
    M049,
    M050,
    M051,
    M052,
    M053,
    M054,
}
#[derive(Clone)]
pub(crate) struct State {
    pub(crate) initial: [AssessmentStatus; 8],
    pub(crate) requirements: [AssessmentStatus; 54],
    pub(crate) full_requested: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            initial: [AssessmentStatus::Unassessed; 8],
            requirements: [AssessmentStatus::Unassessed; 54],
            full_requested: false,
        }
    }
}
impl State {
    pub(crate) fn start(full: bool) -> Self {
        Self {
            initial: [AssessmentStatus::NotApplicable; 8],
            requirements: [AssessmentStatus::NotApplicable; 54],
            full_requested: full,
        }
    }
    pub(crate) fn initial(&mut self, area: usize, status: AssessmentStatus) {
        self.initial[area] = join(self.initial[area], status);
    }
    pub(crate) fn observe(&mut self, requirement: Requirement, status: AssessmentStatus) {
        if self.full_requested {
            let state = &mut self.requirements[requirement as usize];
            *state = join(*state, status);
        }
    }
    pub(crate) fn area_status(
        &self,
        area: &str,
        advice: AssessmentStatus,
    ) -> Option<AssessmentStatus> {
        if !FULL_AREAS.contains(&area) {
            return None;
        }
        if area == "advisory-four/1" {
            return Some(advice);
        }
        let mut status = AssessmentStatus::NotApplicable;
        for (descriptor, member) in DESCRIPTORS.iter().zip(self.requirements) {
            if descriptor.area == area {
                status = join(status, member);
            }
        }
        Some(status)
    }
}
pub(crate) fn join(left: AssessmentStatus, right: AssessmentStatus) -> AssessmentStatus {
    use AssessmentStatus::*;
    match (left, right) {
        (Undetermined, _) | (_, Undetermined) => Undetermined,
        (Unassessed, _) | (_, Unassessed) => Unassessed,
        (Assessed, _) | (_, Assessed) => Assessed,
        _ => NotApplicable,
    }
}
/// A cited member of the immutable RFC 9880 model requirement inventory.
/// The planned check describes capability and scope; query the report for its status.
pub struct RequirementDescriptor {
    pub(crate) id: &'static str,
    pub(crate) citations: &'static str,
    pub(crate) area: &'static str,
    pub(crate) plan: &'static str,
}
impl RequirementDescriptor {
    pub fn id(&self) -> &'static str {
        self.id
    }
    pub fn citations(&self) -> &'static str {
        self.citations
    }
    pub fn assessment_area(&self) -> &'static str {
        self.area
    }
    pub fn planned_check(&self) -> &'static str {
        self.plan
    }
}
pub(crate) type Descriptor = RequirementDescriptor;
pub(crate) static DESCRIPTORS: [Descriptor; 54] = [
    Descriptor {
        id: "rfc9880.model.M001/1",
        citations: "RFC 9880 1.2.3;3;7.1;A sdf-syntax",
        area: "full.document-syntax/1",
        plan: "Decode complete supplied JSON while retaining acquisition failures; transport delivery duties are outside model assessment.",
    },
    Descriptor {
        id: "rfc9880.model.M002/1",
        citations: "RFC 9880 2.2;2.3.1;3.3-3.4;5.1-5.5;6.3;A named/paedataqualities/thing/object/action/event",
        area: "full.document-syntax/1",
        plan: "Traverse named definitions and all admitted data-description containers in their actual roles.",
    },
    Descriptor {
        id: "rfc9880.model.M003/1",
        citations: "RFC 9880 1.2.3;2.2.2;2.2.5;3.3;4.5",
        area: "full.model-roles/1",
        plan: "Establish declaration classes in the resolved model, including copied declarations at their importing coordinates.",
    },
    Descriptor {
        id: "rfc9880.model.M004/1",
        citations: "RFC 9880 2.3.2-2.3.3",
        area: "full.document-syntax/1",
        plan: "Check case-sensitive definition names and reserved colon use.",
    },
    Descriptor {
        id: "rfc9880.model.M005/1",
        citations: "RFC 9880 2.3.3;A quality-name",
        area: "full.document-syntax/1",
        plan: "Check ASCII unqualified and qualified spelling in actual Quality Name positions; preserve admitted unfamiliar vocabulary and opaque extension values.",
    },
    Descriptor {
        id: "rfc9880.model.M006/1",
        citations: "RFC 9880 2.3.2;4.1-4.3",
        area: "full.namespace-reference/1",
        plan: "Resolve encoded reference names through the established namespace and pointer context.",
    },
    Descriptor {
        id: "rfc9880.model.M007/1",
        citations: "RFC 9880 3",
        area: "full.document-syntax/1",
        plan: "Check the document envelope and supported top-level definition groups.",
    },
    Descriptor {
        id: "rfc9880.model.M008/1",
        citations: "RFC 9880 3.3 versus A sdf-syntax",
        area: "full.document-syntax/1",
        plan: "Establish a populated definition group; empty documents retain the unresolved prose and CDDL distinction.",
    },
    Descriptor {
        id: "rfc9880.model.M009/1",
        citations: "RFC 9880 3.1;A sdfinfo",
        area: "full.metadata/1",
        plan: "Check the Information Block and its general string-valued metadata.",
    },
    Descriptor {
        id: "rfc9880.model.M010/1",
        citations: "RFC 9880 3.1;4.6",
        area: "full.metadata/1",
        plan: "Expose authored Information metadata and its absence without inserting values or imposing lifecycle advice.",
    },
    Descriptor {
        id: "rfc9880.model.M011/1",
        citations: "RFC 9880 2.3.3;4.6 Table3;A optional-comment",
        area: "full.metadata/1",
        plan: "Preserve the descriptive comment as metadata without treating its text as a semantic command.",
    },
    Descriptor {
        id: "rfc9880.model.M012/1",
        citations: "RFC 9880 3.1;A modified-date-time/rfc3339z",
        area: "full.metadata/1",
        plan: "Check modified dates against the supported calendar, clock and recorded temporal knowledge.",
    },
    Descriptor {
        id: "rfc9880.model.M013/1",
        citations: "RFC 9880 3.1;A features",
        area: "full.critical-extensions/1",
        plan: "Establish the required feature declaration before interpreting model qualities.",
    },
    Descriptor {
        id: "rfc9880.model.M014/1",
        citations: "RFC 9880 3.2;A sdf-syntax",
        area: "full.namespace-reference/1",
        plan: "Check namespace map shape and URI-valued bindings.",
    },
    Descriptor {
        id: "rfc9880.model.M015/1",
        citations: "RFC 9880 3.2;4",
        area: "full.namespace-reference/1",
        plan: "Check that the declared default namespace has an established binding.",
    },
    Descriptor {
        id: "rfc9880.model.M016/1",
        citations: "RFC 9880 1.2.4;3.2;4.1-4.2;6.1",
        area: "full.namespace-reference/1",
        plan: "Resolve local and cross-document references while retaining missing, ambiguous and unavailable contributors.",
    },
    Descriptor {
        id: "rfc9880.model.M017/1",
        citations: "RFC 9880 4.3",
        area: "full.namespace-reference/1",
        plan: "Establish the target role of each model reference.",
    },
    Descriptor {
        id: "rfc9880.model.M018/1",
        citations: "RFC 9880 4.3;4.6;A sdf-pointer/pointer-list/referenceable-name",
        area: "full.namespace-reference/1",
        plan: "Check reference syntax and referenceable positions in Full and Direct data contexts.",
    },
    Descriptor {
        id: "rfc9880.model.M019/1",
        citations: "RFC 9880 4.4-4.4.1;6.2.1",
        area: "full.composition-requiredness/1",
        plan: "Compose inherited qualities and local overrides, relocating internal copied references to the importing definition.",
    },
    Descriptor {
        id: "rfc9880.model.M020/1",
        citations: "RFC 9880 4.5;4.6",
        area: "full.composition-requiredness/1",
        plan: "Resolve grouping-required declarations in the resolved model, relocating copied pointers by coordinate under the existing requiredness policy.",
    },
    Descriptor {
        id: "rfc9880.model.M021/1",
        citations: "RFC 9880 3.1;4.6;A sdfinfo/commonqualities/jso-items/optional-comment",
        area: "full.metadata/1",
        plan: "Check descriptive metadata string shape in each admitted Full or Direct position.",
    },
    Descriptor {
        id: "rfc9880.model.M022/1",
        citations: "RFC 9880 4.7 Table4;A dataqualities",
        area: "full.data-grammar/1",
        plan: "Check common data-quality admission in its actual data-description role.",
    },
    Descriptor {
        id: "rfc9880.model.M023/1",
        citations: "RFC 9880 4.7 note1;7.3-7.4",
        area: "full.special-type-unit/1",
        plan: "Check unit spelling and recorded registry guidance under the selected knowledge policy.",
    },
    Descriptor {
        id: "rfc9880.model.M024/1",
        citations: "RFC 9880 4.7 note2;RFC9193 §6",
        area: "full.pattern-format/1",
        plan: "Establish recorded numeric tuples and supported JSON/CBOR/plain-text parameters with ordered codings; other meaning and missing evidence remain scoped.",
    },
    Descriptor {
        id: "rfc9880.model.M025/1",
        citations: "RFC 9880 4.7.1;A sdftype-name",
        area: "full.critical-extensions/1",
        plan: "Recognize the two base special-type selectors; extended-name admission remains unfinished without a closed extension vocabulary.",
    },
    Descriptor {
        id: "rfc9880.model.M026/1",
        citations: "RFC 9880 4.7.1 Table5",
        area: "full.special-type-unit/1",
        plan: "Establish the known byte-sequence model interpretation without a value; decoded-byte length remains separately unfinished.",
    },
    Descriptor {
        id: "rfc9880.model.M027/1",
        citations: "RFC 9880 4.7.1 Table5",
        area: "full.special-type-unit/1",
        plan: "Establish the known POSIX-time model interpretation without assigning an actual value or leap-instant meaning.",
    },
    Descriptor {
        id: "rfc9880.model.M028/1",
        citations: "RFC 9880 2.3.3;4.7.1;7.5.2-7.5.3;A extension sockets",
        area: "full.critical-extensions/1",
        plan: "Retain unknown extension meanings as unavailable rather than inferring universal invalidity.",
    },
    Descriptor {
        id: "rfc9880.model.M029/1",
        citations: "RFC 9880 4.7 Table4;A dataqualities",
        area: "full.scalar-meaning/1",
        plan: "Check nullable shape and expose its established domain default.",
    },
    Descriptor {
        id: "rfc9880.model.M030/1",
        citations: "RFC 9880 4.7.2;A optional-choice/jsonschema/jso-items",
        area: "full.composition-requiredness/1",
        plan: "Check choice and enumeration syntax and establish inherited alternative scope.",
    },
    Descriptor {
        id: "rfc9880.model.M031/1",
        citations: "RFC 9880 5.2;A propertyqualities",
        area: "full.model-roles/1",
        plan: "Check Property flags and expose their established domain defaults.",
    },
    Descriptor {
        id: "rfc9880.model.M032/1",
        citations: "RFC 9880 2.2.1;6.3",
        area: "full.formal-numbers/1",
        plan: "Establish inclusive base count meaning for admitted uint values; framework fallback retains unavailable extension repetition meaning.",
    },
    Descriptor {
        id: "rfc9880.model.M033/1",
        citations: "RFC 9880 A EXTENSION-POINT and $$ quality sockets",
        area: "full.document-syntax/1",
        plan: "Check admitted extension sockets without claiming unsupported extension meaning.",
    },
    Descriptor {
        id: "rfc9880.model.M034/1",
        citations: "RFC 9880 A jsonschema/compound-type/jso-items",
        area: "full.data-grammar/1",
        plan: "Check base-type selector shape separately from model-level type capability.",
    },
    Descriptor {
        id: "rfc9880.model.M035/1",
        citations: "RFC 9880 A allowed-types;RFC8610 3.3/D/E",
        area: "full.data-grammar/1",
        plan: "Establish base or framework-extension literal admission; extension semantics and needed const meaning remain separately unfinished.",
    },
    Descriptor {
        id: "rfc9880.model.M036/1",
        citations: "RFC 9880 A jsonschema/jso-items number;RFC8610 3.3/D/E",
        area: "full.formal-numbers/1",
        plan: "Establish formal CDDL number membership for bound parameters from their decoded JSON numbers.",
    },
    Descriptor {
        id: "rfc9880.model.M037/1",
        citations: "RFC 9880 A jsonschema/jso-items/arraydefinitionqualities uint;RFC8610 3.3/D/E",
        area: "full.formal-numbers/1",
        plan: "Establish formal CDDL uint membership for counts; exact nonnegative integer decoding alone is insufficient.",
    },
    Descriptor {
        id: "rfc9880.model.M038/1",
        citations: "RFC 9880 A compound-type",
        area: "full.data-grammar/1",
        plan: "Check map-data properties and required syntax in the compound-type context.",
    },
    Descriptor {
        id: "rfc9880.model.M039/1",
        citations: "RFC 9880 A jsonschema/jso-items",
        area: "full.data-grammar/1",
        plan: "Check pattern, format and uniqueness parameter shapes in Full and Direct items.",
    },
    Descriptor {
        id: "rfc9880.model.M040/1",
        citations: "RFC 9880 A jsonschema items/jso-items",
        area: "full.data-grammar/1",
        plan: "Check the Direct items container and traverse its admitted Full descendants.",
    },
    Descriptor {
        id: "rfc9880.model.M041/1",
        citations: "RFC 9880 A jso-items/items-ext/itemtype-ext;4.4;C.4",
        area: "full.critical-extensions/1",
        plan: "Establish Direct-item extension meaning; preserved unsupported qualities remain unavailable.",
    },
    Descriptor {
        id: "rfc9880.model.M042/1",
        citations: "RFC 9880 C intro;C.1-5",
        area: "full.scalar-meaning/1",
        plan: "Establish ordinary scalar, array and map type meaning in final effective contexts; unavailable extended meanings remain scoped.",
    },
    Descriptor {
        id: "rfc9880.model.M043/1",
        citations: "RFC 9880 C intro",
        area: "full.scalar-meaning/1",
        plan: "Establish scalar and compound const meaning under exact contextual equality for the actual final value; unavailable abstract denotation and extension meanings remain scoped.",
    },
    Descriptor {
        id: "rfc9880.model.M044/1",
        citations: "RFC 9880 C intro",
        area: "full.scalar-meaning/1",
        plan: "Expose default values as optional descriptive metadata without executing them as required characteristics.",
    },
    Descriptor {
        id: "rfc9880.model.M045/1",
        citations: "RFC 9880 C.1",
        area: "full.scalar-meaning/1",
        plan: "Establish exact inclusive/exclusive bounds and positive-step meaning in an available ordinary effective context.",
    },
    Descriptor {
        id: "rfc9880.model.M046/1",
        citations: "RFC 9880 C.2",
        area: "full.scalar-meaning/1",
        plan: "Establish inclusive Unicode-scalar text-length meaning in an available ordinary effective context.",
    },
    Descriptor {
        id: "rfc9880.model.M047/1",
        citations: "RFC 9880 C.2",
        area: "full.pattern-format/1",
        plan: "Check effective pattern syntax within the supported bounded ECMAScript capability.",
    },
    Descriptor {
        id: "rfc9880.model.M048/1",
        citations: "RFC 9880 C.2",
        area: "full.pattern-format/1",
        plan: "Establish bounded Unicode unanchored model matching capability by compilation without a sample or satisfying witness.",
    },
    Descriptor {
        id: "rfc9880.model.M049/1",
        citations: "RFC 9880 C.2",
        area: "full.pattern-format/1",
        plan: "Establish known civil, URI, URI-reference and UUID model interpretations independently of actual value denotation.",
    },
    Descriptor {
        id: "rfc9880.model.M050/1",
        citations: "RFC 9880 C.4",
        area: "full.compound-meaning/1",
        plan: "Establish every final item description recursively in its actual role, independently of samples.",
    },
    Descriptor {
        id: "rfc9880.model.M051/1",
        citations: "RFC 9880 C.4",
        area: "full.compound-meaning/1",
        plan: "Establish inclusive exact mathematical counts separately from formal uint membership and satisfiability.",
    },
    Descriptor {
        id: "rfc9880.model.M052/1",
        citations: "RFC 9880 C.4",
        area: "full.compound-meaning/1",
        plan: "Establish ordinary contextual equality capability recursively across final alternatives; abstract gaps remain scoped.",
    },
    Descriptor {
        id: "rfc9880.model.M053/1",
        citations: "RFC 9880 C.5",
        area: "full.compound-meaning/1",
        plan: "Establish every final property description, including optional entries, under open map semantics.",
    },
    Descriptor {
        id: "rfc9880.model.M054/1",
        citations: "RFC 9880 C.5",
        area: "full.compound-meaning/1",
        plan: "Establish decoded mandatory map-entry names independently of property descriptions and grouping declarations.",
    },
];
/// Construct only after the caller admits row wire and simultaneous staging.
pub(crate) fn requirement_row(index: usize, status: AssessmentStatus) -> Value {
    let descriptor = &DESCRIPTORS[index];
    json!({"id":descriptor.id,"count_unit":"requirement","status":status.as_str(),
        "basis":{"kind":"rfc","citation":descriptor.citations},"coverage_owner":descriptor.area})
}

//! Supplied SDF catalogues, source inspection and explicitly scoped reports.
#![doc = include_str!("../docs/source-workflow.md")]
mod allowance;
mod assessment;
mod author;
mod budgeted;
mod civil;
mod comparison;
mod content_format;
mod context;
mod coordinate_scan;
mod document_request;
mod domain;
mod edit;
mod effective;
mod equality;
mod finding;
mod formal;
mod formats;
mod generation;
mod human;
mod inventory;
mod json;
mod json_emit;
mod limits;
mod modified;
mod numeric;
mod pattern;
mod proof;
mod recorded_extension;
mod registry;
mod report;
mod representative;
mod role;
mod sample;
mod sarif;
mod scalar;
mod source;
mod structure;
mod trace;
mod units;
mod uri;
mod value;
pub use author::{
    AuthorChecking, AuthorObject, AuthorRequest, AuthorRequestLocation, AuthorResult, Fidelity,
    Generation,
};
pub use comparison::{
    COMPARISON_CONTRACT, Classification, ComparisonRequest, ContentFormatReason, Difference,
    DifferenceAcknowledgement, DifferenceAcknowledgements, DifferenceKind, DifferenceSide,
    DifferenceSubject, Direction, Presence, Revision, RevisionComparison, UndeterminedReason,
    UnitReason, VersionWarning, Witness,
};
pub use domain::{
    BaseType, Characteristic, Class, DataDescription, DataDescriptions, DataRole, DataType,
    Definition, Definitions, EffectiveDataDescription, EffectiveDataDescriptions,
    EffectiveDefinition, EffectiveDefinitions, Lookup,
};
pub use edit::{
    Edit, EditChecking, EditOperation, EditRequest, EditRequestLocation, EditResult, EditTarget,
};
pub use effective::{
    AlternativeId, EffectiveAlternatives, EffectiveChildren, EffectiveJson, EffectiveLocation,
    EffectiveModel, EffectiveRequest, Explanation, OriginKind, RequiredDeclarationSet,
    RequiredDeclarationSets, RequiredDeclarations, RequiredDeclarationsState,
    RequiredReferenceForm, RequiredReferenceInspection, RequiredReferenceStatus,
    RequiredReferences, RequiredTarget, RequiredTargetCapability, RequiredTargets,
    RequirednessState,
};
pub use formats::FormatKnowledgeView;
pub use generation::{
    GENERATION_CONTRACT, GeneratedDescription, GeneratedValue, GenerationRequest, GenerationResult,
    GenerationState,
};
pub use human::HumanStyle;
pub use inventory::RequirementDescriptor;
pub use json::{Children, ExactNumber, Field, JsonKind, JsonView, NumberWork};
pub use limits::Limits;
pub use registry::{RecordedRegistryPolicy, RegistryKnowledgeView};
pub use report::{
    Assessment, AssessmentStatus, Basis, BasisKind, CheckRequest, Condition, ConditionDomain,
    Diagnostics, Disposition, EffectiveReportLocation, InspectRequest, Outcome, Report,
    ReportLocation, RequestedCoverage, Severity, SourceLocation, View,
};
pub use sample::{
    Admission, ContextFindings, ContextState, SampleBasisRef, SampleKind, SampleMeaning,
    SampleMeaningArea, SampleMeaningConclusion, SampleRequest, SampleResult,
};
pub use source::{Acquisition, Catalogue, Source, SourceId, SourceInput, SourceRole};
pub use trace::{
    TRACE_CONTRACT, TraceCondition, TraceEntry, TraceOutcome, TraceRequest, TraceResult,
};
pub use units::{UnitGuidance, UnitRegistrySourceEditionView};
pub use value::{
    JsonBuilder, JsonValue, JsonValueView, SampleChildren, SampleValueView, ValueChildren,
};

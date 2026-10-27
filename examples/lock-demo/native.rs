//! Native Model checking and Sample-value checking through the public Catalogue.
//! The Catalogue owns every SDF meaning, following ADR 0016: Selected samples with explicit
//! admission and coverage. Each report is reduced to a compact result and dropped at once.
use crate::scenario::{Command, Credential, LockState};
use crate::trace::{ActionResult, StateChanged};
use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, Disposition, Field, Report, ReportLocation,
    RequestedCoverage, SampleKind, SampleRequest, Severity, SourceId, SourceInput,
};
use std::{fmt, io::Write};

const MODEL_KEY: &str = "lock";
const STATE: &str = "/sdfObject/Lock/sdfProperty/state";
const AUTHORIZE: &str = "/sdfObject/Lock/sdfAction/authorize";
const STATE_CHANGED: &str = "/sdfObject/Lock/sdfEvent/stateChanged";
const RETAINED_ERRORS: usize = 4;

/// Fixed finite work for one run, following ADR 0009: Bounded processing and reporting.
/// Allowances are spent once and never replenished.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub model_checks: usize,
    pub sample_requests: usize,
    pub sample_bytes: usize,
    pub trace_bytes: usize,
}
impl Bounds {
    pub const LOCK_DEMO: Self = Self {
        model_checks: 2,
        sample_requests: 60,
        sample_bytes: 512,
        trace_bytes: 64 * 1024,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Role {
    Input,
    Output,
    Event,
    Reading,
}

/// Named sample identity. Step zero holds only the initial reading.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SampleId {
    pub step: u32,
    pub role: Role,
}
impl SampleId {
    pub fn new(step: u32, role: Role) -> Self {
        Self { step, role }
    }
    fn key(self) -> String {
        format!("step-{}-{}", self.step, self.role_name())
    }
    fn role_name(self) -> &'static str {
        match self.role {
            Role::Input => "input",
            Role::Output => "output",
            Role::Event => "event",
            Role::Reading => "reading",
        }
    }
}
impl fmt::Display for SampleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.step == 0 {
            write!(f, "initial {}", self.role_name())
        } else {
            write!(f, "step {} {}", self.step, self.role_name())
        }
    }
}

/// One interaction value, written as the whole selected sample value.
#[derive(Clone, Copy)]
pub enum SampleValue<'a> {
    Input(&'a Command),
    Output(ActionResult),
    Event(StateChanged),
    Reading(LockState),
}
impl SampleValue<'_> {
    pub fn role(&self) -> Role {
        match self {
            Self::Input(_) => Role::Input,
            Self::Output(_) => Role::Output,
            Self::Event(_) => Role::Event,
            Self::Reading(_) => Role::Reading,
        }
    }
    fn write_json(&self, out: &mut impl Write) -> std::io::Result<()> {
        match self {
            Self::Input(command) => {
                out.write_all(b"{\"credential\":")?;
                match command.credential {
                    Credential::Token(token) => text(out, token)?,
                    Credential::Null => out.write_all(b"null")?,
                }
                if let Some(operation) = command.operation {
                    out.write_all(b",\"operation\":")?;
                    text(out, operation.as_str())?;
                }
                out.write_all(b"}")
            }
            Self::Output(result) => {
                write!(out, "{{\"accepted\":{},\"state\":", result.accepted)?;
                text(out, result.state.as_str())?;
                out.write_all(b"}")
            }
            Self::Event(event) => {
                out.write_all(b"{\"state\":")?;
                text(out, event.state.as_str())?;
                write!(out, ",\"at\":\"{}\"}}", event.at)
            }
            Self::Reading(state) => text(out, state.as_str()),
        }
    }
}
fn text(out: &mut impl Write, value: &str) -> std::io::Result<()> {
    serde_json::to_writer(out, value).map_err(std::io::Error::other)
}

fn selector(role: Role) -> (SampleKind, &'static str) {
    match role {
        Role::Input => (SampleKind::ActionInput, AUTHORIZE),
        Role::Output => (SampleKind::ActionOutput, AUTHORIZE),
        Role::Event => (SampleKind::EventOutput, STATE_CHANGED),
        Role::Reading => (SampleKind::Property, STATE),
    }
}

/// Counts encoded bytes without storing them and stops at the limit.
struct Counter {
    len: usize,
    limit: usize,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.len = self
            .len
            .checked_add(bytes.len())
            .filter(|len| *len <= self.limit)
            .ok_or_else(|| std::io::Error::other("sample value limit"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Writes into already reserved storage and refuses to grow it.
struct Reserved<'a>(&'a mut Vec<u8>);
impl Write for Reserved<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.0.capacity() - self.0.len() {
            return Err(std::io::Error::other("reserved sample storage"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Requested coverage counts retained from a native report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Coverage {
    pub inventory: Option<String>,
    pub total: Option<u32>,
    pub assessed: u32,
    pub not_applicable: u32,
    pub unassessed: u32,
    pub undetermined: u32,
}
impl From<RequestedCoverage> for Coverage {
    fn from(coverage: RequestedCoverage) -> Self {
        Self {
            inventory: coverage.inventory().map(str::to_owned),
            total: coverage.total(),
            assessed: coverage.assessed(),
            not_applicable: coverage.not_applicable(),
            unassessed: coverage.unassessed(),
            undetermined: coverage.undetermined(),
        }
    }
}

/// An error condition's code, sample location and related model source locations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: String,
    pub sample_pointer: Option<String>,
    pub source_pointers: Vec<String>,
}

/// The stable semantic fields of one native sample report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeResult {
    pub kind: SampleKind,
    pub admission: Admission,
    pub disposition: Disposition,
    pub complete: bool,
    pub coverage: Coverage,
    pub errors: Vec<Diagnostic>,
}

/// A checked sample's identity and either its compact result or why no result is available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampleCheck {
    pub id: SampleId,
    /// The checked value's UTF-8 byte length.
    pub bytes: usize,
    pub native: Result<NativeResult, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ModelCase {
    InitialCatalogue,
    FullSelected,
}
impl fmt::Display for ModelCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InitialCatalogue => "Initial Catalogue check",
            Self::FullSelected => "Full Selected-model check",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelResult {
    pub assessment: Option<Assessment>,
    pub disposition: Disposition,
    pub complete: bool,
    pub errors: u32,
    pub warnings: u32,
    pub coverage: Coverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCheck {
    pub case: ModelCase,
    pub native: Result<ModelResult, String>,
}

/// Owns the request allowances and the compact results of one run.
pub struct NativeChecker<'m> {
    model: &'m [u8],
    bounds: Bounds,
    model_checks_remaining: usize,
    sample_requests_remaining: usize,
    checks: Vec<SampleCheck>,
}
impl<'m> NativeChecker<'m> {
    pub fn new(model: &'m [u8], bounds: Bounds) -> Self {
        Self {
            model,
            bounds,
            model_checks_remaining: bounds.model_checks,
            sample_requests_remaining: bounds.sample_requests,
            checks: Vec::with_capacity(bounds.sample_requests),
        }
    }

    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Run one strict Model check on a Catalogue holding only the model.
    pub fn check_model(&mut self, case: ModelCase) -> ModelCheck {
        let Some(remaining) = self.model_checks_remaining.checked_sub(1) else {
            let native = Err("Model check allowance exhausted".to_owned());
            return ModelCheck { case, native };
        };
        self.model_checks_remaining = remaining;
        let native = Catalogue::from_inputs(&[SourceInput::supplied(MODEL_KEY, self.model)])
            .and_then(|catalogue| {
                let request = match case {
                    ModelCase::InitialCatalogue => CheckRequest::new(),
                    ModelCase::FullSelected => CheckRequest::new()
                        .root(
                            catalogue
                                .source_id(MODEL_KEY)
                                .ok_or("model source missing")?,
                        )
                        .assessment(Assessment::FullBaseline),
                };
                let report = catalogue.check(request.strict_warnings(true))?;
                let outcome = report.outcome();
                Ok(ModelResult {
                    assessment: report.assessment(),
                    disposition: outcome.disposition(),
                    complete: outcome.complete(),
                    errors: outcome.errors(),
                    warnings: outcome.warnings(),
                    coverage: report.requested_coverage().into(),
                })
            })
            .map_err(str::to_owned);
        ModelCheck { case, native }
    }

    /// The encoded size of a value, refused before any storage exceeds the sample limit.
    pub fn encoded_len(&self, id: SampleId, value: &SampleValue<'_>) -> Result<usize, String> {
        let mut counter = Counter {
            len: 0,
            limit: self.bounds.sample_bytes,
        };
        value
            .write_json(&mut counter)
            .map_err(|_| self.too_large(id))?;
        Ok(counter.len)
    }

    /// Check one value whose bytes the caller does not retain.
    pub fn check_value(
        &mut self,
        step: u32,
        value: &SampleValue<'_>,
    ) -> Result<Option<Admission>, String> {
        let id = SampleId::new(step, value.role());
        let mut bytes = Vec::with_capacity(self.encoded_len(id, value)?);
        encode(value, &mut bytes)?;
        self.check(id, &bytes)
    }

    /// Check encoded bytes and retain the compact result. `None` means no usable native result.
    pub fn check(&mut self, id: SampleId, bytes: &[u8]) -> Result<Option<Admission>, String> {
        if bytes.len() > self.bounds.sample_bytes {
            return Err(self.too_large(id));
        }
        self.sample_requests_remaining =
            self.sample_requests_remaining
                .checked_sub(1)
                .ok_or_else(|| {
                    format!(
                        "{id}: allowance of {} Sample-value checking requests exhausted",
                        self.bounds.sample_requests
                    )
                })?;
        let native = self.sample(id, bytes);
        let admission = native.as_ref().ok().map(|result| result.admission);
        self.checks.push(SampleCheck {
            id,
            bytes: bytes.len(),
            native,
        });
        Ok(admission)
    }

    fn sample(&self, id: SampleId, bytes: &[u8]) -> Result<NativeResult, String> {
        let key = id.key();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied(MODEL_KEY, self.model),
            SourceInput::supplied(&key, bytes).sample(),
        ])?;
        let model = catalogue
            .source_id(MODEL_KEY)
            .ok_or("model source missing")?;
        let sample = catalogue.source_id(&key).ok_or("sample source missing")?;
        let (kind, pointer) = selector(id.role);
        let report =
            catalogue.check_sample(SampleRequest::new(model, kind, pointer).sample(sample))?;
        let result = report
            .sample_result()
            .ok_or("native sample result unavailable")?;
        Ok(NativeResult {
            kind: result.selected_kind(),
            admission: result.admission(),
            disposition: report.outcome().disposition(),
            complete: report.outcome().complete(),
            coverage: report.requested_coverage().into(),
            errors: errors(&report, model, sample),
        })
    }

    fn too_large(&self, id: SampleId) -> String {
        format!(
            "{id}: sample value exceeds {} UTF-8 bytes",
            self.bounds.sample_bytes
        )
    }

    pub fn into_checks(self) -> Vec<SampleCheck> {
        self.checks
    }
}

pub fn encode(value: &SampleValue<'_>, reserved: &mut Vec<u8>) -> Result<(), String> {
    value
        .write_json(&mut Reserved(reserved))
        .map_err(|_| "sample value outgrew its reserved storage".to_owned())
}

fn errors(report: &Report<'_>, model: SourceId, sample: SourceId) -> Vec<Diagnostic> {
    report
        .diagnostics()
        .conditions()
        .filter(|condition| condition.severity() == Severity::Error)
        .take(RETAINED_ERRORS)
        .map(|condition| Diagnostic {
            code: condition.code().to_owned(),
            sample_pointer: match condition.primary() {
                Field::Present(ReportLocation::Sample(location))
                    if location.source_id() == sample =>
                {
                    location.pointer().present().map(str::to_owned)
                }
                _ => None,
            },
            source_pointers: condition
                .related()
                .filter_map(|location| match location {
                    ReportLocation::Source(location) if location.source_id() == model => {
                        location.pointer().present().map(str::to_owned)
                    }
                    _ => None,
                })
                .collect(),
        })
        .collect()
}

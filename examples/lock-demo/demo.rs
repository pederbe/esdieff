//! The lock-demo/1 demonstration: model checks, replay, separate summaries and output.
use crate::behavior::{BehaviorSummary, assess_behavior};
use crate::expected_native::{MODELS, samples};
use crate::matching::{NativeSummary, Summary, match_models, match_samples};
use crate::native::{Bounds, ModelCase, ModelCheck, NativeChecker, Role, SampleId};
use crate::scenario::{self, LITERAL_PREFIX, REPLAY, Seed};
use crate::simulator::{Run, replay};
use crate::trace::Interaction;
use crate::wording::{event, invocation, result};
use esdieff::Admission;
use std::{
    ffi::OsString,
    io::{self, Write},
};

pub const RICH_MODEL: &[u8] = include_bytes!("../door-lock-check/rich.sdf.json");

pub struct Demonstration {
    pub seed: Seed,
    pub models: Vec<ModelCheck>,
    pub model_summary: Summary,
    pub run: Option<Run>,
    pub native: Option<NativeSummary>,
    pub behavior: Option<BehaviorSummary>,
}
impl Demonstration {
    /// Success needs matching model checks, a finished run and both separate summaries.
    pub fn passed(&self) -> bool {
        self.model_summary.passed()
            && self.run.as_ref().is_some_and(|run| run.halt.is_none())
            && self
                .native
                .as_ref()
                .is_some_and(|native| native.summary.passed())
            && self.behavior.as_ref().is_some_and(BehaviorSummary::passed)
    }
}

/// The demonstration takes at most one seed argument and refuses anything else before simulating.
pub fn arguments(mut arguments: impl Iterator<Item = OsString>) -> Result<Seed, String> {
    let Some(seed) = arguments.next() else {
        return Ok(Seed::DEFAULT);
    };
    if arguments.next().is_some() {
        return Err("the demonstration takes at most one seed argument".to_owned());
    }
    Seed::parse(&seed)
}

pub fn demonstrate(model: &[u8], seed: Seed) -> Demonstration {
    let mut checker = NativeChecker::new(model, Bounds::LOCK_DEMO);
    let models: Vec<ModelCheck> = [ModelCase::InitialCatalogue, ModelCase::FullSelected]
        .into_iter()
        .map(|case| checker.check_model(case))
        .collect();
    let model_summary = match_models(&MODELS, &models);
    let mut demonstration = Demonstration {
        seed,
        models,
        model_summary,
        run: None,
        native: None,
        behavior: None,
    };
    if demonstration.model_summary.passed() {
        let commands = scenario::commands(seed);
        let run = replay(checker, &commands);
        demonstration.native = Some(match_samples(&samples(&commands), &run.checks));
        demonstration.behavior = Some(assess_behavior(&commands, &run.trace));
        demonstration.run = Some(run);
    }
    demonstration
}

pub fn render(out: &mut impl Write, demonstration: &Demonstration) -> io::Result<()> {
    writeln!(
        out,
        "Door-lock demonstration {REPLAY} with seed {}",
        demonstration.seed.get()
    )?;
    for check in &demonstration.models {
        match &check.native {
            Ok(result) => writeln!(
                out,
                "{}: {}, {}, {} with {} of {} areas finished",
                check.case,
                result.disposition.as_str(),
                if result.complete {
                    "complete"
                } else {
                    "incomplete"
                },
                result
                    .coverage
                    .inventory
                    .as_deref()
                    .unwrap_or("no inventory"),
                result
                    .coverage
                    .assessed
                    .saturating_add(result.coverage.not_applicable),
                result.coverage.total.unwrap_or(0),
            )?,
            Err(reason) => writeln!(out, "{}: no native result ({reason})", check.case)?,
        }
    }
    summary(
        out,
        "",
        "Model expectations",
        &demonstration.model_summary,
        "",
    )?;
    let Some(run) = &demonstration.run else {
        writeln!(
            out,
            "Simulation not started because the model checks did not match."
        )?;
        return writeln!(out, "Result: failed");
    };
    writeln!(out)?;
    let initial = &run.trace.initial;
    writeln!(
        out,
        "{} initial reading {}{}",
        initial.at,
        initial.state.as_str(),
        admission_note(run, SampleId::new(0, Role::Reading))
    )?;
    for step in &run.trace.steps {
        let generated = usize::try_from(step.number).is_ok_and(|n| n > LITERAL_PREFIX.len());
        let origin = if generated { " (generated)" } else { "" };
        writeln!(
            out,
            "{} step {}{origin}, {}",
            step.at,
            step.number,
            invocation(step.invoked)
        )?;
        for interaction in &step.interactions {
            let note = |role| admission_note(run, SampleId::new(step.number, role));
            match interaction {
                Interaction::Input(bytes) => writeln!(
                    out,
                    "  input {}{}",
                    String::from_utf8_lossy(bytes),
                    note(Role::Input)
                )?,
                Interaction::Output(r) => writeln!(
                    out,
                    "  result {}{}",
                    result(r.accepted, r.state.as_str()),
                    note(Role::Output)
                )?,
                Interaction::Event(e) => writeln!(
                    out,
                    "  event {} at {}{}",
                    event(e.state.as_str()),
                    e.at,
                    note(Role::Event)
                )?,
                Interaction::Reading(state) => {
                    writeln!(out, "  reading {}{}", state.as_str(), note(Role::Reading))?
                }
            }
        }
    }
    if let Some(reason) = &run.halt {
        writeln!(out, "Run stopped: {reason}")?;
    }
    writeln!(out)?;
    let bounds = Bounds::LOCK_DEMO;
    writeln!(
        out,
        "Finite work: {} of {} Model checks, {} of {} Sample-value checking requests, largest sample value {} of {} bytes, {} of {} trace bytes",
        demonstration.models.len(),
        bounds.model_checks,
        run.checks.len(),
        bounds.sample_requests,
        run.checks
            .iter()
            .map(|check| check.bytes)
            .max()
            .unwrap_or(0),
        bounds.sample_bytes,
        run.trace.charged(),
        bounds.trace_bytes
    )?;
    if let Some(native) = &demonstration.native {
        let counts = format!(
            "; {} checked: {} admitted, {} rejected, {} undetermined, {} unavailable",
            run.checks.len(),
            native.admitted,
            native.rejected,
            native.undetermined,
            native.unavailable
        );
        summary(
            out,
            "",
            "Native sample expectations",
            &native.summary,
            &counts,
        )?;
    }
    if let Some(behavior) = &demonstration.behavior {
        writeln!(
            out,
            "Authored behavior assertions: {}",
            verdict(behavior.passed())
        )?;
        summary(out, "  ", "literal prefix comparison", &behavior.prefix, "")?;
        for rule in &behavior.rules {
            writeln!(
                out,
                "  {}: {}, {} of {} held",
                rule.rule.name(),
                verdict(rule.passed()),
                rule.matched,
                rule.applied
            )?;
            mismatches(out, "    ", &rule.mismatches)?;
        }
    }
    writeln!(out, "Result: {}", verdict(demonstration.passed()))
}

fn verdict(passed: bool) -> &'static str {
    if passed { "passed" } else { "failed" }
}

fn summary(
    out: &mut impl Write,
    indent: &str,
    name: &str,
    summary: &Summary,
    detail: &str,
) -> io::Result<()> {
    writeln!(
        out,
        "{indent}{name}: {}, {} of {} matched{detail}",
        verdict(summary.passed()),
        summary.matched,
        summary.expected
    )?;
    mismatches(out, &format!("{indent}  "), &summary.mismatches)
}

fn mismatches(out: &mut impl Write, indent: &str, mismatches: &[String]) -> io::Result<()> {
    for mismatch in mismatches {
        writeln!(out, "{indent}mismatch: {mismatch}")?;
    }
    Ok(())
}

/// The note shown beside a value: its native admission, decisive diagnostic or missing result.
fn admission_note(run: &Run, id: SampleId) -> String {
    let Some(check) = run.checks.iter().find(|check| check.id == id) else {
        return " (not checked)".to_owned();
    };
    match &check.native {
        Err(reason) => format!(" (no native result: {reason})"),
        Ok(result) => match (result.admission, result.errors.first()) {
            (Admission::Rejected, Some(error)) => format!(
                " (rejected, {} at {})",
                error.code,
                match error.sample_pointer.as_deref() {
                    Some("") => "the whole value",
                    Some(pointer) => pointer,
                    None => "an unknown location",
                }
            ),
            (admission, _) => format!(" ({})", admission.as_str()),
        },
    }
}

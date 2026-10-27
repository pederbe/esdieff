//! Authored behavior assertions over the observed Simulation trace: the literal prefix
//! comparison and the independent oracle's named rules, reported separately.
use crate::expected::{INITIAL_READING, LiteralStep, PREFIX_TRACE};
use crate::matching::Summary;
use crate::oracle::{self, RuleResult};
use crate::scenario::Command;
use crate::trace::{Step, Trace};
use crate::wording::{event, invocation, listed, mismatch, result};

pub struct BehaviorSummary {
    pub prefix: Summary,
    pub rules: Vec<RuleResult>,
}
impl BehaviorSummary {
    pub fn passed(&self) -> bool {
        self.prefix.passed() && !self.rules.is_empty() && self.rules.iter().all(RuleResult::passed)
    }
}

pub fn assess_behavior(commands: &[Command], trace: &Trace) -> BehaviorSummary {
    BehaviorSummary {
        prefix: compare_prefix(trace),
        rules: oracle::assess(commands, trace),
    }
}

/// Compare the initial reading and each literal step with the authored prefix table. Step
/// accounting belongs to the oracle.
pub fn compare_prefix(trace: &Trace) -> Summary {
    let mut summary = Summary {
        expected: PREFIX_TRACE.len() + 1,
        ..Summary::default()
    };
    let (at, state) = INITIAL_READING;
    match mismatch(
        "initial reading",
        format!("{state} at {at}"),
        format!("{} at {}", trace.initial.state.as_str(), trace.initial.at),
    ) {
        None => summary.matched += 1,
        Some(problem) => summary.mismatches.push(problem),
    }
    for literal in &PREFIX_TRACE {
        match trace.steps.iter().find(|step| step.number == literal.step) {
            Some(step) => {
                let problems = step_problems(literal, step);
                if problems.is_empty() {
                    summary.matched += 1;
                }
                summary.mismatches.extend(problems);
            }
            None => summary
                .mismatches
                .push(format!("step {}: missing from the trace", literal.step)),
        }
    }
    summary
}

fn step_problems(literal: &LiteralStep, step: &Step) -> Vec<String> {
    let timed = |state: &str, at: &str| format!("{} at {at}", event(state));
    [
        mismatch("time", literal.at.to_owned(), step.at.to_string()),
        mismatch(
            "Action input",
            literal.input.to_owned(),
            listed(step.inputs(), |bytes| {
                String::from_utf8_lossy(bytes).into_owned()
            }),
        ),
        mismatch(
            "invocation",
            invocation(literal.invoked),
            invocation(step.invoked),
        ),
        mismatch(
            "Action result",
            listed(literal.result, |(accepted, state)| result(accepted, state)),
            listed(step.outputs(), |r| result(r.accepted, r.state.as_str())),
        ),
        mismatch(
            "Event",
            listed(literal.event, |(state, at)| timed(state, at)),
            listed(step.events(), |e| {
                timed(e.state.as_str(), &e.at.to_string())
            }),
        ),
        mismatch(
            "subsequent reading",
            literal.reading.to_owned(),
            listed(step.readings(), |state| state.as_str().to_owned()),
        ),
    ]
    .into_iter()
    .flatten()
    .map(|problem| format!("step {}: {problem}", literal.step))
    .collect()
}

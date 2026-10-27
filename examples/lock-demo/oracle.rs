//! The independent sequence oracle for the authored lock-demo/1 behavior rules.
//! It reads only the authored commands, the authored expectations and the observed Simulation
//! trace. It never sees the simulator or native checking, and it tracks its own expected state
//! from the authored initial conditions. The rules belong to this demonstration and are not
//! inferred from SDF descriptions.
use crate::expected::{Expectation, ExpectedStep, INITIAL_STATE, Outcome, expected_steps};
use crate::scenario::{Command, Credential, LockState};
use crate::trace::{Interaction, Step, Trace};
use crate::wording::{event, invocation, listed, mismatch, result};
use serde_json::{Map, Value};
use std::fmt::Display;

/// The named lock-demo/1 behavior rules. Variant order is the reporting order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    StepAccounting,
    StepTiming,
    InteractionOrder,
    RejectedInput,
    RefusedRequest,
    AcceptedRequest,
    StateContinuity,
}
impl Rule {
    pub const ALL: [Self; 7] = [
        Self::StepAccounting,
        Self::StepTiming,
        Self::InteractionOrder,
        Self::RejectedInput,
        Self::RefusedRequest,
        Self::AcceptedRequest,
        Self::StateContinuity,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::StepAccounting => "step accounting",
            Self::StepTiming => "step timing",
            Self::InteractionOrder => "interaction order",
            Self::RejectedInput => "rejected input",
            Self::RefusedRequest => "refused request",
            Self::AcceptedRequest => "accepted request",
            Self::StateContinuity => "state continuity",
        }
    }
}

/// How often one rule applied and held, with every mismatch it found.
#[derive(Debug)]
pub struct RuleResult {
    pub rule: Rule,
    pub applied: usize,
    pub matched: usize,
    pub mismatches: Vec<String>,
}
impl RuleResult {
    pub fn passed(&self) -> bool {
        self.matched == self.applied && self.mismatches.is_empty()
    }
}

struct Assertions {
    results: Vec<RuleResult>,
}
impl Assertions {
    /// Apply one rule at one place. Each problem names the place, the rule and both values.
    fn apply(
        &mut self,
        rule: Rule,
        place: &dyn Display,
        problems: impl IntoIterator<Item = String>,
    ) {
        let result = &mut self.results[rule as usize];
        result.applied += 1;
        let before = result.mismatches.len();
        result.mismatches.extend(
            problems
                .into_iter()
                .map(|problem| format!("{place} {}: {problem}", rule.name())),
        );
        if result.mismatches.len() == before {
            result.matched += 1;
        }
    }
}

fn state(state: LockState) -> String {
    state.as_str().to_owned()
}

/// Assess the observed trace against the authored commands. Native results play no part.
pub fn assess(commands: &[Command], trace: &Trace) -> Vec<RuleResult> {
    let mut assertions = Assertions {
        results: Rule::ALL
            .iter()
            .map(|rule| RuleResult {
                rule: *rule,
                applied: 0,
                matched: 0,
                mismatches: Vec::new(),
            })
            .collect(),
    };
    // Expected state follows the authored commands, whether or not a step was observed.
    let expected = expected_steps(commands);
    account(&mut assertions, commands, &expected, trace);
    timing(&mut assertions, trace);
    for step in &trace.steps {
        assertions.apply(
            Rule::InteractionOrder,
            &format!("step {}", step.number),
            order(step),
        );
    }
    for expected in &expected {
        let (Some(expectation), Some(outcome)) = (expected.expectation, expected.outcome()) else {
            continue;
        };
        let Some(step) = trace.steps.iter().find(|s| s.number == expected.step) else {
            continue;
        };
        let rule = match expectation {
            Expectation::Rejected => Rule::RejectedInput,
            Expectation::Refused => Rule::RefusedRequest,
            Expectation::Accepted(_) => Rule::AcceptedRequest,
        };
        assertions.apply(
            rule,
            &format!("step {}", expected.step),
            compare_outcome(step, &outcome),
        );
    }
    continuity(&mut assertions, trace);
    assertions.results
}

/// Each authored command has exactly one step, in command order, carrying that command as its
/// Action input, and every command has an authored admission.
fn account(
    assertions: &mut Assertions,
    commands: &[Command],
    expected: &[ExpectedStep],
    trace: &Trace,
) {
    for (command, expected) in commands.iter().zip(expected) {
        let recorded: Vec<&Step> = trace
            .steps
            .iter()
            .filter(|step| step.number == expected.step)
            .collect();
        let mut problems: Vec<String> = mismatch("recorded steps", 1, recorded.len())
            .into_iter()
            .collect();
        if expected.expectation.is_none() {
            problems.push("authored admission expected for its command, observed none".to_owned());
        }
        if let [step] = recorded[..] {
            problems.extend(input_problem(command, step));
        }
        assertions.apply(
            Rule::StepAccounting,
            &format!("step {}", expected.step),
            problems,
        );
    }
    let numbers: Vec<u32> = trace.steps.iter().map(|step| step.number).collect();
    if numbers.windows(2).any(|pair| pair[0] >= pair[1]) {
        assertions.apply(
            Rule::StepAccounting,
            &"trace",
            [format!(
                "step order expected increasing, observed {}",
                listed(&numbers, u32::to_string)
            )],
        );
    }
    for step in &trace.steps {
        let authored =
            usize::try_from(step.number).is_ok_and(|number| (1..=commands.len()).contains(&number));
        if !authored {
            assertions.apply(
                Rule::StepAccounting,
                &format!("step {}", step.number),
                ["authored command expected, observed none".to_owned()],
            );
        }
    }
}

/// Compare the meaning of the recorded input bytes with the authored command: the credential
/// text or null, the operation or its absence, and no other entries. Byte layout is ignored.
fn input_problem(command: &Command, step: &Step) -> Option<String> {
    let mut entries = Map::new();
    let credential = match command.credential {
        Credential::Token(token) => Value::from(token),
        Credential::Null => Value::Null,
    };
    entries.insert("credential".to_owned(), credential);
    if let Some(operation) = command.operation {
        entries.insert("operation".to_owned(), Value::from(operation.as_str()));
    }
    let authored = Value::Object(entries);
    let inputs: Vec<Option<Value>> = step
        .inputs()
        .map(|bytes| serde_json::from_slice(bytes).ok())
        .collect();
    if inputs[..] == [Some(authored.clone())] {
        return None;
    }
    let shown = listed(&inputs, |input| {
        input
            .as_ref()
            .map_or_else(|| "undecodable bytes".to_owned(), Value::to_string)
    });
    Some(format!(
        "Action input expected {authored}, observed {shown}"
    ))
}

/// The initial reading is at the epoch, step n is n whole seconds later, and every Event
/// occurs at its step's instant.
fn timing(assertions: &mut Assertions, trace: &Trace) {
    assertions.apply(
        Rule::StepTiming,
        &"initial reading",
        mismatch("seconds after the epoch", 0, trace.initial.at.seconds()),
    );
    for step in &trace.steps {
        let problems = mismatch("seconds after the epoch", step.number, step.at.seconds())
            .into_iter()
            .chain(
                step.events()
                    .filter_map(|event| mismatch("Event time", step.at, event.at)),
            );
        assertions.apply(Rule::StepTiming, &format!("step {}", step.number), problems);
    }
}

/// One input first, then any output, then any Event, then one reading last.
fn order(step: &Step) -> Option<String> {
    let rank = |interaction: &Interaction| match interaction {
        Interaction::Input(_) => 0,
        Interaction::Output(_) => 1,
        Interaction::Event(_) => 2,
        Interaction::Reading(_) => 3,
    };
    let ranks: Vec<u8> = step.interactions.iter().map(rank).collect();
    let ordered = ranks.first() == Some(&0)
        && ranks.last() == Some(&3)
        && ranks.iter().filter(|rank| **rank == 0).count() == 1
        && ranks.iter().filter(|rank| **rank == 3).count() == 1
        && ranks.windows(2).all(|pair| pair[0] <= pair[1]);
    let names = ["input", "output", "Event", "reading"];
    (!ordered).then(|| {
        format!(
            "sequence expected input, optional output, optional Event, reading, observed {}",
            listed(&ranks, |rank| names[usize::from(*rank)].to_owned())
        )
    })
}

/// Compare a step with the outcome its authored expectation predicts.
fn compare_outcome(step: &Step, expected: &Outcome) -> Vec<String> {
    [
        mismatch(
            "invocation",
            invocation(expected.invoked),
            invocation(step.invoked),
        ),
        mismatch(
            "Action result",
            listed(expected.result, |(accepted, state)| {
                result(accepted, state.as_str())
            }),
            listed(step.outputs(), |r| result(r.accepted, r.state.as_str())),
        ),
        mismatch(
            "Event",
            listed(expected.event, |state| event(state.as_str())),
            listed(step.events(), |e| event(e.state.as_str())),
        ),
        mismatch(
            "reading",
            state(expected.reading),
            listed(step.readings(), state),
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Readings start from the authored initial state and change only through a stateChanged
/// Event in the same step, so nothing changes the state between steps.
fn continuity(assertions: &mut Assertions, trace: &Trace) {
    assertions.apply(
        Rule::StateContinuity,
        &"initial reading",
        mismatch("state", state(INITIAL_STATE), state(trace.initial.state)),
    );
    let mut previous = trace.initial.state;
    for step in &trace.steps {
        let Some(current) = step.readings().last() else {
            continue;
        };
        let announced = (current != previous).then_some(current);
        assertions.apply(
            Rule::StateContinuity,
            &format!("step {}", step.number),
            mismatch(
                &format!(
                    "Event between readings {} and {}",
                    previous.as_str(),
                    current.as_str()
                ),
                listed(announced, |state| event(state.as_str())),
                listed(step.events(), |e| event(e.state.as_str())),
            ),
        );
        previous = current;
    }
}

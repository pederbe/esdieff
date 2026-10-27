//! The lock-demo/1 demonstration observed through its runner, trace and summaries.
#[path = "../examples/lock-demo/behavior.rs"]
mod behavior;
#[path = "../examples/lock-demo/demo.rs"]
mod demo;
#[path = "../examples/lock-demo/expected.rs"]
mod expected;
#[path = "../examples/lock-demo/expected_native.rs"]
mod expected_native;
#[path = "../examples/lock-demo/matching.rs"]
mod matching;
#[path = "../examples/lock-demo/native.rs"]
mod native;
#[path = "../examples/lock-demo/oracle.rs"]
mod oracle;
#[path = "../examples/lock-demo/scenario.rs"]
mod scenario;
#[path = "../examples/lock-demo/simulator.rs"]
mod simulator;
#[path = "../examples/lock-demo/trace.rs"]
mod trace;
#[path = "../examples/lock-demo/wording.rs"]
mod wording;

use behavior::{assess_behavior, compare_prefix};
use demo::{Demonstration, RICH_MODEL, demonstrate, render};
use esdieff::{Admission, Disposition};
use matching::{NativeSummary, match_samples};
use native::{Bounds, NativeChecker, NativeResult, Role, SampleCheck, SampleId, SampleValue};
use oracle::{Rule, RuleResult};
use scenario::{Command, Credential, LockState, Operation, Seed, SimTime};
use serde_json::{Value, json};
use simulator::{Run, replay};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use trace::{ActionResult, Interaction, StateChanged, Step, Trace};

const LIMITED_MODEL: &[u8] = include_bytes!("../examples/door-lock-check/limited.sdf.json");

#[test]
fn simulation_starts_only_after_the_rich_model_checks_match() {
    let rich = demonstrate(RICH_MODEL, Seed::DEFAULT);
    assert_eq!(rich.model_summary.mismatches, Vec::<String>::new());
    assert_eq!(rich.model_summary.matched, 2);
    assert!(rich.run.is_some());

    // The limited variant fails its own model checks, so nothing is simulated or summarized.
    let limited = demonstrate(LIMITED_MODEL, Seed::DEFAULT);
    assert!(limited.run.is_none());
    assert!(limited.native.is_none() && limited.behavior.is_none());
    assert!(!limited.passed());
    for expected in [
        "Initial Catalogue check: disposition expected passed, observed failed",
        "Initial Catalogue check: completion expected true, observed false",
        "Full Selected-model check: disposition expected passed, observed failed",
    ] {
        assert!(
            limited
                .model_summary
                .mismatches
                .iter()
                .any(|m| m == expected),
            "{expected}: {:?}",
            limited.model_summary.mismatches
        );
    }
}

const MINIMAL_MODEL: &[u8] = include_bytes!("../examples/door-lock-check/minimal.sdf.json");

fn default_run() -> Demonstration {
    let demonstration = demonstrate(RICH_MODEL, Seed::DEFAULT);
    assert!(
        demonstration.run.is_some(),
        "the rich model must reach simulation"
    );
    demonstration
}

fn replayed(demonstration: &Demonstration) -> &Run {
    demonstration.run.as_ref().unwrap()
}

fn rendered(demonstration: &Demonstration) -> String {
    let mut out = Vec::new();
    render(&mut out, demonstration).unwrap();
    String::from_utf8(out).unwrap()
}

/// A step's interactions in observed order, written plainly.
fn sequence(step: &Step) -> Vec<String> {
    step.interactions
        .iter()
        .map(|interaction| match interaction {
            Interaction::Input(_) => "input".to_owned(),
            Interaction::Output(r) => format!("result {} {}", r.accepted, r.state.as_str()),
            Interaction::Event(e) => format!("event {} at {}", e.state.as_str(), e.at),
            Interaction::Reading(state) => format!("reading {}", state.as_str()),
        })
        .collect()
}

fn check(run: &Run, step: u32, role: Role) -> &NativeResult {
    let id = SampleId::new(step, role);
    let mut found = run.checks.iter().filter(|check| check.id == id);
    let check = found
        .next()
        .unwrap_or_else(|| panic!("{id} was not checked"));
    assert!(found.next().is_none(), "{id} was checked twice");
    check.native.as_ref().unwrap()
}

#[test]
fn default_replay_passes_every_native_expectation_and_the_literal_prefix() {
    let demonstration = default_run();
    assert!(demonstration.passed());
    let run = replayed(&demonstration);
    assert_eq!(run.halt, None);
    assert_eq!(run.trace.steps.len(), 17);
    let native = demonstration.native.as_ref().unwrap();
    assert_eq!(native.summary.mismatches, Vec::<String>::new());
    // The prefix has the initial reading, four values in steps 1, 7 and 9, three in steps 2,
    // 3 and 8, and an input and reading in each of steps 4, 5 and 6. Seed 1 adds three values
    // for each generated step and one Event when step 15 locks.
    assert_eq!((native.summary.matched, native.summary.expected), (53, 53));
    assert_eq!(run.checks.len(), 53);
    // Expected rejections stay rejected while still counting as matched expectations.
    assert_eq!(
        (
            native.admitted,
            native.rejected,
            native.undetermined,
            native.unavailable
        ),
        (50, 3, 0, 0)
    );
    let behavior = demonstration.behavior.as_ref().unwrap();
    assert_eq!(behavior.prefix.mismatches, Vec::<String>::new());
    assert_eq!(
        (behavior.prefix.matched, behavior.prefix.expected),
        (10, 10)
    );
}

#[test]
fn simulated_time_advances_one_second_per_step_including_refused_and_malformed_requests() {
    let demonstration = default_run();
    let trace = &replayed(&demonstration).trace;
    assert_eq!(trace.initial.at.to_string(), "2024-01-01T12:00:00Z");
    assert_eq!(trace.initial.state.as_str(), "locked");
    let times: Vec<String> = trace.steps.iter().map(|s| s.at.to_string()).collect();
    assert_eq!(
        times,
        [
            "2024-01-01T12:00:01Z",
            "2024-01-01T12:00:02Z",
            "2024-01-01T12:00:03Z",
            "2024-01-01T12:00:04Z",
            "2024-01-01T12:00:05Z",
            "2024-01-01T12:00:06Z",
            "2024-01-01T12:00:07Z",
            "2024-01-01T12:00:08Z",
            "2024-01-01T12:00:09Z",
            "2024-01-01T12:00:10Z",
            "2024-01-01T12:00:11Z",
            "2024-01-01T12:00:12Z",
            "2024-01-01T12:00:13Z",
            "2024-01-01T12:00:14Z",
            "2024-01-01T12:00:15Z",
            "2024-01-01T12:00:16Z",
            "2024-01-01T12:00:17Z",
        ]
    );
}

#[test]
fn refused_idempotent_and_changing_requests_follow_the_authored_rules() {
    let demonstration = default_run();
    let steps = &replayed(&demonstration).trace.steps;
    assert!(
        steps
            .iter()
            .all(|step| step.invoked != (4..=6).contains(&step.number))
    );
    // The well-formed nope request is invoked and refused without a change or Event.
    assert_eq!(
        sequence(&steps[2]),
        ["input", "result false unlocked", "reading unlocked"]
    );
    // Repeating a satisfied request is accepted without an Event.
    assert_eq!(
        sequence(&steps[1]),
        ["input", "result true unlocked", "reading unlocked"]
    );
    assert_eq!(
        sequence(&steps[7]),
        ["input", "result true locked", "reading locked"]
    );
    // Each actual change has one Event at the step time, then a matching reading.
    for (index, state, at) in [
        (0, "unlocked", "2024-01-01T12:00:01Z"),
        (6, "locked", "2024-01-01T12:00:07Z"),
        (8, "unlocked", "2024-01-01T12:00:09Z"),
    ] {
        assert_eq!(
            sequence(&steps[index]),
            [
                "input".to_owned(),
                format!("result true {state}"),
                format!("event {state} at {at}"),
                format!("reading {state}"),
            ]
        );
    }
}

#[test]
fn malformed_inputs_reject_with_decisive_diagnostics_and_are_never_invoked() {
    let demonstration = default_run();
    let run = replayed(&demonstration);
    let authorize = "/sdfObject/Lock/sdfAction/authorize/sdfInputData";
    for (step, code, sample_pointer, source_pointer) in [
        (
            4,
            "sample.length",
            "/credential",
            "/properties/credential/minLength",
        ),
        (
            5,
            "sample.null",
            "/credential",
            "/properties/credential/nullable",
        ),
        (6, "sample.required", "", "/required/1"),
    ] {
        let input = check(run, step, Role::Input);
        assert_eq!(input.admission, Admission::Rejected, "step {step}");
        assert_eq!(input.disposition, Disposition::Failed, "step {step}");
        assert!(input.complete, "step {step}");
        assert_eq!(
            input.coverage.inventory.as_deref(),
            Some("esdieff.sample/1")
        );
        assert!(
            input.errors.iter().any(|error| error.code == code
                && error.sample_pointer.as_deref() == Some(sample_pointer)
                && error
                    .source_pointers
                    .contains(&format!("{authorize}{source_pointer}"))),
            "step {step}: {:?}",
            input.errors
        );
        let trace = &run.trace.steps[step as usize - 1];
        assert!(!trace.invoked);
        assert_eq!(sequence(trace), ["input", "reading unlocked"]);
        for role in [Role::Output, Role::Event] {
            assert!(
                !run.checks.iter().any(|c| c.id == SampleId::new(step, role)),
                "step {step} has a fabricated {role:?}"
            );
        }
        assert_eq!(
            check(run, step, Role::Reading).admission,
            Admission::Admitted
        );
    }
    for step in [1, 2, 3, 7, 8, 9] {
        let input = check(run, step, Role::Input);
        assert_eq!(
            (input.admission, input.disposition, input.complete),
            (Admission::Admitted, Disposition::Passed, true),
            "step {step}"
        );
    }
}

fn assert_mentions(mismatches: &[String], expected: &str) {
    assert!(
        mismatches.iter().any(|m| m.contains(expected)),
        "{expected}: {mismatches:?}"
    );
}

/// Render a demonstration whose native comparison used altered inputs.
fn rendered_with_native(native: NativeSummary) -> String {
    let mut demonstration = default_run();
    demonstration.native = Some(native);
    assert!(!demonstration.passed());
    let text = rendered(&demonstration);
    assert!(
        text.contains("Native sample expectations: failed"),
        "{text}"
    );
    assert!(text.ends_with("Result: failed\n"), "{text}");
    assert!(
        !text.contains("Native sample expectations: passed"),
        "{text}"
    );
    text
}

type Alteration = fn(&mut Vec<SampleCheck>);

fn position(checks: &[SampleCheck], step: u32, role: Role) -> usize {
    let id = SampleId::new(step, role);
    checks.iter().position(|check| check.id == id).unwrap()
}

#[test]
fn native_regression_failures_are_named_and_never_summarized_as_success() {
    let demonstration = default_run();
    let good = &replayed(&demonstration).checks;

    let mut wrong = expected_native::samples(&scenario::commands(Seed::DEFAULT));
    let index = wrong
        .iter()
        .position(|e| e.id == SampleId::new(4, Role::Input))
        .unwrap();
    wrong[index].admission = Admission::Admitted;
    wrong[index].decisive = None;
    let native = match_samples(&wrong, good);
    assert_mentions(
        &native.summary.mismatches,
        "step 4 input: admission expected admitted, observed rejected",
    );
    // A wrong expectation does not relabel the native rejection.
    assert_eq!(native.rejected, 3);
    assert_eq!(native.summary.matched, 52);
    rendered_with_native(native);

    let changes: [(Alteration, &str); 5] = [
        (
            |checks| {
                checks.remove(position(checks, 7, Role::Event));
            },
            "step 7 event: missing sample result",
        ),
        (
            |checks| checks.push(checks[position(checks, 1, Role::Input)].clone()),
            "step 1 input: duplicate sample result",
        ),
        (
            |checks| {
                let mut extra = checks[position(checks, 2, Role::Reading)].clone();
                extra.id = SampleId::new(2, Role::Event);
                checks.push(extra);
            },
            "step 2 event: extra sample result without an expectation",
        ),
        (
            |checks| {
                let index = position(checks, 3, Role::Output);
                checks[index].native = Err("selected description unavailable".to_owned());
            },
            "step 3 output: native result unavailable (selected description unavailable)",
        ),
        (
            |checks| {
                let index = position(checks, 9, Role::Reading);
                let native = checks[index].native.as_mut().unwrap();
                native.complete = false;
                native.coverage.assessed -= 1;
                native.coverage.undetermined += 1;
            },
            "step 9 reading: unexpected incomplete result",
        ),
    ];
    for (change, message) in changes {
        let mut checks = good.clone();
        change(&mut checks);
        let native = match_samples(
            &expected_native::samples(&scenario::commands(Seed::DEFAULT)),
            &checks,
        );
        assert!(!native.summary.passed(), "{message}");
        assert_mentions(&native.summary.mismatches, message);
        let text = rendered_with_native(native);
        assert!(text.contains(&format!("  mismatch: {message}")), "{text}");
    }
}

#[test]
fn an_unavailable_native_input_result_stops_the_run_without_invoking() {
    // The minimal variant passes its own model checks but has no authorize Action.
    let demonstration = demonstrate(MINIMAL_MODEL, Seed::DEFAULT);
    assert_eq!(demonstration.model_summary.mismatches, Vec::<String>::new());
    assert!(!demonstration.passed());
    let run = replayed(&demonstration);
    let halt = run.halt.as_deref().unwrap();
    assert!(
        halt.starts_with("step 1 input: no usable native result, not invoked"),
        "{halt}"
    );
    // The refused step keeps its observed input and nothing after it.
    assert_eq!(run.trace.steps.len(), 1);
    assert!(!run.trace.steps[0].invoked);
    assert_eq!(sequence(&run.trace.steps[0]), ["input"]);
    assert_eq!(run.checks.len(), 2);
    let native = demonstration.native.as_ref().unwrap();
    assert_mentions(
        &native.summary.mismatches,
        "step 1 input: native result unavailable",
    );
    assert_mentions(
        &native.summary.mismatches,
        "step 9 reading: missing sample result",
    );
    let behavior = demonstration.behavior.as_ref().unwrap();
    assert_mentions(
        &behavior.prefix.mismatches,
        "step 2: missing from the trace",
    );
    assert_mentions(
        &failures(&behavior.rules, Rule::StepAccounting),
        "step 2 step accounting: recorded steps expected 1, observed 0",
    );
    let text = rendered(&demonstration);
    assert!(text.contains(&format!("Run stopped: {halt}")), "{text}");
    assert!(text.ends_with("Result: failed\n"), "{text}");
    assert!(
        !text.contains("Native sample expectations: passed")
            && !text.contains("Authored behavior assertions: passed"),
        "{text}"
    );
}

#[test]
fn an_undetermined_input_admission_stops_the_run_without_invoking() {
    // An unsupplied credential definition leaves every authorize input undetermined.
    let mut model: Value = serde_json::from_slice(RICH_MODEL).unwrap();
    model["sdfObject"]["Lock"]["sdfAction"]["authorize"]["sdfInputData"]["properties"]["credential"] =
        json!({"sdfRef": "#/sdfData/Unsupplied"});
    let model = serde_json::to_vec(&model).unwrap();
    let run = replay(
        NativeChecker::new(&model, Bounds::LOCK_DEMO),
        &scenario::commands(Seed::DEFAULT),
    );
    assert_eq!(
        run.halt.as_deref(),
        Some("step 1 input: undetermined admission, not invoked")
    );
    assert_eq!(run.trace.steps.len(), 1);
    assert!(!run.trace.steps[0].invoked);
    assert_eq!(sequence(&run.trace.steps[0]), ["input"]);
    assert_eq!(
        check(&run, 1, Role::Input).admission,
        Admission::Undetermined
    );
    assert_eq!(run.checks.len(), 2);
}

fn changing_commands(count: usize) -> Vec<Command> {
    (0..count)
        .map(|index| Command {
            credential: Credential::Token("demo"),
            operation: Some(if index % 2 == 0 {
                Operation::Unlock
            } else {
                Operation::Lock
            }),
        })
        .collect()
}

#[test]
fn sample_checking_requests_stop_at_sixty_and_are_never_replenished() {
    // Each changing step needs four checks, so the reading of step 15 would be the 61st.
    let run = replay(
        NativeChecker::new(RICH_MODEL, Bounds::LOCK_DEMO),
        &changing_commands(17),
    );
    assert_eq!(run.checks.len(), 60);
    assert_eq!(
        run.halt.as_deref(),
        Some("step 15 reading: allowance of 60 Sample-value checking requests exhausted")
    );
    // Step 15 keeps what was observed before its reading was refused.
    assert_eq!(run.trace.steps.len(), 15);
    assert_eq!(
        sequence(&run.trace.steps[14]),
        [
            "input",
            "result true unlocked",
            "event unlocked at 2024-01-01T12:00:15Z"
        ]
    );
    // The demonstration itself stays well inside the allowance after its two Model checks.
    let demonstration = default_run();
    assert_eq!(demonstration.models.len(), 2);
    assert!(replayed(&demonstration).checks.len() <= 60);
}

#[test]
fn sample_values_are_limited_to_512_utf8_bytes_before_checking() {
    // The input wrapper adds 36 bytes around the credential text.
    for (length, halted) in [(476, false), (477, true)] {
        let token: &'static str = "x".repeat(length).leak();
        let command = Command {
            credential: Credential::Token(token),
            operation: Some(Operation::Lock),
        };
        let run = replay(
            NativeChecker::new(RICH_MODEL, Bounds::LOCK_DEMO),
            &[command],
        );
        if halted {
            assert_eq!(
                run.halt.as_deref(),
                Some("step 1 input: sample value exceeds 512 UTF-8 bytes")
            );
            assert_eq!(run.checks.len(), 1, "only the initial reading was checked");
            assert!(run.trace.steps.is_empty());
        } else {
            assert_eq!(run.halt, None);
            let input = run.trace.steps[0].inputs().next().unwrap();
            assert_eq!(input.len(), 512);
            assert_eq!(check(&run, 1, Role::Input).admission, Admission::Rejected);
        }
    }
    // Multibyte credential text counts its UTF-8 bytes rather than its characters.
    let token: &'static str = "é".repeat(239).leak();
    let command = Command {
        credential: Credential::Token(token),
        operation: Some(Operation::Lock),
    };
    let run = replay(
        NativeChecker::new(RICH_MODEL, Bounds::LOCK_DEMO),
        &[command],
    );
    assert_eq!(
        run.halt.as_deref(),
        Some("step 1 input: sample value exceeds 512 UTF-8 bytes")
    );
}

#[test]
fn trace_storage_is_charged_before_it_grows() {
    let bounds = Bounds {
        trace_bytes: 0,
        ..Bounds::LOCK_DEMO
    };
    let run = replay(
        NativeChecker::new(RICH_MODEL, bounds),
        &scenario::commands(Seed::DEFAULT),
    );
    assert_eq!(
        run.halt.as_deref(),
        Some("trace storage allowance exhausted")
    );
    assert!(run.trace.steps.is_empty());
    assert_eq!(run.checks.len(), 1, "the input was refused before checking");
    // The fixed 64 KiB allowance holds the whole literal prefix.
    assert_eq!(Bounds::LOCK_DEMO.trace_bytes, 65536);
    assert_eq!(replayed(&default_run()).trace.steps.len(), 17);
}

#[test]
fn repeated_runs_reproduce_commands_traces_native_outcomes_and_output() {
    assert_eq!(
        scenario::commands(Seed::DEFAULT),
        scenario::commands(seed(1))
    );
    let first = default_run();
    let second = default_run();
    assert_eq!(replayed(&first).trace, replayed(&second).trace);
    assert_eq!(replayed(&first).checks, replayed(&second).checks);
    assert_eq!(first.models, second.models);
    assert_eq!(rendered(&first), rendered(&second));
}

fn input_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut bytes = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let contents = fs::read(&path).unwrap();
                bytes.insert(path.strip_prefix(root).unwrap().to_owned(), contents);
            }
        }
    }
    bytes
}

#[test]
fn curated_model_and_sample_bytes_remain_unchanged() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let curated = examples.join("door-lock-check");
    let demonstration_files = examples.join("lock-demo");
    let before = (input_bytes(&curated), input_bytes(&demonstration_files));
    assert_eq!(fs::read(curated.join("rich.sdf.json")).unwrap(), RICH_MODEL);
    for value in [1, 2, 4294967295] {
        assert!(
            demonstrate(RICH_MODEL, seed(value)).passed(),
            "seed {value}"
        );
    }
    let after = (input_bytes(&curated), input_bytes(&demonstration_files));
    assert!(before == after, "the demonstration changed or added files");
}

#[test]
fn the_seed_argument_is_optional_and_refused_before_simulating_when_invalid() {
    let arguments = |values: &[&str]| demo::arguments(values.iter().map(OsString::from));
    assert_eq!(arguments(&[]), Ok(Seed::DEFAULT));
    assert_eq!(Seed::DEFAULT.get(), 1);
    assert_eq!(arguments(&["2"]).map(Seed::get), Ok(2));
    assert_eq!(arguments(&["4294967295"]).map(Seed::get), Ok(4294967295));
    for values in [
        &["0"][..],
        &["4294967296"],
        &["-1"],
        &["+1"],
        &["abc"],
        &[""],
    ] {
        let refused = arguments(values).unwrap_err();
        assert!(
            refused.contains("1 through 4294967295"),
            "{values:?}: {refused}"
        );
    }
    let refused = arguments(&["1", "2"]).unwrap_err();
    assert!(refused.contains("at most one seed argument"), "{refused}");
}

#[test]
fn output_identifies_the_replay_and_ends_with_separate_summaries() {
    let text = rendered(&default_run());
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "Door-lock demonstration lock-demo/1 with seed 1");
    for expected in [
        "2024-01-01T12:00:00Z initial reading locked (admitted)",
        "2024-01-01T12:00:03Z step 3, invoked",
        "  result accepted false with unlocked (admitted)",
        "2024-01-01T12:00:04Z step 4, not invoked",
        r#"  input {"credential":"abc","operation":"lock"} (rejected, sample.length at /credential)"#,
        "  event stateChanged locked at 2024-01-01T12:00:07Z (admitted)",
        "  reading unlocked (admitted)",
        "2024-01-01T12:00:09Z step 9, invoked",
        "2024-01-01T12:00:10Z step 10 (generated), invoked",
        r#"  input {"credential":"nope","operation":"unlock"} (admitted)"#,
        "2024-01-01T12:00:17Z step 17 (generated), invoked",
    ] {
        assert!(lines.contains(&expected), "{expected}\n{text}");
    }
    let tail = &lines[lines.len() - 12..];
    // The longest value is a stateChanged unlocked Event, and trace storage is host-sized.
    let event = r#"{"state":"unlocked","at":"2024-01-01T12:00:01Z"}"#;
    let finite = format!(
        "Finite work: 2 of 2 Model checks, 53 of 60 Sample-value checking requests, largest sample value {} of 512 bytes, ",
        event.len()
    );
    assert!(tail[0].starts_with(&finite), "{text}");
    assert!(tail[0].ends_with(" of 65536 trace bytes"), "{text}");
    assert!(
        tail[1].starts_with("Native sample expectations: passed, 53 of 53 matched; 53 checked: 50 admitted, 3 rejected"),
        "{text}"
    );
    assert_eq!(
        tail[2..],
        [
            "Authored behavior assertions: passed",
            "  literal prefix comparison: passed, 10 of 10 matched",
            "  step accounting: passed, 17 of 17 held",
            "  step timing: passed, 18 of 18 held",
            "  interaction order: passed, 17 of 17 held",
            "  rejected input: passed, 3 of 3 held",
            "  refused request: passed, 4 of 4 held",
            "  accepted request: passed, 10 of 10 held",
            "  state continuity: passed, 18 of 18 held",
            "Result: passed",
        ]
    );
}

struct FailingOutput;
impl Write for FailingOutput {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("closed output"))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_host_output_failure_is_reported_to_the_caller() {
    assert!(render(&mut FailingOutput, &default_run()).is_err());
}

#[test]
fn the_literal_prefix_comparison_names_each_deviation() {
    let demonstration = default_run();
    let good = &replayed(&demonstration).trace;
    let compare = compare_prefix;
    let mut refused = good.clone();
    refused.steps[2].interactions[1] = Interaction::Output(ActionResult {
        accepted: true,
        state: LockState::Locked,
    });
    refused.steps[2].interactions[2] = Interaction::Reading(LockState::Locked);
    let summary = compare(&refused);
    assert_mentions(
        &summary.mismatches,
        "step 3: Action result expected accepted false with unlocked, observed accepted true with locked",
    );
    assert_mentions(
        &summary.mismatches,
        "step 3: subsequent reading expected unlocked, observed locked",
    );
    assert_eq!(summary.matched, 9);
    let text = assert_no_success(&refused);
    assert!(
        text.contains("  literal prefix comparison: failed, 9 of 10 matched"),
        "{text}"
    );
    assert!(
        text.contains(
            "    mismatch: step 3: subsequent reading expected unlocked, observed locked"
        ),
        "{text}"
    );

    let mut late = good.clone();
    late.steps[6].interactions[2] = Interaction::Event(StateChanged {
        state: LockState::Locked,
        at: SimTime::at_step(8).unwrap(),
    });
    assert_mentions(
        &compare(&late).mismatches,
        "step 7: Event expected stateChanged locked at 2024-01-01T12:00:07Z, observed stateChanged locked at 2024-01-01T12:00:08Z",
    );

    let mut short = good.clone();
    short.steps.remove(8);
    assert_mentions(
        &compare(&short).mismatches,
        "step 9: missing from the trace",
    );
    // Generated steps and their accounting belong to the oracle, not the literal comparison.
    let mut generated_only = good.clone();
    generated_only.steps.truncate(10);
    assert!(compare(good).passed() && compare(&generated_only).passed());
    assert!(!compare(&short).passed());
}

/// The rule results for a trace replayed from the authored lock-demo/1 commands.
fn rules(trace: &Trace) -> Vec<RuleResult> {
    oracle::assess(&scenario::commands(Seed::DEFAULT), trace)
}

fn failures(results: &[RuleResult], rule: Rule) -> Vec<String> {
    let result = results.iter().find(|result| result.rule == rule).unwrap();
    assert!(!result.passed(), "{} passed", rule.name());
    result.mismatches.clone()
}

fn failed_rules(trace: &Trace) -> Vec<&'static str> {
    rules(trace)
        .iter()
        .filter(|result| !result.passed())
        .map(|result| result.rule.name())
        .collect()
}

/// A mutated trace must never produce a passing behavior summary or verdict.
fn assert_no_success(trace: &Trace) -> String {
    let mut demonstration = default_run();
    let behavior = assess_behavior(&scenario::commands(Seed::DEFAULT), trace);
    assert!(!behavior.passed());
    demonstration.behavior = Some(behavior);
    demonstration.run.as_mut().unwrap().trace = trace.clone();
    assert!(!demonstration.passed());
    let text = rendered(&demonstration);
    assert!(
        text.contains("Authored behavior assertions: failed\n"),
        "{text}"
    );
    assert!(text.ends_with("Result: failed\n"), "{text}");
    text
}

/// Check an altered value through a fresh native checker, as the replay would have.
fn natively_admitted(step: u32, value: SampleValue<'_>) -> bool {
    let mut checker = NativeChecker::new(RICH_MODEL, Bounds::LOCK_DEMO);
    checker.check_value(step, &value) == Ok(Some(Admission::Admitted))
}

fn interaction_index(step: &Step, kind: fn(&Interaction) -> bool) -> usize {
    step.interactions.iter().position(kind).unwrap()
}

#[test]
fn seeds_one_two_and_u32_max_pass_the_same_finite_matrix() {
    // Worked by hand from each seed's generated commands after the prefix ends unlocked.
    // Seed 1 changes state once (step 15) with four refusals, seed 2 never changes state
    // with eight refusals, and seed 4294967295 changes once (step 15) with six refusals.
    for (value, checks, refused, accepted, events) in [
        (1, 53, 4, 10, 4),
        (2, 52, 8, 6, 3),
        (4294967295, 53, 6, 8, 4),
    ] {
        let demonstration = demonstrate(RICH_MODEL, seed(value));
        assert!(demonstration.passed(), "seed {value}");
        assert_eq!(demonstration.model_summary.matched, 2, "seed {value}");
        let run = replayed(&demonstration);
        assert_eq!(run.halt, None, "seed {value}");
        assert_eq!(run.trace.steps.len(), 17, "seed {value}");
        let observed_events: usize = run.trace.steps.iter().map(|s| s.events().count()).sum();
        assert_eq!(observed_events, events, "seed {value}");

        let native = demonstration.native.as_ref().unwrap();
        assert_eq!(
            native.summary.mismatches,
            Vec::<String>::new(),
            "seed {value}"
        );
        assert_eq!(
            (
                native.summary.matched,
                native.summary.expected,
                run.checks.len()
            ),
            (checks, checks, checks),
            "seed {value}"
        );
        assert_eq!(
            (
                native.admitted,
                native.rejected,
                native.undetermined,
                native.unavailable
            ),
            (checks - 3, 3, 0, 0),
            "seed {value}"
        );

        let behavior = demonstration.behavior.as_ref().unwrap();
        assert_eq!(
            (behavior.prefix.matched, behavior.prefix.expected),
            (10, 10),
            "seed {value}"
        );
        let counts: Vec<(&str, usize, usize)> = behavior
            .rules
            .iter()
            .map(|result| {
                assert_eq!(result.mismatches, Vec::<String>::new(), "seed {value}");
                (result.rule.name(), result.matched, result.applied)
            })
            .collect();
        // Seventeen steps, plus the initial reading for timing and continuity.
        assert_eq!(
            counts,
            [
                ("step accounting", 17, 17),
                ("step timing", 18, 18),
                ("interaction order", 17, 17),
                ("rejected input", 3, 3),
                ("refused request", refused, refused),
                ("accepted request", accepted, accepted),
                ("state continuity", 18, 18),
            ],
            "seed {value}"
        );

        // Ceilings: 60 requests, 512 bytes per value and 64 KiB of trace.
        assert!(run.checks.len() <= 60);
        let largest = run.checks.iter().map(|check| check.bytes).max().unwrap();
        assert_eq!(
            largest,
            r#"{"state":"unlocked","at":"2024-01-01T12:00:01Z"}"#.len()
        );
        assert!(largest <= 512);
        assert!(run.trace.charged() <= 64 * 1024, "seed {value}");
    }
}

#[test]
fn admitted_but_wrong_results_and_readings_fail_their_named_rules() {
    let demonstration = default_run();
    let good = &replayed(&demonstration).trace;
    let is_output = |i: &Interaction| matches!(i, Interaction::Output(_));
    let is_reading = |i: &Interaction| matches!(i, Interaction::Reading(_));
    for (step, kind, altered, rule, message) in [
        (
            1,
            is_output as fn(&Interaction) -> bool,
            Interaction::Output(ActionResult {
                accepted: true,
                state: LockState::Locked,
            }),
            Rule::AcceptedRequest,
            "step 1 accepted request: Action result expected accepted true with unlocked, observed accepted true with locked",
        ),
        (
            3,
            is_output,
            Interaction::Output(ActionResult {
                accepted: false,
                state: LockState::Locked,
            }),
            Rule::RefusedRequest,
            "step 3 refused request: Action result expected accepted false with unlocked, observed accepted false with locked",
        ),
        (
            2,
            is_reading,
            Interaction::Reading(LockState::Locked),
            Rule::AcceptedRequest,
            "step 2 accepted request: reading expected unlocked, observed locked",
        ),
        (
            5,
            is_reading,
            Interaction::Reading(LockState::Locked),
            Rule::RejectedInput,
            "step 5 rejected input: reading expected unlocked, observed locked",
        ),
    ] {
        let value = match altered {
            Interaction::Output(result) => SampleValue::Output(result),
            Interaction::Reading(state) => SampleValue::Reading(state),
            _ => unreachable!("only results and readings are altered here"),
        };
        assert!(natively_admitted(step, value), "{message}");
        let mut trace = good.clone();
        let index = step as usize - 1;
        let position = interaction_index(&trace.steps[index], kind);
        trace.steps[index].interactions[position] = altered;
        assert_mentions(&failures(&rules(&trace), rule), message);
        assert_no_success(&trace);
    }
}

#[test]
fn removed_duplicated_shifted_and_unchanged_state_events_each_fail_a_named_rule() {
    let demonstration = default_run();
    let good = &replayed(&demonstration).trace;
    let is_event = |i: &Interaction| matches!(i, Interaction::Event(_));

    let mut removed = good.clone();
    let index = interaction_index(&removed.steps[6], is_event);
    removed.steps[6].interactions.remove(index);
    assert_eq!(
        failed_rules(&removed),
        ["accepted request", "state continuity"]
    );
    assert_mentions(
        &failures(&rules(&removed), Rule::AcceptedRequest),
        "step 7 accepted request: Event expected stateChanged locked, observed none",
    );
    assert_no_success(&removed);

    let mut duplicated = good.clone();
    let index = interaction_index(&duplicated.steps[0], is_event);
    let event = duplicated.steps[0].interactions[index].clone();
    duplicated.steps[0].interactions.insert(index, event);
    assert_eq!(
        failed_rules(&duplicated),
        ["accepted request", "state continuity"]
    );
    assert_mentions(
        &failures(&rules(&duplicated), Rule::AcceptedRequest),
        "step 1 accepted request: Event expected stateChanged unlocked, observed stateChanged unlocked, stateChanged unlocked",
    );
    assert_no_success(&duplicated);

    // One valid second later is still an admitted Event, but not at its step's instant.
    let shifted_event = StateChanged {
        state: LockState::Unlocked,
        at: SimTime::at_step(10).unwrap(),
    };
    assert!(natively_admitted(9, SampleValue::Event(shifted_event)));
    let mut shifted = good.clone();
    let index = interaction_index(&shifted.steps[8], is_event);
    shifted.steps[8].interactions[index] = Interaction::Event(shifted_event);
    assert_eq!(failed_rules(&shifted), ["step timing"]);
    assert_mentions(
        &failures(&rules(&shifted), Rule::StepTiming),
        "step 9 step timing: Event time expected 2024-01-01T12:00:09Z, observed 2024-01-01T12:00:10Z",
    );
    assert_no_success(&shifted);

    // An already locked request announcing locked again is admitted but invents a change.
    let unchanged_event = StateChanged {
        state: LockState::Locked,
        at: SimTime::at_step(8).unwrap(),
    };
    assert!(natively_admitted(8, SampleValue::Event(unchanged_event)));
    let mut invented = good.clone();
    invented.steps[7]
        .interactions
        .insert(2, Interaction::Event(unchanged_event));
    assert_eq!(
        failed_rules(&invented),
        ["accepted request", "state continuity"]
    );
    assert_mentions(
        &failures(&rules(&invented), Rule::AcceptedRequest),
        "step 8 accepted request: Event expected none, observed stateChanged locked",
    );
    assert_no_success(&invented);
}

#[test]
fn missing_duplicate_extra_and_reordered_steps_fail_step_accounting() {
    let demonstration = default_run();
    let good = &replayed(&demonstration).trace;

    let mut missing = good.clone();
    missing.steps.remove(4);
    let mut duplicate = good.clone();
    duplicate.steps.insert(3, good.steps[2].clone());
    let mut extra = good.clone();
    extra.steps.push(Step {
        number: 18,
        at: SimTime::at_step(18).unwrap(),
        ..good.steps[16].clone()
    });
    let mut reordered = good.clone();
    reordered.steps.swap(1, 2);
    for (trace, message) in [
        (
            &missing,
            "step 5 step accounting: recorded steps expected 1, observed 0",
        ),
        (
            &duplicate,
            "step 3 step accounting: recorded steps expected 1, observed 2",
        ),
        (
            &extra,
            "step 18 step accounting: authored command expected, observed none",
        ),
        (
            &reordered,
            "trace step accounting: step order expected increasing, observed 1, 3, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17",
        ),
    ] {
        assert_mentions(&failures(&rules(trace), Rule::StepAccounting), message);
        assert_no_success(trace);
    }
}

#[test]
fn a_misordered_interaction_fails_the_order_rule() {
    let demonstration = default_run();
    let mut trace = replayed(&demonstration).trace.clone();
    trace.steps[0].interactions.swap(1, 2);
    assert_eq!(failed_rules(&trace), ["interaction order"]);
    assert_mentions(
        &failures(&rules(&trace), Rule::InteractionOrder),
        "step 1 interaction order: sequence expected input, optional output, optional Event, reading, observed input, Event, output, reading",
    );
    assert_no_success(&trace);
}

#[test]
fn expectations_come_from_authored_commands_and_tracked_state_not_the_trace() {
    // A trace that is consistent with an unlocked start still fails, because the authored
    // initial state is locked and the oracle never adopts observed state as its expectation.
    let demonstration = default_run();
    let mut trace = replayed(&demonstration).trace.clone();
    trace.initial.state = LockState::Unlocked;
    trace.steps[0]
        .interactions
        .retain(|i| !matches!(i, Interaction::Event(_)));
    let results = rules(&trace);
    assert_mentions(
        &failures(&results, Rule::StateContinuity),
        "initial reading state continuity: state expected locked, observed unlocked",
    );
    assert_mentions(
        &failures(&results, Rule::AcceptedRequest),
        "step 1 accepted request: Event expected stateChanged unlocked, observed none",
    );
    assert_no_success(&trace);
}

#[test]
fn a_wrong_native_admission_is_followed_for_invocation_but_still_fails() {
    // This model admits the three-character credential, so the simulator invokes abc.
    let mut model: Value = serde_json::from_slice(RICH_MODEL).unwrap();
    model["sdfObject"]["Lock"]["sdfAction"]["authorize"]["sdfInputData"]["properties"]["credential"]
        ["minLength"] = json!(3);
    let model = serde_json::to_vec(&model).unwrap();
    let demonstration = demonstrate(&model, Seed::DEFAULT);
    assert_eq!(demonstration.model_summary.mismatches, Vec::<String>::new());
    let run = replayed(&demonstration);
    assert!(run.trace.steps[3].invoked);
    assert_mentions(
        &demonstration.native.as_ref().unwrap().summary.mismatches,
        "step 4 input: admission expected rejected, observed admitted",
    );
    let behavior = demonstration.behavior.as_ref().unwrap();
    assert_mentions(
        &failures(&behavior.rules, Rule::RejectedInput),
        "step 4 rejected input: invocation expected not invoked, observed invoked",
    );
    assert!(!demonstration.passed());
    assert!(rendered(&demonstration).ends_with("Result: failed\n"));
}

#[test]
fn the_oracle_judges_any_authored_command_list() {
    let request = |token, operation| Command {
        credential: Credential::Token(token),
        operation: Some(operation),
    };
    let commands = [
        request("demo", Operation::Lock),
        request("nope", Operation::Unlock),
        request("demo", Operation::Unlock),
        request("demo", Operation::Unlock),
    ];
    let run = replay(NativeChecker::new(RICH_MODEL, Bounds::LOCK_DEMO), &commands);
    assert_eq!(run.halt, None);
    let counts: Vec<(&str, usize, usize, bool)> = oracle::assess(&commands, &run.trace)
        .iter()
        .map(|r| (r.rule.name(), r.matched, r.applied, r.passed()))
        .collect();
    // Locking an already locked lock and the repeated unlock change nothing, the nope request
    // is refused and only the first unlock changes the state.
    assert_eq!(
        counts,
        [
            ("step accounting", 4, 4, true),
            ("step timing", 5, 5, true),
            ("interaction order", 4, 4, true),
            ("rejected input", 0, 0, true),
            ("refused request", 1, 1, true),
            ("accepted request", 3, 3, true),
            ("state continuity", 5, 5, true),
        ]
    );

    // A command form without an authored admission cannot be judged and fails accounting.
    let unknown = [request("abcd", Operation::Lock)];
    let run = replay(NativeChecker::new(RICH_MODEL, Bounds::LOCK_DEMO), &unknown);
    assert_mentions(
        &failures(&oracle::assess(&unknown, &run.trace), Rule::StepAccounting),
        "step 1 step accounting: authored admission expected for its command, observed none",
    );
}

fn seed(value: u32) -> Seed {
    Seed::parse(OsString::from(value.to_string()).as_os_str()).unwrap()
}

#[test]
fn eight_xorshift32_commands_follow_the_literal_prefix_for_each_seed() {
    use Operation::{Lock, Unlock};
    let (demo, nope) = ("demo", "nope");
    // Worked independently of this code: xorshift32 states 270369, 67634689, ... for seed 1.
    for (value, generated) in [
        (
            1,
            [
                (demo, Unlock),
                (demo, Unlock),
                (demo, Unlock),
                (nope, Unlock),
                (demo, Unlock),
                (demo, Lock),
                (nope, Lock),
                (nope, Lock),
            ],
        ),
        (
            2,
            [
                (nope, Lock),
                (nope, Lock),
                (nope, Lock),
                (nope, Lock),
                (nope, Lock),
                (nope, Unlock),
                (demo, Unlock),
                (nope, Lock),
            ],
        ),
        (
            4294967295,
            [
                (nope, Unlock),
                (nope, Unlock),
                (nope, Unlock),
                (nope, Lock),
                (demo, Unlock),
                (demo, Lock),
                (demo, Lock),
                (nope, Unlock),
            ],
        ),
    ] {
        let commands = scenario::commands(seed(value));
        assert_eq!(commands.len(), 17, "seed {value}");
        assert_eq!(commands[..9], scenario::LITERAL_PREFIX, "seed {value}");
        let expected: Vec<Command> = generated
            .iter()
            .map(|(token, operation)| Command {
                credential: Credential::Token(token),
                operation: Some(*operation),
            })
            .collect();
        assert_eq!(commands[9..], expected, "seed {value}");
    }
    // The specification fixes the first generated command of seeds 1 and 2.
    let first = |value| scenario::commands(seed(value))[9];
    assert_eq!(
        (first(1).credential, first(1).operation),
        (Credential::Token("demo"), Some(Unlock))
    );
    assert_eq!(
        (first(2).credential, first(2).operation),
        (Credential::Token("nope"), Some(Lock))
    );
}

#[test]
fn seeds_are_unsigned_decimal_integers_from_one_through_u32_max() {
    for (text, value) in [
        ("1", 1),
        ("2", 2),
        ("4294967295", 4294967295),
        // Leading zeros still spell an unsigned decimal integer.
        ("01", 1),
        ("007", 7),
        ("0004294967295", 4294967295),
    ] {
        assert_eq!(
            Seed::parse(OsString::from(text).as_os_str()).map(Seed::get),
            Ok(value),
            "{text:?}"
        );
    }
    for text in [
        "",
        "0",
        "00",
        "4294967296",
        // The final digit overflows the addition rather than the multiplication.
        "4294967299",
        "18446744073709551617",
        "-1",
        "+1",
        " 1",
        "1 ",
        "1_000",
        "0x10",
        "1e3",
        "abc",
        "\u{ff11}",
    ] {
        let refused = Seed::parse(OsString::from(text).as_os_str()).unwrap_err();
        assert!(
            refused.contains("1 through 4294967295"),
            "{text:?}: {refused}"
        );
    }
    // The output names the canonical seed, so a padded argument reproduces the same run.
    let padded = demo::arguments([OsString::from("007")].into_iter()).unwrap();
    let text = rendered(&demonstrate(RICH_MODEL, padded));
    assert!(
        text.starts_with("Door-lock demonstration lock-demo/1 with seed 7\n"),
        "{text}"
    );
}

fn natively_admitted_input(step: u32, bytes: &[u8]) -> bool {
    let mut checker = NativeChecker::new(RICH_MODEL, Bounds::LOCK_DEMO);
    checker.check(SampleId::new(step, Role::Input), bytes) == Ok(Some(Admission::Admitted))
}

#[test]
fn swapped_generated_inputs_stay_admitted_but_fail_step_accounting() {
    let demonstration = default_run();
    let mut trace = replayed(&demonstration).trace.clone();
    // Seed 1 generates nope unlock at step 13 and demo unlock at step 14.
    let (left, right) = trace.steps.split_at_mut(13);
    std::mem::swap(&mut left[12].interactions[0], &mut right[0].interactions[0]);
    for (number, index) in [(13, 12), (14, 13)] {
        let input = trace.steps[index].inputs().next().unwrap();
        assert!(natively_admitted_input(number, input), "step {number}");
    }
    assert_eq!(failed_rules(&trace), ["step accounting"]);
    let failures = failures(&rules(&trace), Rule::StepAccounting);
    assert_mentions(
        &failures,
        r#"step 13 step accounting: Action input expected {"credential":"nope","operation":"unlock"}, observed {"credential":"demo","operation":"unlock"}"#,
    );
    assert_mentions(
        &failures,
        r#"step 14 step accounting: Action input expected {"credential":"demo","operation":"unlock"}, observed {"credential":"nope","operation":"unlock"}"#,
    );
    assert_no_success(&trace);
}

#[test]
fn recorded_inputs_are_compared_by_meaning_rather_than_byte_layout() {
    let demonstration = default_run();
    let good = &replayed(&demonstration).trace;
    // Step 10 is a generated demo unlock, so only the oracle judges its input.
    let mut reordered = good.clone();
    let bytes = br#"{ "operation": "unlock", "credential": "demo" }"#.to_vec();
    reordered.steps[9].interactions[0] = Interaction::Input(bytes);
    assert_eq!(failed_rules(&reordered), Vec::<&str>::new());

    let mut extended = good.clone();
    let bytes = br#"{"credential":"demo","operation":"unlock","note":"extra"}"#;
    assert!(natively_admitted_input(10, bytes));
    extended.steps[9].interactions[0] = Interaction::Input(bytes.to_vec());
    assert_eq!(failed_rules(&extended), ["step accounting"]);
    assert_mentions(
        &failures(&rules(&extended), Rule::StepAccounting),
        r#"step 10 step accounting: Action input expected {"credential":"demo","operation":"unlock"}, observed {"credential":"demo","note":"extra","operation":"unlock"}"#,
    );
    assert_no_success(&extended);
}

//! The demonstration lock and the bounded replay runner.
//! Native admission decides invocation. The lock never inspects SDF descriptions.
use crate::native::{NativeChecker, Role, SampleCheck, SampleId, SampleValue, encode};
use crate::scenario::{Command, Credential, INITIAL_STATE, LockState, Operation, SimTime};
use crate::trace::{ActionResult, Interaction, Reading, StateChanged, Trace};
use esdieff::Admission;

const ACCEPTED_CREDENTIAL: Credential = Credential::Token("demo");

/// The authored demonstration policy. Only the demo token is accepted, and an accepted
/// request emits one stateChanged Event only when the state changes.
struct Lock {
    state: LockState,
}
impl Lock {
    fn invoke(
        &mut self,
        command: &Command,
        at: SimTime,
    ) -> Result<(ActionResult, Option<StateChanged>), &'static str> {
        let operation = command
            .operation
            .ok_or("admitted input has no operation to invoke")?;
        if command.credential != ACCEPTED_CREDENTIAL {
            let refused = ActionResult {
                accepted: false,
                state: self.state,
            };
            return Ok((refused, None));
        }
        let requested = match operation {
            Operation::Lock => LockState::Locked,
            Operation::Unlock => LockState::Unlocked,
        };
        let event = (requested != self.state).then_some(StateChanged {
            state: requested,
            at,
        });
        self.state = requested;
        let accepted = ActionResult {
            accepted: true,
            state: requested,
        };
        Ok((accepted, event))
    }
}

/// A finished or stopped replay. A stopped run keeps the trace and results observed so far.
pub struct Run {
    pub trace: Trace,
    pub checks: Vec<SampleCheck>,
    pub halt: Option<String>,
}

/// Replay commands from the lock-demo/1 initial conditions, spending the checker's allowances.
pub fn replay(mut checker: NativeChecker<'_>, commands: &[Command]) -> Run {
    let initial = Reading {
        at: SimTime::EPOCH,
        state: INITIAL_STATE,
    };
    let mut trace = Trace::new(initial, checker.bounds().trace_bytes);
    let halt = simulate(&mut checker, &mut trace, commands).err();
    Run {
        trace,
        checks: checker.into_checks(),
        halt,
    }
}

fn simulate(
    checker: &mut NativeChecker<'_>,
    trace: &mut Trace,
    commands: &[Command],
) -> Result<(), String> {
    let mut lock = Lock {
        state: trace.initial.state,
    };
    checker.check_value(0, &SampleValue::Reading(lock.state))?;
    for (index, command) in commands.iter().enumerate() {
        let step = index
            .checked_add(1)
            .and_then(|step| u32::try_from(step).ok())
            .ok_or("step count overflow")?;
        let at = SimTime::at_step(step).ok_or("simulated clock range exhausted")?;
        let id = SampleId::new(step, Role::Input);
        let value = SampleValue::Input(command);
        let mut input = trace.input_buffer(checker.encoded_len(id, &value)?)?;
        encode(&value, &mut input)?;
        let decision = match checker.check(id, &input)? {
            Some(Admission::Admitted) => Ok(true),
            Some(Admission::Rejected) => Ok(false),
            Some(_) => Err(format!("{id}: undetermined admission, not invoked")),
            None => Err(format!("{id}: no usable native result, not invoked")),
        };
        trace.begin(step, at, decision == Ok(true))?;
        trace.record(Interaction::Input(input))?;
        if decision? {
            let (result, event) = lock
                .invoke(command, at)
                .map_err(|reason| format!("{id}: {reason}"))?;
            checker.check_value(step, &SampleValue::Output(result))?;
            trace.record(Interaction::Output(result))?;
            if let Some(event) = event {
                checker.check_value(step, &SampleValue::Event(event))?;
                trace.record(Interaction::Event(event))?;
            }
        }
        checker.check_value(step, &SampleValue::Reading(lock.state))?;
        trace.record(Interaction::Reading(lock.state))?;
    }
    Ok(())
}

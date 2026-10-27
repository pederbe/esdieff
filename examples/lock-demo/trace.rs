//! The Simulation trace: plain observed data that an independent oracle can read.
//! Each step keeps its interactions in observed order, so missing, repeated and misordered
//! outputs or Events remain representable and detectable.
use crate::scenario::{LockState, SimTime};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reading {
    pub at: SimTime,
    pub state: LockState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionResult {
    pub accepted: bool,
    pub state: LockState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateChanged {
    pub state: LockState,
    pub at: SimTime,
}

/// One observed value within a step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Interaction {
    /// The exact Action input bytes that were checked.
    Input(Vec<u8>),
    Output(ActionResult),
    Event(StateChanged),
    Reading(LockState),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    pub number: u32,
    pub at: SimTime,
    pub invoked: bool,
    pub interactions: Vec<Interaction>,
}
impl Step {
    pub fn inputs(&self) -> impl Iterator<Item = &[u8]> {
        self.interactions.iter().filter_map(|i| match i {
            Interaction::Input(bytes) => Some(bytes.as_slice()),
            _ => None,
        })
    }
    pub fn outputs(&self) -> impl Iterator<Item = ActionResult> + '_ {
        self.interactions.iter().filter_map(|i| match i {
            Interaction::Output(result) => Some(*result),
            _ => None,
        })
    }
    pub fn events(&self) -> impl Iterator<Item = StateChanged> + '_ {
        self.interactions.iter().filter_map(|i| match i {
            Interaction::Event(event) => Some(*event),
            _ => None,
        })
    }
    pub fn readings(&self) -> impl Iterator<Item = LockState> + '_ {
        self.interactions.iter().filter_map(|i| match i {
            Interaction::Reading(state) => Some(*state),
            _ => None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trace {
    pub initial: Reading,
    pub steps: Vec<Step>,
    allowance: usize,
    remaining: usize,
}
impl Trace {
    pub fn new(initial: Reading, allowance: usize) -> Self {
        Self {
            initial,
            steps: Vec::new(),
            allowance,
            remaining: allowance,
        }
    }

    /// The storage charged so far against the trace allowance.
    pub fn charged(&self) -> usize {
        self.allowance - self.remaining
    }

    /// Charge an input buffer before allocating it.
    pub fn input_buffer(&mut self, len: usize) -> Result<Vec<u8>, String> {
        charge(&mut self.remaining, len)?;
        Ok(Vec::with_capacity(len))
    }

    /// Open the next step. Later interactions belong to it.
    pub fn begin(&mut self, number: u32, at: SimTime, invoked: bool) -> Result<(), String> {
        grow(&mut self.steps, &mut self.remaining)?;
        self.steps.push(Step {
            number,
            at,
            invoked,
            interactions: Vec::new(),
        });
        Ok(())
    }

    /// Append an interaction to the open step in the order it was observed.
    pub fn record(&mut self, interaction: Interaction) -> Result<(), String> {
        let step = self.steps.last_mut().ok_or("no open trace step")?;
        grow(&mut step.interactions, &mut self.remaining)?;
        step.interactions.push(interaction);
        Ok(())
    }
}

/// Charge any growth before it happens. The allowance is never replenished.
fn grow<T>(items: &mut Vec<T>, remaining: &mut usize) -> Result<(), String> {
    if items.len() == items.capacity() {
        let grown = items.capacity().max(4);
        let bytes = grown
            .checked_mul(std::mem::size_of::<T>())
            .ok_or("trace slot size overflow")?;
        charge(remaining, bytes)?;
        items.reserve_exact(grown);
    }
    Ok(())
}

fn charge(remaining: &mut usize, bytes: usize) -> Result<(), String> {
    *remaining = remaining
        .checked_sub(bytes)
        .ok_or("trace storage allowance exhausted")?;
    Ok(())
}

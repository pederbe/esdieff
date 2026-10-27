//! The lock-demo/1 replay contract: identity, seed, simulated clock, initial state and commands.
//! These are plain authored inputs. They hold no simulator state or transition rules.
use std::{ffi::OsStr, fmt, num::NonZeroU32};

pub const REPLAY: &str = "lock-demo/1";

/// Lock states this demonstration stores. The model's jammed alternative is never produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockState {
    Locked,
    Unlocked,
}
impl LockState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Locked => "locked",
            Self::Unlocked => "unlocked",
        }
    }
}

pub const INITIAL_STATE: LockState = LockState::Locked;

/// The generator seed, an unsigned decimal integer from 1 through 4294967295.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Seed(NonZeroU32);
impl Seed {
    pub const DEFAULT: Self = Self(NonZeroU32::MIN);

    /// Accept a nonempty string of ASCII digits, leading zeros included, whose value lies in
    /// 1 through 4294967295. Signs, spaces, prefixes and other digits are refused. Checked
    /// arithmetic stops at the first digit that would overflow.
    pub fn parse(text: &OsStr) -> Result<Self, String> {
        let refused = || {
            format!(
                "seed {:?} is not a decimal integer from 1 through 4294967295",
                text.to_string_lossy()
            )
        };
        let digits = text
            .to_str()
            .filter(|digits| !digits.is_empty())
            .ok_or_else(refused)?;
        let mut value = 0u32;
        for byte in digits.bytes() {
            let digit = char::from(byte).to_digit(10).ok_or_else(refused)?;
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
                .ok_or_else(refused)?;
        }
        NonZeroU32::new(value).map(Self).ok_or_else(refused)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

/// Whole seconds after the virtual epoch 2024-01-01T12:00:00Z. Step i occurs at second i.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimTime(u32);
impl SimTime {
    pub const EPOCH: Self = Self(0);
    /// Times stay within the epoch's calendar day, which no bounded run approaches.
    pub fn at_step(step: u32) -> Option<Self> {
        (step < 12 * 3600).then_some(Self(step))
    }
    pub fn seconds(self) -> u32 {
        self.0
    }
}
impl fmt::Display for SimTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let clock = 12 * 3600 + self.0;
        write!(
            f,
            "2024-01-01T{:02}:{:02}:{:02}Z",
            clock / 3600,
            clock % 3600 / 60,
            clock % 60
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Lock,
    Unlock,
}
impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lock => "lock",
            Self::Unlock => "unlock",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Credential {
    Token(&'static str),
    Null,
}

/// One authorize request. A missing operation is a deliberately malformed command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Command {
    pub credential: Credential,
    pub operation: Option<Operation>,
}

const fn request(token: &'static str, operation: Operation) -> Command {
    Command {
        credential: Credential::Token(token),
        operation: Some(operation),
    }
}

/// The nine literal steps. They consume no random state.
pub const LITERAL_PREFIX: [Command; 9] = [
    request("demo", Operation::Unlock),
    request("demo", Operation::Unlock),
    request("nope", Operation::Lock),
    request("abc", Operation::Lock),
    Command {
        credential: Credential::Null,
        operation: Some(Operation::Lock),
    },
    Command {
        credential: Credential::Token("demo"),
        operation: None,
    },
    request("demo", Operation::Lock),
    request("demo", Operation::Lock),
    request("demo", Operation::Unlock),
];

pub const GENERATED_COMMANDS: usize = 8;

/// One xorshift32 advance on the low 32 bits. This is a reproducible command source, not a
/// statistical or security generator.
fn xorshift32(mut state: u32) -> u32 {
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    state
}

/// The nine literal steps followed by eight commands generated from the seed. Each generated
/// command advances the state once. Bit 0 selects unlock over lock and bit 1 selects nope
/// over demo.
pub fn commands(seed: Seed) -> Vec<Command> {
    let mut commands = Vec::with_capacity(LITERAL_PREFIX.len() + GENERATED_COMMANDS);
    commands.extend_from_slice(&LITERAL_PREFIX);
    let mut state = seed.get();
    for _ in 0..GENERATED_COMMANDS {
        state = xorshift32(state);
        let operation = if state & 1 == 0 {
            Operation::Lock
        } else {
            Operation::Unlock
        };
        let token = if state & 2 == 0 { "demo" } else { "nope" };
        commands.push(request(token, operation));
    }
    commands
}

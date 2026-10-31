//! Follows ADR0014: Bounded partial ECMAScript pattern capability,
//! ADR0028: Bounded effective model pattern syntax, and ADR0045: One pattern
//! engine per trace or generation request.
use crate::AssessmentStatus;

pub(crate) const AREA: &str = "pattern-syntax/1";
pub(crate) const REPORT_RESERVE: usize = 171;
pub(crate) const WORKER_STACK_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(crate) enum Condition {
    Invalid,
    Unsupported,
    Unavailable,
    Limit,
}
impl Condition {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Invalid => "pattern.syntax",
            Self::Unsupported => "pattern.unsupported",
            Self::Unavailable => "pattern.unavailable",
            Self::Limit => "operation.limit",
        }
    }
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Invalid => {
                "The effective pattern has invalid Unicode ECMAScript regular-expression syntax"
            }
            Self::Unsupported => {
                "Effective pattern syntax requires a capability outside the qualified parser subset"
            }
            Self::Unavailable => {
                "Effective pattern syntax requires unavailable text, role or interpretation"
            }
            Self::Limit => {
                "Effective pattern syntax reached its finite operation or parser capacity"
            }
        }
    }
    pub(crate) fn basis_id(self) -> Option<&'static str> {
        match self {
            Self::Invalid => None,
            Self::Unavailable => Some("pattern-establishment"),
            Self::Unsupported | Self::Limit => Some("pattern-syntax-capability"),
        }
    }
}

#[derive(Default)]
pub(crate) struct Scope {
    pub(crate) enabled: bool,
    pub(crate) applicable: bool,
    pub(crate) unknown: bool,
}
impl Scope {
    pub(crate) fn status(&self) -> AssessmentStatus {
        if self.unknown {
            AssessmentStatus::Undetermined
        } else if self.applicable {
            AssessmentStatus::Assessed
        } else {
            AssessmentStatus::NotApplicable
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Verdict {
    Valid,
    Invalid,
    Unsupported,
    Limit,
}
#[derive(Default)]
pub(crate) struct MatchingWork {
    stack_charged: bool,
    heap_charged: bool,
    /// Whether this allowance has spent the poll a new engine makes on its first call.
    first_poll_charged: bool,
    worker: Option<MatchWorker>,
    remaining_interrupts: Option<usize>,
    remaining_time: Option<std::time::Duration>,
    pub(crate) model_patterns: Vec<ModelPattern>,
    pub(crate) reported_models: usize,
}
pub(crate) struct ModelPattern {
    pub(crate) source: crate::SourceId,
    pub(crate) node: usize,
    pub(crate) invalid: bool,
    pub(crate) reported: bool,
}
impl MatchingWork {
    /// Admit independent model-condition storage before proving syntax. Its
    /// identity is the actual effective context, never a retained diagnostic.
    pub(crate) fn model_pattern(
        &mut self,
        source: crate::SourceId,
        node: usize,
        work: &mut usize,
        allocation: &mut usize,
    ) -> Option<usize> {
        // Prepay a later independent-condition drain as well as this lookup.
        if !crate::allowance::charge(work, self.model_patterns.len() + 1) {
            return None;
        }
        for (index, known) in self.model_patterns.iter().enumerate() {
            if !crate::allowance::charge(work, 1) {
                return None;
            }
            if known.source == source && known.node == node {
                return Some(index);
            }
        }
        if !crate::allowance::charge(work, 3)
            || crate::allowance::admit(&mut self.model_patterns, allocation).is_err()
        {
            return None;
        }
        let index = self.model_patterns.len();
        self.model_patterns.push(ModelPattern {
            source,
            node,
            invalid: false,
            reported: false,
        });
        Some(index)
    }
    /// Lend this allowance's worker, with its engine, to `borrower`, a fresh allowance for
    /// an independent check, so successive checks can reuse a live engine. A stopped worker
    /// is not lent, and the independent check may start another. Only the worker moves.
    /// Each allowance keeps its own counters and pays its own worker stack and engine heap.
    pub(crate) fn lend_worker(&mut self, borrower: &mut Self) {
        if self.worker.as_ref().is_some_and(MatchWorker::running) {
            borrower.worker = self.worker.take();
        }
    }
    /// Take back the worker lent to `borrower`, or the one it started. A stopped worker is
    /// never lent, so the next check starts its own.
    pub(crate) fn reclaim_worker(&mut self, borrower: &mut Self) {
        let returned = borrower.worker.take().filter(MatchWorker::running);
        if self.worker.is_none() {
            self.worker = returned;
        }
    }
}
pub(crate) enum MatchVerdict {
    Matched,
    Mismatched,
    Invalid,
    Unsupported,
    Limit,
}

/// Keep syntax, engine ownership and finite capacities behind one internal interface.
/// Callers retain only domain conclusions and original operation counters.
pub(crate) fn matches(
    pattern: &str,
    subject: &str,
    limits: &crate::Limits,
    budget: &mut crate::allowance::OperationBudget,
) -> MatchVerdict {
    run(pattern, Some(subject), limits, budget)
}

/// Establish that the matching implementation can compile the modeled pattern.
/// No subject is supplied and no search for an admitting value is performed.
pub(crate) fn matching_capability(
    pattern: &str,
    limits: &crate::Limits,
    budget: &mut crate::allowance::OperationBudget,
) -> Verdict {
    match run(pattern, None, limits, budget) {
        MatchVerdict::Matched => Verdict::Valid,
        MatchVerdict::Invalid => Verdict::Invalid,
        MatchVerdict::Unsupported => Verdict::Unsupported,
        MatchVerdict::Limit | MatchVerdict::Mismatched => Verdict::Limit,
    }
}

fn run(
    pattern: &str,
    subject: Option<&str>,
    limits: &crate::Limits,
    budget: &mut crate::allowance::OperationBudget,
) -> MatchVerdict {
    let mut matching = std::mem::take(&mut budget.patterns);
    let result = match_inner(pattern, subject, false, limits, budget, &mut matching);
    budget.patterns = matching;
    result
}
#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn match_inner(
    pattern: &str,
    subject: Option<&str>,
    syntax_only: bool,
    limits: &crate::Limits,
    budget: &mut crate::allowance::OperationBudget,
    matching: &mut MatchingWork,
) -> MatchVerdict {
    if !crate::allowance::charge(&mut budget.work, 2) {
        return MatchVerdict::Limit;
    }
    if !matching.stack_charged {
        if !crate::allowance::charge(&mut budget.allocation, WORKER_STACK_BYTES) {
            return MatchVerdict::Limit;
        }
        matching.stack_charged = true;
    }
    // Own request text before sending it to the operation's joined worker.
    if pattern.len() > limits.pattern_bytes {
        return MatchVerdict::Limit;
    }
    let subject_bytes = subject.map_or(0, str::len);
    let subject = subject.filter(|_| subject_bytes <= limits.pattern_subject);
    let text_bytes = pattern.len().saturating_add(subject.map_or(0, str::len));
    if !crate::allowance::charge(&mut budget.work, text_bytes)
        || !crate::allowance::charge(&mut budget.allocation, text_bytes)
    {
        return MatchVerdict::Limit;
    }
    if matching.worker.is_none() {
        matching.worker = MatchWorker::new();
    }
    let Some(worker) = matching.worker.as_mut() else {
        return MatchVerdict::Limit;
    };
    let request = MatchRequest {
        syntax_only,
        pattern: pattern.to_owned(),
        subject: subject.map(str::to_owned),
        subject_bytes,
        limits: limits.clone(),
        work: budget.work,
        allocation: budget.allocation,
        polls: matching
            .remaining_interrupts
            .unwrap_or(limits.pattern_interrupts),
        duration: matching.remaining_time.unwrap_or(limits.pattern_deadline),
        heap_charged: matching.heap_charged,
        first_poll_charged: matching.first_poll_charged,
    };
    let reply = worker
        .requests
        .as_ref()
        .and_then(|requests| requests.send(request).ok())
        .and_then(|()| worker.results.recv().ok());
    let Some((result, request)) = reply else {
        // Keep the stopped worker, so this allowance's later matches also reach a limit.
        worker.requests = None;
        return MatchVerdict::Limit;
    };
    budget.work = request.work;
    budget.allocation = request.allocation;
    matching.remaining_interrupts = Some(request.polls);
    matching.remaining_time = Some(request.duration);
    matching.heap_charged = request.heap_charged;
    matching.first_poll_charged = request.first_poll_charged;
    result
}

struct MatchRequest {
    syntax_only: bool,
    pattern: String,
    subject: Option<String>,
    subject_bytes: usize,
    limits: crate::Limits,
    work: usize,
    allocation: usize,
    polls: usize,
    duration: std::time::Duration,
    heap_charged: bool,
    first_poll_charged: bool,
}
struct MatchWorker {
    requests: Option<std::sync::mpsc::SyncSender<MatchRequest>>,
    results: std::sync::mpsc::Receiver<(MatchVerdict, MatchRequest)>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl MatchWorker {
    /// Whether the worker still takes requests. A stopped one stays with its allowance.
    fn running(&self) -> bool {
        self.requests.is_some()
    }
    fn new() -> Option<Self> {
        let (requests, incoming) = std::sync::mpsc::sync_channel::<MatchRequest>(0);
        let (outgoing, results) = std::sync::mpsc::sync_channel(0);
        let thread = std::thread::Builder::new()
            .stack_size(WORKER_STACK_BYTES)
            .spawn(move || {
                let mut engine = None;
                while let Ok(mut request) = incoming.recv() {
                    let verdict = request.run(&mut engine);
                    if outgoing.send((verdict, request)).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Self {
            requests: Some(requests),
            results,
            thread: Some(thread),
        })
    }
}
impl Drop for MatchWorker {
    fn drop(&mut self) {
        // Disconnect first so an idle worker exits before we join it. Engine
        // objects are destroyed on their owning bounded stack.
        drop(self.requests.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl MatchRequest {
    fn run(&mut self, engine: &mut Option<Engine>) -> MatchVerdict {
        let request = self;
        let limits = &request.limits;
        let mut meter = Meter {
            work: &mut request.work,
            allocation: &mut request.allocation,
        };
        match syntax(&request.pattern, limits, &mut meter) {
            Ok(Verdict::Valid) => {}
            Err(Verdict::Valid) => return MatchVerdict::Unsupported,
            Ok(Verdict::Invalid) | Err(Verdict::Invalid) => return MatchVerdict::Invalid,
            Ok(Verdict::Unsupported) | Err(Verdict::Unsupported) => {
                return MatchVerdict::Unsupported;
            }
            Ok(Verdict::Limit) | Err(Verdict::Limit) => return MatchVerdict::Limit,
        }
        if request.syntax_only {
            return MatchVerdict::Matched;
        }
        let subject_bytes = request.subject_bytes;
        if subject_bytes > limits.pattern_subject {
            return MatchVerdict::Limit;
        }
        let capacity = match guards(&request.pattern, limits, &mut meter) {
            Ok(capacity) => capacity,
            Err(_) => return MatchVerdict::Limit,
        };
        if capacity.captures > 254
            || capacity.name_bytes > 124
            || capacity.named && capacity.alternatives > 256
        {
            return MatchVerdict::Limit;
        }
        let polls = &mut request.polls;
        let duration = &mut request.duration;
        if *polls == 0
            || duration.is_zero()
            || meter
                .visit(subject_bytes + request.pattern.len() + 8)
                .is_err()
        {
            return MatchVerdict::Limit;
        }
        if !request.heap_charged {
            if meter.allocate(limits.pattern_heap).is_err() {
                return MatchVerdict::Limit;
            }
            request.heap_charged = true;
        }
        if engine.is_none() {
            match Engine::new(limits) {
                Ok(created) => *engine = Some(created),
                Err(verdict) => return verdict,
            }
            // The new engine makes this allowance's first poll on its own first call.
            request.first_poll_charged = true;
        }
        let first_poll = !std::mem::replace(&mut request.first_poll_charged, true);
        engine.as_ref().expect("initialized engine").matches(
            &request.pattern,
            request.subject.as_deref(),
            first_poll,
            polls,
            duration,
        )
    }
}

struct Engine {
    // Drop the context before its runtime.
    context: rquickjs::Context,
    runtime: rquickjs::Runtime,
}
impl Engine {
    #[cfg_attr(feature = "hotpath", hotpath::measure(impl_type = "Engine"))]
    fn new(limits: &crate::Limits) -> Result<Self, MatchVerdict> {
        use rquickjs::{Context, Runtime};
        // The probe also protects embeddings that unify allocator features.
        let Ok(probe) = Runtime::new() else {
            return Err(MatchVerdict::Limit);
        };
        probe.set_memory_limit(1);
        if Context::full(&probe).is_ok() {
            return Err(MatchVerdict::Unsupported);
        }
        drop(probe);
        let Ok(runtime) = Runtime::new() else {
            return Err(MatchVerdict::Limit);
        };
        runtime.set_memory_limit(limits.pattern_heap);
        runtime.set_max_stack_size(limits.pattern_stack);
        let Ok(context) = Context::full(&runtime) else {
            return Err(MatchVerdict::Limit);
        };
        Ok(Self { context, runtime })
    }
    fn matches(
        &self,
        pattern: &str,
        subject: Option<&str>,
        first_poll: bool,
        polls: &mut usize,
        duration: &mut std::time::Duration,
    ) -> MatchVerdict {
        use rquickjs::Function;
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        // A reused context need not reach QuickJS's internal polling interval
        // on a short match. Poll at entry as well so tiny matches still spend
        // the original operation allowance.
        *polls -= 1;
        let remaining = Arc::new(AtomicUsize::new(*polls));
        let started = std::time::Instant::now();
        let allowed = *duration;
        let interrupt = {
            let remaining = Arc::clone(&remaining);
            move || {
                started.elapsed() >= allowed
                    || remaining
                        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                            value.checked_sub(1)
                        })
                        .is_err()
            }
        };
        self.runtime
            .set_interrupt_handler(Some(Box::new(interrupt.clone())));
        let result = self.context.with(|ctx| {
            let constructor: Function = match ctx.eval("pattern => new RegExp(pattern, 'u')") {
                Ok(function) => function,
                Err(_) => return MatchVerdict::Limit,
            };
            // A new context polls on its first call, once that helper is compiled. A lent
            // engine made that poll for the allowance that started it, so a borrowing
            // allowance's first match makes it here instead, never earlier.
            if first_poll && interrupt() {
                return MatchVerdict::Limit;
            }
            let regexp: rquickjs::Object = match constructor.call((pattern,)) {
                Ok(regexp) => regexp,
                Err(error) => {
                    return match rquickjs::CaughtError::from_error(&ctx, error) {
                        rquickjs::CaughtError::Exception(exception) => {
                            match exception.message().as_deref() {
                                Some(
                                    "out of memory"
                                    | "stack overflow"
                                    | "too many captures"
                                    | "too many named groups"
                                    | "too many imbricated quantifiers"
                                    | "interrupted",
                                ) => MatchVerdict::Limit,
                                _ => MatchVerdict::Unsupported,
                            }
                        }
                        _ => MatchVerdict::Limit,
                    };
                }
            };
            let Some(subject) = subject else {
                return MatchVerdict::Matched;
            };
            let test: Function = match ctx.eval("(regexp, subject) => regexp.test(subject)") {
                Ok(function) => function,
                Err(_) => return MatchVerdict::Limit,
            };
            match test.call::<_, bool>((regexp, subject)) {
                Ok(true) => MatchVerdict::Matched,
                Ok(false) => MatchVerdict::Mismatched,
                Err(_) => MatchVerdict::Limit,
            }
        });
        // Clear any pending matching exception before reusing the context.
        self.context.with(|ctx| {
            let _ = ctx.catch();
        });
        self.runtime.run_gc();
        *polls = remaining.load(Ordering::Relaxed);
        *duration = duration.saturating_sub(started.elapsed());
        result
    }
}
struct Meter<'a> {
    work: &'a mut usize,
    allocation: &'a mut usize,
}
impl Meter<'_> {
    fn visit(&mut self, bytes: usize) -> Result<(), Verdict> {
        crate::allowance::charge(self.work, bytes)
            .then_some(())
            .ok_or(Verdict::Limit)
    }
    fn allocate(&mut self, bytes: usize) -> Result<(), Verdict> {
        crate::allowance::charge(self.allocation, bytes)
            .then_some(())
            .ok_or(Verdict::Limit)
    }
    fn find<'a>(
        &mut self,
        text: &'a str,
        delimiter: char,
    ) -> Result<Option<(&'a str, &'a str)>, Verdict> {
        self.visit(text.len() + 1)?;
        Ok(text.split_once(delimiter))
    }
    fn decimal<'a>(&mut self, text: &'a str) -> Result<&'a str, Verdict> {
        self.visit(text.len() + 1)?;
        let normalized = text.trim_start_matches('0');
        Ok(if normalized.is_empty() {
            "0"
        } else {
            normalized
        })
    }
    fn compare_decimal(&mut self, a: &str, b: &str) -> Result<std::cmp::Ordering, Verdict> {
        self.visit(a.len() + b.len() + 1)?;
        Ok(a.len().cmp(&b.len()).then_with(|| a.cmp(b)))
    }
}

/// Syntax shares the operation's worker without initializing a matching engine.
pub(crate) fn evaluate(
    pattern: &str,
    limits: &crate::Limits,
    budget: &mut crate::allowance::OperationBudget,
) -> Verdict {
    let mut matching = std::mem::take(&mut budget.patterns);
    let result = match_inner(pattern, None, true, limits, budget, &mut matching);
    budget.patterns = matching;
    match result {
        MatchVerdict::Matched => Verdict::Valid,
        MatchVerdict::Invalid => Verdict::Invalid,
        MatchVerdict::Unsupported => Verdict::Unsupported,
        MatchVerdict::Limit | MatchVerdict::Mismatched => Verdict::Limit,
    }
}

fn syntax(
    pattern: &str,
    limits: &crate::Limits,
    meter: &mut Meter<'_>,
) -> Result<Verdict, Verdict> {
    guards(pattern, limits, meter)?;
    // Count one admitted dependency invocation and supplied UTF-8 bytes.
    // This does not measure the parser/emitter's private instructions or heap.
    meter.visit(pattern.len() + 1)?;
    let flags = regress::Flags {
        unicode: true,
        no_opt: true,
        ..Default::default()
    };
    match regress::Regex::with_flags(pattern, flags) {
        Ok(_) => Ok(Verdict::Valid),
        Err(error) => {
            meter.visit(
                error
                    .text
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(22))
                    .ok_or(Verdict::Limit)?,
            )?;
            if error.text.contains("limit") || error.text.contains("deeply nested") {
                Err(Verdict::Limit)
            } else if established_syntax_error(&error.text) {
                Err(Verdict::Invalid)
            } else {
                Err(Verdict::Unsupported)
            }
        }
    }
}
fn established_syntax_error(error: &str) -> bool {
    matches!(
        error,
        "Unbalanced bracket"
            | "Unbalanced parenthesis"
            | "Invalid atom character"
            | "Invalid character escape"
            | "Invalid unicode escape"
            | "Invalid braced quantifier"
            | "Invalid quantifier"
            | "Quantifier not allowed here"
            | "Invalid character range"
            | "Invalid group modifier"
            | "Invalid property name"
            | "Invalid property escape"
            | "Incomplete escape"
            | "Unterminated escape"
            | "Range values reversed, start char code is greater than end char code."
    )
}
struct Capacity {
    captures: usize,
    name_bytes: usize,
    named: bool,
    alternatives: usize,
}
fn guards(
    pattern: &str,
    limits: &crate::Limits,
    meter: &mut Meter<'_>,
) -> Result<Capacity, Verdict> {
    if pattern.len() > limits.pattern_bytes {
        return Err(Verdict::Limit);
    }
    let mut names: Vec<String> = Vec::new();
    let mut in_class = false;
    let mut escaped = false;
    let mut backreference_name_end = 0;
    let mut depth = 0usize;
    let mut alternatives = 1usize;
    let mut captures = 0usize;
    let mut name_bytes = 0usize;
    let mut index = 0;
    while index < pattern.len() {
        // Charge the maximum UTF-8 scalar decoder visit before reading it.
        meter.visit(5)?;
        let c = pattern[index..].chars().next().expect("remaining scalar");
        let i = index;
        index += c.len_utf8();
        if escaped {
            escaped = false;
            if c == 'u' {
                index = if i < backreference_name_end {
                    name_unicode_escape_end(pattern, index, meter)?
                } else {
                    unicode_escape_end(pattern, index, meter)?
                };
            } else if c == 'k' && !in_class {
                meter.visit(1)?;
                if pattern[index..].starts_with('<') {
                    let name = &pattern[index + 1..];
                    backreference_name_end = index
                        + 1
                        + meter
                            .find(name, '>')?
                            .map_or(name.len(), |(name, _)| name.len());
                }
            }
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        match c {
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => {
                depth += 1;
                if depth > limits.pattern_nesting {
                    return Err(Verdict::Limit);
                }
                meter.visit(8)?;
                if !pattern[index..].starts_with('?')
                    || pattern[i..].starts_with("(?<") && !pattern[i + 3..].starts_with(['=', '!'])
                {
                    captures += 1;
                }
                if let Some(tail) = pattern[i..]
                    .strip_prefix("(?<")
                    .filter(|tail| !tail.starts_with(['=', '!']))
                {
                    let raw = meter
                        .find(tail, '>')?
                        .map(|(name, _)| name)
                        .ok_or(Verdict::Unsupported)?;
                    let decoded = decode_name(raw, meter)?;
                    name_bytes = name_bytes.max(decoded.len());
                    for name in &names {
                        meter.visit(name.len() + decoded.len() + 1)?;
                        if name == &decoded {
                            return Err(Verdict::Unsupported);
                        }
                    }
                    if names.len() == names.capacity() {
                        let old = names.capacity();
                        let new = old.max(4).checked_mul(2).ok_or(Verdict::Limit)?;
                        meter.allocate(
                            new.checked_mul(std::mem::size_of::<String>())
                                .ok_or(Verdict::Limit)?,
                        )?;
                        names.reserve_exact(new - old);
                    }
                    names.push(decoded);
                }
            }
            ')' if !in_class => depth = depth.saturating_sub(1),
            '|' if !in_class => {
                alternatives += 1;
                if alternatives > limits.pattern_alternatives {
                    return Err(Verdict::Limit);
                }
            }
            '{' if !in_class => {
                if let Some((body, _)) = meter.find(&pattern[index..], '}')? {
                    let (lo, hi) = meter
                        .find(body, ',')?
                        .map_or((body, None), |(lo, hi)| (lo, Some(hi)));
                    meter.visit(lo.len() + hi.map_or(0, str::len) + 1)?;
                    if !lo.is_empty()
                        && lo.bytes().all(|c| c.is_ascii_digit())
                        && hi.is_none_or(|v| v.bytes().all(|c| c.is_ascii_digit()))
                    {
                        let lo = meter.decimal(lo)?;
                        let hi = hi
                            .filter(|v| !v.is_empty())
                            .map(|v| meter.decimal(v))
                            .transpose()?;
                        if let Some(hi) = hi
                            && meter.compare_decimal(lo, hi)?.is_gt()
                        {
                            return Err(Verdict::Invalid);
                        }
                        if !meter.compare_decimal(lo, "1000000")?.is_lt()
                            || if let Some(hi) = hi {
                                !meter.compare_decimal(hi, "1000000")?.is_lt()
                            } else {
                                false
                            }
                        {
                            return Err(Verdict::Limit);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(Capacity {
        captures,
        name_bytes,
        named: !names.is_empty(),
        alternatives,
    })
}
fn unicode_escape_end(
    pattern: &str,
    digits_start: usize,
    meter: &mut Meter<'_>,
) -> Result<usize, Verdict> {
    let tail = &pattern[digits_start..];
    meter.visit(1)?;
    if let Some(braced) = tail.strip_prefix('{') {
        let (digits, _) = meter.find(braced, '}')?.ok_or(Verdict::Invalid)?;
        meter.visit(digits.len() + 1)?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Verdict::Invalid);
        }
        Ok(digits_start + digits.len() + 2)
    } else {
        meter.visit(4)?;
        let digits = tail.get(..4).ok_or(Verdict::Invalid)?;
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Verdict::Invalid);
        }
        Ok(digits_start + 4)
    }
}
fn unicode_escape_digits(pattern: &str, start: usize, end: usize) -> &str {
    if pattern[start..].starts_with('{') {
        &pattern[start + 1..end - 1]
    } else {
        &pattern[start..end]
    }
}
fn name_unicode_escape_end(
    pattern: &str,
    start: usize,
    meter: &mut Meter<'_>,
) -> Result<usize, Verdict> {
    let end = unicode_escape_end(pattern, start, meter)?;
    meter.visit(1)?;
    let digits = unicode_escape_digits(pattern, start, end);
    meter.visit(digits.len() + 3)?;
    if digits.trim_start_matches('0').eq_ignore_ascii_case("3e") {
        return Err(Verdict::Invalid);
    }
    Ok(end)
}
fn decode_name(raw: &str, meter: &mut Meter<'_>) -> Result<String, Verdict> {
    // Unicode escape decoding never expands the UTF-8 source length. Pay the
    // complete requested String capacity before decoding or copying any scalar.
    meter.allocate(raw.len())?;
    let mut output = String::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        meter.visit(5)?;
        let c = raw[index..].chars().next().expect("remaining name scalar");
        index += c.len_utf8();
        if c != '\\' {
            meter.visit(c.len_utf8())?;
            output.push(c);
            continue;
        }
        meter.visit(1)?;
        if !raw[index..].starts_with('u') {
            return Err(Verdict::Unsupported);
        }
        index += 1;
        let end = name_unicode_escape_end(raw, index, meter)?;
        meter.visit(1)?;
        let digits = unicode_escape_digits(raw, index, end);
        meter.visit(digits.len() + 1)?;
        let scalar = u32::from_str_radix(digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or(Verdict::Unsupported)?;
        meter.visit(scalar.len_utf8())?;
        output.push(scalar);
        index = end;
    }
    Ok(output)
}

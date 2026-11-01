//! Contextual equality follows ADR0015: Bounded value equality in a resolved data context.
use crate::allowance::{OperationBudget, charge};
use crate::finding::{Basis, Verdict};
use crate::scalar::{Context, ResourceFormat, TemporalFormat};
use crate::{EffectiveJson, Field, JsonKind, NumberWork};

mod comparison {
    use super::*;
    use std::ops::Range;

    use crate::value::{
        BorrowedChildren as OperandChildren, BorrowedValue as Operand, ValueContext,
    };
    #[derive(Clone, Copy)]
    enum ContextCandidate<'a> {
        Ordinary,
        Description(EffectiveJson<'a>),
        Unavailable,
    }

    enum Phase<'a> {
        Start,
        Array {
            left: OperandChildren<'a>,
            right: OperandChildren<'a>,
        },
        Map {
            left: OperandChildren<'a>,
        },
    }
    struct EqualityFrame<'a> {
        left: Operand<'a>,
        right: Operand<'a>,
        candidates: Range<usize>,
        phase: Phase<'a>,
        unknown: bool,
        // Only established containers on this path support a decisive child.
        ancestors: u32,
    }
    struct ProjectionCursor<'a> {
        target: Operand<'a>,
        step: Operand<'a>,
        candidates: Range<usize>,
        depth: usize,
    }
    // One uniqueness invocation retains every candidate, never an operand verdict.
    pub(crate) struct PreparedItems<'a> {
        context: ValueContext<'a>,
        enclosing: Operand<'a>,
        attempted: bool,
        candidates: Option<Vec<ContextCandidate<'a>>>,
    }
    impl<'a> PreparedItems<'a> {
        pub(crate) fn new(context: ValueContext<'a>, enclosing: Operand<'a>) -> Self {
            Self {
                context,
                enclosing,
                attempted: false,
                candidates: None,
            }
        }
    }
    /// The equality context of the latest constant comparison. Consecutive constants
    /// compared with one value, such as every enum or choice alternative, share its
    /// derivation and classify its candidates once for each constant kind.
    struct ConstantSeed<'a> {
        context: ValueContext<'a>,
        value: Operand<'a>,
        /// The candidates retained at the front of `contexts`, or none when the context
        /// could not be derived.
        retained: Option<usize>,
        /// Classification of the retained candidates by the constant's operand slot.
        /// It depends on nothing else, since the value side is the seed's own.
        meanings: [Option<Context>; OPERAND_SLOTS],
    }
    pub(crate) struct EqualityWork<'a> {
        pairs: usize,
        frames: Vec<EqualityFrame<'a>>,
        contexts: Vec<ContextCandidate<'a>>,
        pending: Vec<EffectiveJson<'a>>,
        seed: Option<ConstantSeed<'a>>,
        pub(crate) supporting: u32,
        pub(crate) limitations: u16,
        pub(crate) abstract_use: u8,
        pub(crate) temporal_use: u8,
        pub(crate) format_use: u8,
        pub(crate) format_established: bool,
    }
    pub(crate) struct Decision<'a> {
        pub(crate) verdict: Verdict,
        left: Operand<'a>,
        right: Operand<'a>,
    }
    impl<'a> Decision<'a> {
        pub(crate) fn constant_pair(&self) -> Option<(Operand<'a>, EffectiveJson<'a>)> {
            match (self.left, self.right) {
                (left @ (Operand::Sample(_) | Operand::Model(_)), Operand::Constraint(right)) => {
                    Some((left, right))
                }
                _ => None,
            }
        }
        pub(crate) fn item_pair(&self) -> Option<(Operand<'a>, Operand<'a>)> {
            match (self.left, self.right) {
                (left @ Operand::Sample(_), right @ Operand::Sample(_))
                | (left @ Operand::Model(_), right @ Operand::Model(_)) => Some((left, right)),
                _ => None,
            }
        }
        fn unavailable(left: Operand<'a>, right: Operand<'a>) -> Self {
            Self {
                verdict: Verdict::Unavailable,
                left,
                right,
            }
        }
    }
    impl<'a> EqualityWork<'a> {
        pub(crate) fn new(pairs: usize) -> Self {
            Self {
                pairs,
                frames: Vec::new(),
                contexts: Vec::new(),
                pending: Vec::new(),
                seed: None,
                supporting: 0,
                limitations: 0,
                abstract_use: 0,
                temporal_use: 0,
                format_use: 0,
                format_established: false,
            }
        }
        pub(crate) fn remaining(&self) -> usize {
            self.pairs
        }
        fn begin_pair(&mut self, budget: &mut OperationBudget) -> bool {
            self.supporting = 0;
            self.limitations = 0;
            if !charge(&mut self.pairs, 1) || !charge(&mut budget.work, 1) {
                self.limitations = 1 << 3;
                return false;
            }
            debug_assert!(
                self.frames.is_empty()
                    && self.contexts.len() == self.retained()
                    && self.pending.is_empty()
            );
            true
        }
        fn finish_pair(&mut self) {
            self.frames.clear();
            self.contexts.truncate(self.retained());
            self.pending.clear();
        }
        fn retained(&self) -> usize {
            self.seed
                .as_ref()
                .and_then(|seed| seed.retained)
                .unwrap_or(0)
        }
        fn forget_seed(&mut self) {
            self.seed = None;
            self.contexts.clear();
        }
        pub(crate) fn compare_constant(
            &mut self,
            context: &ValueContext<'a>,
            sample: Operand<'a>,
            constant: EffectiveJson<'a>,
            work: &mut NumberWork,
            budget: &mut OperationBudget,
        ) -> Decision<'a> {
            let left = sample;
            let right = Operand::Constraint(constant);
            let decision = if self.begin_pair(budget) {
                match self.constant_seed(context, sample, budget) {
                    Some(contexts) => self.compare(left, right, contexts, true, work, budget),
                    None => Decision::unavailable(left, right),
                }
            } else {
                Decision::unavailable(left, right)
            };
            if matches!(decision.verdict, Verdict::Unavailable) && self.limitations == 0 {
                self.limitations = if budget.work == 0 {
                    1 << 3
                } else if budget.allocation == 0 {
                    1 << 5
                } else {
                    1
                };
            }
            self.constant_orientation(decision, sample, constant)
        }
        fn constant_orientation(
            &mut self,
            decision: Decision<'a>,
            sample: Operand<'a>,
            constant: EffectiveJson<'a>,
        ) -> Decision<'a> {
            self.finish_pair();
            if decision.constant_pair().is_some() {
                decision
            } else {
                Decision::unavailable(sample, Operand::Constraint(constant))
            }
        }
        pub(crate) fn compare_items(
            &mut self,
            prepared: &mut PreparedItems<'a>,
            left: Operand<'a>,
            right: Operand<'a>,
            work: &mut NumberWork,
            budget: &mut OperationBudget,
        ) -> Decision<'a> {
            let a = left;
            let b = right;
            // Item seeding and candidate copies start from empty scratch storage.
            self.forget_seed();
            let decision = if self.begin_pair(budget) {
                match self.item_candidates(prepared, budget) {
                    Some(contexts) => self.compare(a, b, contexts, false, work, budget),
                    None => Decision::unavailable(a, b),
                }
            } else {
                Decision::unavailable(a, b)
            };
            if matches!(decision.verdict, Verdict::Unavailable) && self.limitations == 0 {
                self.limitations = if budget.work == 0 {
                    1 << 3
                } else if budget.allocation == 0 {
                    1 << 5
                } else {
                    1
                };
            }
            self.item_orientation(decision, left, right)
        }
        fn item_candidates(
            &mut self,
            prepared: &mut PreparedItems<'a>,
            budget: &mut OperationBudget,
        ) -> Option<Range<usize>> {
            if !prepared.attempted {
                prepared.attempted = true;
                let roots = self.seed_for_value(&prepared.context, prepared.enclosing, budget)?;
                if !self.ordinary(
                    roots.clone(),
                    prepared.enclosing,
                    prepared.enclosing,
                    budget,
                ) {
                    return None;
                }
                let projected = self.project(roots, None, budget)?;
                // Pay the actual cache copy and its own retained storage. Scratch
                // ranges never point into this immutable invocation-local Vec.
                if !charge(&mut budget.work, projected.len()) {
                    return None;
                }
                let mut candidates = Vec::new();
                for index in projected {
                    if !retain(&mut candidates, self.contexts[index], budget) {
                        return None;
                    }
                }
                prepared.candidates = Some(candidates);
                self.contexts.clear();
            }
            let candidates = prepared.candidates.as_ref()?;
            // Each pair still classifies the actual operands after copying all
            // alternatives, including explicit unavailable candidates.
            if !charge(&mut budget.work, candidates.len()) {
                return None;
            }
            for &candidate in candidates {
                if !retain(&mut self.contexts, candidate, budget) {
                    return None;
                }
            }
            Some(0..self.contexts.len())
        }
        fn item_orientation(
            &mut self,
            decision: Decision<'a>,
            left: Operand<'a>,
            right: Operand<'a>,
        ) -> Decision<'a> {
            self.finish_pair();
            if decision.item_pair().is_some() {
                decision
            } else {
                Decision::unavailable(left, right)
            }
        }
        fn constant_seed(
            &mut self,
            context: &ValueContext<'a>,
            value: Operand<'a>,
            budget: &mut OperationBudget,
        ) -> Option<Range<usize>> {
            if let Some(seed) = &self.seed
                && same_json(seed.context.description, context.description)
                && same_operand(seed.context.root, context.root)
                && same_operand(seed.value, value)
            {
                let retained = seed.retained;
                if !charge(&mut budget.work, 1) {
                    return None;
                }
                return retained.map(|end| 0..end);
            }
            self.forget_seed();
            // A derived context spans all of `contexts`; finishing the pair drops a failed one.
            let candidates = self.seed_for_value(context, value, budget);
            self.seed = Some(ConstantSeed {
                context: *context,
                value,
                retained: candidates.as_ref().map(|range| range.end),
                meanings: [None; OPERAND_SLOTS],
            });
            candidates
        }
        /// Classify the root candidates of a seeded constant comparison, reusing the
        /// classification of an earlier constant of the same kind and availability.
        fn seed_meaning(
            &mut self,
            candidates: Range<usize>,
            left: Operand<'a>,
            right: Operand<'a>,
            budget: &mut OperationBudget,
        ) -> Context {
            let slot = operand_slot(right);
            if let Some(meaning) = self.seed.as_ref().and_then(|seed| seed.meanings[slot]) {
                return meaning;
            }
            let meaning = self.context(candidates, left, right, budget);
            if budget.work != 0
                && let Some(seed) = &mut self.seed
            {
                seed.meanings[slot] = Some(meaning);
            }
            meaning
        }
        fn seed_for_value(
            &mut self,
            context: &ValueContext<'a>,
            sample: Operand<'a>,
            budget: &mut OperationBudget,
        ) -> Option<Range<usize>> {
            if !charge(&mut budget.work, 1) || !context.valid() {
                return None;
            }
            let selected = context.description;
            if !self.expand(selected, budget) {
                return None;
            }
            let mut cursor = ProjectionCursor {
                target: sample,
                step: sample,
                candidates: 0..self.contexts.len(),
                depth: 0,
            };
            loop {
                if !charge(&mut budget.work, 2) {
                    return None;
                }
                let Some(parent) = cursor.step.parent(context.root).ok()? else {
                    break;
                };
                cursor.depth = cursor.depth.checked_add(1)?;
                cursor.step = parent;
            }
            while cursor.depth != 0 {
                cursor.step = cursor.target;
                for _ in 1..cursor.depth {
                    if !charge(&mut budget.work, 2) {
                        return None;
                    }
                    cursor.step = cursor.step.parent(context.root).ok()??;
                }
                if !charge(&mut budget.work, 2) {
                    return None;
                }
                let parent = cursor.step.parent(context.root).ok()??;
                if !self.ordinary(cursor.candidates.clone(), parent, parent, budget) {
                    return None;
                }
                let projected = match parent.kind() {
                    JsonKind::Array => self.project(cursor.candidates, None, budget)?,
                    JsonKind::Map => {
                        let Field::Present(key) = cursor.step.member_name() else {
                            return None;
                        };
                        self.project(cursor.candidates, Some(key), budget)?
                    }
                    _ => return None,
                };
                if !charge(&mut budget.work, projected.len()) {
                    return None;
                }
                // Seeding has no frames. Only numerical ranges refer to this Vec.
                debug_assert!(self.frames.is_empty());
                self.contexts.copy_within(projected.clone(), 0);
                self.contexts.truncate(projected.len());
                cursor.candidates = 0..projected.len();
                cursor.depth -= 1;
            }
            Some(cursor.candidates)
        }
        fn expand(&mut self, root: EffectiveJson<'a>, budget: &mut OperationBudget) -> bool {
            if !retain(&mut self.pending, root, budget) {
                return false;
            }
            while let Some(json) = self.pending.pop() {
                if !charge(&mut budget.work, 1) {
                    return false;
                }
                if json.n().unavailable.is_some()
                    || json.n().donor_unavailable.is_some()
                    || json.n().sample_skipped
                {
                    if !retain(&mut self.contexts, ContextCandidate::Unavailable, budget) {
                        return false;
                    }
                    continue;
                }
                match crate::sample::alternatives(json, budget) {
                    Field::Present(rows) => {
                        let mut seen = false;
                        for (_, branch) in rows {
                            if !charge(&mut budget.work, 1) {
                                return false;
                            }
                            seen = true;
                            match branch {
                                Field::Present(branch) => {
                                    if !retain(&mut self.pending, branch.json(), budget) {
                                        return false;
                                    }
                                }
                                _ => {
                                    if !retain(
                                        &mut self.contexts,
                                        ContextCandidate::Unavailable,
                                        budget,
                                    ) {
                                        return false;
                                    }
                                }
                            }
                        }
                        if !seen
                            && !retain(&mut self.contexts, ContextCandidate::Unavailable, budget)
                        {
                            return false;
                        }
                    }
                    Field::Absent => {
                        if !retain(
                            &mut self.contexts,
                            ContextCandidate::Description(json),
                            budget,
                        ) {
                            return false;
                        }
                    }
                    Field::Unavailable(_) => {
                        if !retain(&mut self.contexts, ContextCandidate::Unavailable, budget) {
                            return false;
                        }
                    }
                }
            }
            true
        }
        fn ordinary(
            &self,
            candidates: Range<usize>,
            left: Operand<'a>,
            right: Operand<'a>,
            budget: &mut OperationBudget,
        ) -> bool {
            self.context(candidates, left, right, budget) == Context::Ordinary
        }
        // Constant seeds reuse this result by `operand_slot`, so it reads only the
        // operands' kinds and availability. A reused result also skips the extension
        // vocabulary that `description_ordinary` would note in `budget.set_aside` again,
        // which is correct only because one `EqualityWork` serves one budget, so the first
        // classification already noted it there.
        fn context(
            &self,
            candidates: Range<usize>,
            left: Operand<'a>,
            right: Operand<'a>,
            budget: &mut OperationBudget,
        ) -> Context {
            let mut common = None;
            let mut known = true;
            for index in candidates {
                if !charge(&mut budget.work, 1) {
                    return Context::Unavailable;
                }
                let meaning = match self.contexts[index] {
                    ContextCandidate::Ordinary => Context::Ordinary,
                    ContextCandidate::Unavailable => Context::Unavailable,
                    ContextCandidate::Description(json) => {
                        match crate::sample::scalar_format_context(json, budget) {
                            Context::Temporal(_) | Context::Resource(_)
                                if (left.kind() == JsonKind::Null
                                    || right.kind() == JsonKind::Null)
                                    && !json.n().role.is_some_and(crate::role::Role::full_data) =>
                            {
                                Context::Unavailable
                            }
                            context @ (Context::Bytes | Context::Unix) => {
                                // A fully classified conventional channel excludes a
                                // nonnull operand of the other JSON kind. Unknown
                                // candidate meaning never reaches this branch.
                                let kind = if context == Context::Bytes {
                                    JsonKind::String
                                } else {
                                    JsonKind::Number
                                };
                                if left.available()
                                    && right.available()
                                    && (left.kind() != kind && left.kind() != JsonKind::Null
                                        || right.kind() != kind && right.kind() != JsonKind::Null)
                                {
                                    continue;
                                }
                                context
                            }
                            Context::Temporal(format)
                                if left.kind() == JsonKind::Null
                                    || right.kind() == JsonKind::Null
                                    || left.kind() == JsonKind::String
                                        && right.kind() == JsonKind::String =>
                            {
                                Context::Temporal(format)
                            }
                            Context::Resource(format)
                                if left.kind() == JsonKind::Null
                                    || right.kind() == JsonKind::Null
                                    || left.kind() == JsonKind::String
                                        && right.kind() == JsonKind::String =>
                            {
                                Context::Resource(format)
                            }
                            _ if description_ordinary(json, left, right, budget) => {
                                Context::Ordinary
                            }
                            _ => Context::Unavailable,
                        }
                    }
                };
                known &= meaning != Context::Unavailable
                    && common.is_none_or(|previous| previous == meaning);
                common = Some(meaning);
            }
            if known {
                common.unwrap_or(Context::Unavailable)
            } else {
                Context::Unavailable
            }
        }
        // Append child candidates; numerical parent ranges remain valid across growth.
        fn project(
            &mut self,
            candidates: Range<usize>,
            key: Option<&str>,
            budget: &mut OperationBudget,
        ) -> Option<Range<usize>> {
            let start = self.contexts.len();
            for index in candidates {
                if !charge(&mut budget.work, 1) {
                    return None;
                }
                match self.contexts[index] {
                    ContextCandidate::Description(json) => {
                        let child = match key {
                            None => json.lookup("items", budget),
                            Some(key) => match json.lookup("properties", budget) {
                                Field::Absent => Field::Absent,
                                Field::Present(properties)
                                    if crate::sample::description_members_known(
                                        properties,
                                        &mut budget.work,
                                    ) =>
                                {
                                    properties.lookup(key, budget)
                                }
                                _ => Field::Unavailable("equality property context unavailable"),
                            },
                        };
                        match child {
                            Field::Present(child) => {
                                if !self.expand(child, budget) {
                                    return None;
                                }
                            }
                            Field::Absent => {
                                if !retain(&mut self.contexts, ContextCandidate::Ordinary, budget) {
                                    return None;
                                }
                            }
                            Field::Unavailable(_) => {
                                if !retain(
                                    &mut self.contexts,
                                    ContextCandidate::Unavailable,
                                    budget,
                                ) {
                                    return None;
                                }
                            }
                        }
                    }
                    candidate => {
                        if !retain(&mut self.contexts, candidate, budget) {
                            return None;
                        }
                    }
                }
            }
            Some(start..self.contexts.len())
        }
        fn compare(
            &mut self,
            left: Operand<'a>,
            right: Operand<'a>,
            candidates: Range<usize>,
            seeded: bool,
            work: &mut NumberWork,
            budget: &mut OperationBudget,
        ) -> Decision<'a> {
            if !retain(
                &mut self.frames,
                EqualityFrame {
                    left,
                    right,
                    candidates,
                    phase: Phase::Start,
                    unknown: false,
                    ancestors: 0,
                },
                budget,
            ) {
                return Decision::unavailable(left, right);
            }
            while let Some(mut frame) = self.frames.pop() {
                if !charge(&mut budget.work, 1) {
                    return Decision::unavailable(left, right);
                }
                match &mut frame.phase {
                    Phase::Start => {
                        // Only the root frame starts with an empty stack.
                        let meaning = if seeded && self.frames.is_empty() {
                            self.seed_meaning(
                                frame.candidates.clone(),
                                frame.left,
                                frame.right,
                                budget,
                            )
                        } else {
                            self.context(frame.candidates.clone(), frame.left, frame.right, budget)
                        };
                        if matches!(meaning, Context::Bytes | Context::Unix) {
                            let (verdict, limitations, supporting) =
                                crate::sample::abstract_value::compare(
                                    meaning,
                                    frame.left,
                                    frame.right,
                                    work,
                                    budget,
                                    &mut self.abstract_use,
                                );
                            self.supporting |= supporting;
                            self.limitations |= limitations;
                            match verdict {
                                Verdict::Equal => {}
                                Verdict::Unavailable => frame.unknown = true,
                                Verdict::Different => {
                                    self.supporting = frame.ancestors | supporting;
                                    return Decision {
                                        verdict,
                                        left: frame.left,
                                        right: frame.right,
                                    };
                                }
                            }
                        } else if matches!(meaning, Context::Temporal(_) | Context::Resource(_))
                            && (frame.left.kind() == JsonKind::Null
                                || frame.right.kind() == JsonKind::Null)
                        {
                            let (verdict, supporting) =
                                self.formatted_null(meaning, frame.left, frame.right, budget);
                            self.supporting |= supporting;
                            match verdict {
                                Verdict::Equal => {}
                                Verdict::Unavailable => frame.unknown = true,
                                Verdict::Different => {
                                    self.supporting = frame.ancestors | supporting;
                                    return Decision {
                                        verdict,
                                        left: frame.left,
                                        right: frame.right,
                                    };
                                }
                            }
                        } else if let Context::Temporal(format) = meaning {
                            crate::civil::begin_attempt(&mut self.temporal_use);
                            if let (Field::Present(a), Field::Present(b)) =
                                (frame.left.text(), frame.right.text())
                            {
                                let mut established = true;
                                for text in [a, b] {
                                    match self.temporal_established(format, text, budget) {
                                        Some(denotes) => established &= denotes,
                                        None => {
                                            established = false;
                                            break;
                                        }
                                    }
                                }
                                if established {
                                    self.supporting |= Basis::Temporal.support_mask();
                                }
                                frame.unknown = if !established {
                                    true
                                } else if !a
                                    .len()
                                    .checked_add(b.len())
                                    .and_then(|n| {
                                        if format == TemporalFormat::DateTime {
                                            n.checked_mul(2)?.checked_add(256)
                                        } else {
                                            n.checked_add(1)
                                        }
                                    })
                                    .is_some_and(|cost| charge(&mut budget.work, cost))
                                {
                                    self.limitations |= 1 << 3;
                                    true
                                } else if if format == TemporalFormat::DateTime {
                                    !crate::civil::same_dated_instant(a, b)
                                } else {
                                    a != b
                                } {
                                    if matches!(
                                        format,
                                        TemporalFormat::Date | TemporalFormat::DateTime
                                    ) {
                                        self.supporting =
                                            frame.ancestors | Basis::Temporal.support_mask();
                                        return Decision {
                                            verdict: Verdict::Different,
                                            left: frame.left,
                                            right: frame.right,
                                        };
                                    }
                                    self.limitations |= 1 << 7;
                                    true
                                } else {
                                    false
                                };
                            } else {
                                frame.unknown = true;
                            }
                        } else if let Context::Resource(format) = meaning {
                            if let (Field::Present(a), Field::Present(b)) =
                                (frame.left.text(), frame.right.text())
                            {
                                let (verdict, limitations) = crate::formats::compare_meaning(
                                    format,
                                    a,
                                    b,
                                    &mut budget.work,
                                    &mut self.format_use,
                                );
                                self.limitations |= limitations;
                                match verdict {
                                    Verdict::Equal => {
                                        self.supporting |= resource_basis(format).support_mask();
                                    }
                                    Verdict::Unavailable => frame.unknown = true,
                                    Verdict::Different => {
                                        self.supporting =
                                            frame.ancestors | resource_basis(format).support_mask();
                                        return Decision {
                                            verdict: Verdict::Different,
                                            left: frame.left,
                                            right: frame.right,
                                        };
                                    }
                                }
                            } else {
                                frame.unknown = true;
                            }
                        } else if !frame.left.available()
                            || !frame.right.available()
                            || meaning == Context::Unavailable
                        {
                            frame.unknown = true;
                        } else if frame.left.kind() != frame.right.kind() {
                            self.supporting = frame.ancestors | Basis::TypeNull.support_mask();
                            return Decision {
                                verdict: Verdict::Different,
                                left: frame.left,
                                right: frame.right,
                            };
                        } else {
                            match frame.left.kind() {
                                JsonKind::Array => {
                                    self.supporting |= Basis::Array.support_mask();
                                    frame.ancestors |= Basis::Array.support_mask();
                                    frame.phase = Phase::Array {
                                        left: frame.left.children(),
                                        right: frame.right.children(),
                                    }
                                }
                                JsonKind::Map => match (
                                    frame.left.map_count(budget),
                                    frame.right.map_count(budget),
                                ) {
                                    (Some(a), Some(b)) if a != b => {
                                        self.supporting =
                                            frame.ancestors | Basis::Map.support_mask();
                                        return Decision {
                                            verdict: Verdict::Different,
                                            left: frame.left,
                                            right: frame.right,
                                        };
                                    }
                                    (Some(_), Some(_)) => {
                                        self.supporting |= Basis::Map.support_mask();
                                        frame.ancestors |= Basis::Map.support_mask();
                                        frame.phase = Phase::Map {
                                            left: frame.left.children(),
                                        }
                                    }
                                    _ => frame.unknown = true,
                                },
                                _ => match primitive(
                                    frame.left,
                                    frame.right,
                                    work,
                                    budget,
                                    &mut self.limitations,
                                ) {
                                    Verdict::Different => {
                                        self.supporting = frame.ancestors
                                            | primitive_basis(frame.left.kind()).support_mask();
                                        return Decision {
                                            verdict: Verdict::Different,
                                            left: frame.left,
                                            right: frame.right,
                                        };
                                    }
                                    Verdict::Unavailable => frame.unknown = true,
                                    Verdict::Equal => {
                                        self.supporting |=
                                            primitive_basis(frame.left.kind()).support_mask();
                                    }
                                },
                            }
                            if !matches!(frame.phase, Phase::Start) {
                                if !retain(&mut self.frames, frame, budget) {
                                    return Decision::unavailable(left, right);
                                }
                                continue;
                            }
                        }
                    }
                    Phase::Array { left: a, right: b } => match (a.next(), b.next()) {
                        (None, None) => {}
                        (Some(a), Some(b)) => {
                            let Some(candidates) =
                                self.project(frame.candidates.clone(), None, budget)
                            else {
                                return Decision::unavailable(left, right);
                            };
                            let ancestors = frame.ancestors;
                            if !retain(&mut self.frames, frame, budget)
                                || !retain(
                                    &mut self.frames,
                                    EqualityFrame {
                                        left: a,
                                        right: b,
                                        candidates,
                                        phase: Phase::Start,
                                        unknown: false,
                                        ancestors,
                                    },
                                    budget,
                                )
                            {
                                return Decision::unavailable(left, right);
                            }
                            continue;
                        }
                        _ => {
                            self.supporting = frame.ancestors | Basis::Array.support_mask();
                            return Decision {
                                verdict: Verdict::Different,
                                left: frame.left,
                                right: frame.right,
                            };
                        }
                    },
                    Phase::Map { left: values } => {
                        if let Some(a) = values.next() {
                            let Field::Present(key) = a.member_name() else {
                                return Decision::unavailable(left, right);
                            };
                            match frame.right.member(key, &mut budget.work) {
                                Field::Absent => {
                                    self.supporting = frame.ancestors | Basis::Map.support_mask();
                                    return Decision {
                                        verdict: Verdict::Different,
                                        left: frame.left,
                                        right: frame.right,
                                    };
                                }
                                Field::Present(b) => {
                                    let Some(candidates) =
                                        self.project(frame.candidates.clone(), Some(key), budget)
                                    else {
                                        return Decision::unavailable(left, right);
                                    };
                                    let ancestors = frame.ancestors;
                                    if !retain(&mut self.frames, frame, budget)
                                        || !retain(
                                            &mut self.frames,
                                            EqualityFrame {
                                                left: a,
                                                right: b,
                                                candidates,
                                                phase: Phase::Start,
                                                unknown: false,
                                                ancestors,
                                            },
                                            budget,
                                        )
                                    {
                                        return Decision::unavailable(left, right);
                                    }
                                    continue;
                                }
                                Field::Unavailable(_) => {
                                    frame.unknown = true;
                                    if !retain(&mut self.frames, frame, budget) {
                                        return Decision::unavailable(left, right);
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                }
                if let Some(parent) = self.frames.last_mut() {
                    parent.unknown |= frame.unknown;
                    // Root candidates stay for a seeded context; finishing the pair drops the rest.
                    self.contexts.truncate(frame.candidates.start);
                } else {
                    return Decision {
                        verdict: if frame.unknown {
                            Verdict::Unavailable
                        } else {
                            Verdict::Equal
                        },
                        left,
                        right,
                    };
                }
            }
            Decision::unavailable(left, right)
        }
        fn temporal_established(
            &mut self,
            format: TemporalFormat,
            text: &str,
            budget: &mut OperationBudget,
        ) -> Option<bool> {
            use crate::civil::{Establishment, Unavailable};
            match crate::civil::establish(
                format,
                Field::Present(text),
                &mut budget.work,
                &mut self.temporal_use,
            ) {
                Establishment::Established => Some(true),
                Establishment::Invalid(_) => {
                    self.limitations |= 1 << 2;
                    Some(false)
                }
                Establishment::Unavailable(cause) => {
                    self.limitations |= match cause {
                        Unavailable::Input => 1,
                        Unavailable::Work => 1 << 3,
                        Unavailable::Knowledge => 1 << 6,
                        Unavailable::Meaning => 1 << 7,
                    };
                    if matches!(cause, Unavailable::Work) {
                        None
                    } else {
                        Some(false)
                    }
                }
            }
        }
        fn formatted_null(
            &mut self,
            meaning: Context,
            left: Operand<'a>,
            right: Operand<'a>,
            budget: &mut OperationBudget,
        ) -> (Verdict, u32) {
            let supporting = Basis::TypeNull.support_mask();
            if !left.available() || !right.available() {
                self.limitations |= 1;
                return (Verdict::Unavailable, 0);
            }
            if left.kind() == JsonKind::Null && right.kind() == JsonKind::Null {
                return (Verdict::Equal, supporting);
            }
            let value = if left.kind() == JsonKind::Null {
                right
            } else {
                left
            };
            if value.kind() != JsonKind::String {
                self.limitations |= 1 << 2;
                return (Verdict::Unavailable, supporting);
            }
            let Field::Present(text) = value.text() else {
                self.limitations |= 1;
                return (Verdict::Unavailable, supporting);
            };
            let basis = match meaning {
                Context::Temporal(format) => {
                    if self.temporal_established(format, text, budget) != Some(true) {
                        return (Verdict::Unavailable, supporting);
                    }
                    Basis::Temporal
                }
                Context::Resource(format) => {
                    let limitation = match crate::formats::evaluate(
                        format,
                        text,
                        &mut budget.work,
                        &mut self.format_use,
                    ) {
                        crate::formats::Evaluation::Established(_) => 0,
                        crate::formats::Evaluation::WorkUnavailable => 1 << 3,
                        crate::formats::Evaluation::MeaningUnavailable => 1 << 8,
                        _ => 1 << 2,
                    };
                    self.limitations |= limitation;
                    if limitation != 0 {
                        return (Verdict::Unavailable, supporting);
                    }
                    resource_basis(format)
                }
                _ => unreachable!("formatted null context"),
            };
            (Verdict::Different, supporting | basis.support_mask())
        }
    }
    fn same_json(a: EffectiveJson<'_>, b: EffectiveJson<'_>) -> bool {
        std::ptr::eq(a.model, b.model) && a.node == b.node
    }
    fn same_operand(a: Operand<'_>, b: Operand<'_>) -> bool {
        match (a, b) {
            // Source identities repeat across catalogues, so the text location tells them apart.
            (Operand::Sample(a), Operand::Sample(b)) => {
                a.source_id() == b.source_id()
                    && a.node() == b.node()
                    && std::ptr::eq(a.raw_json(), b.raw_json())
            }
            (Operand::Model(a), Operand::Model(b))
            | (Operand::Constraint(a), Operand::Constraint(b)) => same_json(a, b),
            _ => false,
        }
    }
    /// The operand properties that context classification reads.
    /// One slot for each JSON kind, available or not, as `operand_slot` numbers them.
    const OPERAND_SLOTS: usize = 2 * 6;
    fn operand_slot(operand: Operand<'_>) -> usize {
        let kind = match operand.kind() {
            JsonKind::Null => 0,
            JsonKind::Boolean => 1,
            JsonKind::Number => 2,
            JsonKind::String => 3,
            JsonKind::Array => 4,
            JsonKind::Map => 5,
        };
        2 * kind + usize::from(operand.available())
    }
    fn resource_basis(format: ResourceFormat) -> Basis {
        if format == ResourceFormat::Uuid {
            Basis::UuidValue
        } else {
            Basis::Resource
        }
    }
    fn primitive_basis(kind: JsonKind) -> Basis {
        match kind {
            JsonKind::Number => Basis::Numeric,
            JsonKind::String => Basis::Text,
            JsonKind::Boolean => Basis::Boolean,
            _ => Basis::TypeNull,
        }
    }
    // Local structure is independent of descendant capability/completion.
    fn description_ordinary(
        json: EffectiveJson<'_>,
        left: Operand<'_>,
        right: Operand<'_>,
        budget: &mut OperationBudget,
    ) -> bool {
        if json.kind() != JsonKind::Map
            || json.n().unavailable.is_some()
            || json.n().meaning_unavailable.is_some()
            || json.n().donor_unavailable.is_some()
            || json.n().sample_skipped
        {
            return false;
        }
        let Some(role) = json.n().role.filter(|role| role.data_description_role()) else {
            return false;
        };
        let mut set_aside = crate::role::SetAside::default();
        let kind = match json.lookup("type", budget) {
            Field::Absent => None,
            Field::Present(value) => match value.text() {
                Field::Present(
                    kind @ ("number" | "integer" | "string" | "boolean" | "array" | "object"),
                ) => Some(kind),
                _ => return false,
            },
            _ => return false,
        };
        if role == crate::role::Role::DirectItem
            && (kind == Some("array")
                || left.kind() == JsonKind::Null
                || right.kind() == JsonKind::Null)
        {
            return false;
        }
        for child in json.children() {
            let Some(key) = child.n().key else {
                return false;
            };
            if !charge(&mut budget.work, 64 * (key.len() + 1)) {
                return false;
            }
            let vocabulary = set_aside.classify(role, key, kind == Some("object"));
            if vocabulary.blocks_base_meaning() {
                return false;
            }
            if vocabulary.ignorable() {
                continue;
            }
            match key {
                "items" if kind != Some("array") => return false,
                "properties" if kind != Some("object") => return false,
                "items" | "properties" => {}
                "sdfType" | "contentFormat" | "sdfRef" => return false,
                "format" => {
                    if kind != Some("string")
                        || child.n().donor_unavailable.is_some()
                        || !matches!(
                            child.text(),
                            Field::Present(
                                "date-time" | "date" | "time" | "uri" | "uri-reference" | "uuid"
                            )
                        )
                        || matches!(left.kind(), JsonKind::String | JsonKind::Null)
                            && matches!(right.kind(), JsonKind::String | JsonKind::Null)
                    {
                        return false;
                    }
                }
                "description" | "label" | "$comment" | "default" | "unit" | "readable"
                | "writable" | "observable" | "sdfRequired" | "nullable" | "const" | "enum"
                | "sdfChoice" | "type" | "minimum" | "maximum" | "exclusiveMinimum"
                | "exclusiveMaximum" | "multipleOf" | "minLength" | "maxLength" | "pattern"
                | "minItems" | "maxItems" | "uniqueItems" | "required" => {
                    if child.n().donor_unavailable.is_some() {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        budget.set_aside |= set_aside;
        true
    }
    fn primitive(
        left: Operand<'_>,
        right: Operand<'_>,
        work: &mut NumberWork,
        budget: &mut OperationBudget,
        limitations: &mut u16,
    ) -> Verdict {
        let equal = match left.kind() {
            JsonKind::Null => true,
            JsonKind::Boolean => match (left.boolean(), right.boolean()) {
                (Field::Present(a), Field::Present(b)) => a == b,
                _ => return Verdict::Unavailable,
            },
            JsonKind::String => match (left.text(), right.text()) {
                (Field::Present(a), Field::Present(b)) => {
                    if !charge(&mut budget.work, a.len() + b.len() + 1) {
                        return Verdict::Unavailable;
                    }
                    a == b
                }
                _ => return Verdict::Unavailable,
            },
            JsonKind::Number => match (left.number(), right.number()) {
                (Field::Present(a), Field::Present(b)) => match a.compare_with_work(b, work) {
                    Ok(order) => order.is_eq(),
                    Err(_) => {
                        *limitations |= 1 << 4;
                        return Verdict::Unavailable;
                    }
                },
                _ => return Verdict::Unavailable,
            },
            _ => return Verdict::Unavailable,
        };
        if equal {
            Verdict::Equal
        } else {
            Verdict::Different
        }
    }
    fn retain<T>(values: &mut Vec<T>, value: T, budget: &mut OperationBudget) -> bool {
        if values.len() == values.capacity() {
            let old = values.capacity();
            let Some(new) = old.max(4).checked_mul(2) else {
                return false;
            };
            let Some(bytes) = new.checked_mul(std::mem::size_of::<T>()) else {
                return false;
            };
            if !charge(&mut budget.allocation, bytes) {
                return false;
            }
            values.reserve_exact(new - old);
        }
        values.push(value);
        true
    }
    pub(crate) fn fixed_headers() -> usize {
        use std::mem::size_of;
        let common = size_of::<EqualityWork<'_>>()
            + size_of::<PreparedItems<'_>>()
            + size_of::<Vec<ContextCandidate<'_>>>()
            + size_of::<Range<usize>>()
            + size_of::<ContextCandidate<'_>>()
            + size_of::<std::slice::Iter<'_, ContextCandidate<'_>>>()
            + size_of::<&mut PreparedItems<'_>>()
            + size_of::<EqualityFrame<'_>>()
            + size_of::<ContextCandidate<'_>>()
            + 2 * size_of::<Operand<'_>>()
            + 2 * size_of::<OperandChildren<'_>>()
            + size_of::<Range<usize>>()
            + size_of::<Decision<'_>>()
            + 2 * size_of::<usize>();
        let projection = size_of::<ProjectionCursor<'_>>()
            + 2 * size_of::<Operand<'_>>()
            + size_of::<ValueContext<'_>>()
            + size_of::<EffectiveJson<'_>>()
            + size_of::<Range<usize>>()
            + size_of::<Field<&str>>();
        // Pay both text Fields in the scalar phase without a projection credit.
        let scalar_caller = size_of::<Context>()
            + 2 * size_of::<Field<&str>>()
            + 2 * size_of::<&str>()
            + size_of::<ResourceFormat>()
            + size_of::<(Verdict, u16)>()
            + size_of::<Verdict>()
            + size_of::<u16>()
            + 2 * size_of::<bool>()
            + size_of::<&mut EqualityWork<'_>>()
            + size_of::<&mut NumberWork>()
            + size_of::<&mut OperationBudget>();
        // Conservative temporal caller copies, including both the source
        // array and IntoIter, are kept in the same scalar phase bound. Some
        // already appear in civil::fixed_headers; no credit is taken here.
        let temporal_caller = scalar_caller
            + size_of::<[&str; 2]>()
            + size_of::<std::array::IntoIter<&str, 2>>()
            + size_of::<&str>()
            + size_of::<Option<usize>>()
            + size_of::<usize>()
            + size_of::<Option<bool>>()
            + size_of::<crate::civil::Establishment>()
            + 2 * size_of::<TemporalFormat>()
            + size_of::<&str>()
            + size_of::<&mut EqualityWork<'_>>()
            + size_of::<&mut OperationBudget>();
        // The null helper copies both operands and selects one nonnull value.
        // Its caller, support record and scanner result remain funded through
        // denotation evaluation; scanner storage has its own containing owner.
        let formatted_null_caller = temporal_caller
            + 3 * size_of::<Operand<'_>>()
            + size_of::<Context>()
            + size_of::<Field<&str>>()
            + size_of::<Basis>()
            + size_of::<crate::formats::Evaluation<'_>>()
            + size_of::<(Verdict, u32)>()
            + 2 * size_of::<u32>()
            + size_of::<&mut EqualityWork<'_>>()
            + size_of::<&mut OperationBudget>();
        // The ancestry copy survives moving the parent into retain. Pay the
        // extracted operands, returned range, caller/reborrow references and
        // retain's checked growth temporaries in this separate child phase.
        let child_common = 2 * size_of::<Operand<'_>>()
            + size_of::<Range<usize>>()
            + size_of::<u32>()
            + 4 * size_of::<usize>()
            + size_of::<Option<usize>>()
            + 2 * size_of::<bool>()
            + size_of::<&mut EqualityWork<'_>>()
            + size_of::<&mut NumberWork>()
            + 2 * size_of::<&mut OperationBudget>()
            + size_of::<&mut Vec<EqualityFrame<'_>>>();
        let array_child = child_common
            + 2 * size_of::<Option<Operand<'_>>>()
            + 2 * size_of::<&mut OperandChildren<'_>>();
        let map_child = child_common
            + size_of::<Option<Operand<'_>>>()
            + size_of::<Field<Operand<'_>>>()
            + size_of::<&str>()
            + size_of::<&mut OperandChildren<'_>>();
        // Seeding's ProjectionCursor returns before compare. Classification,
        // expansion and projection do not evaluate denotations; Array/Map
        // projection completes before the next Start, where scalar helpers run.
        // Renew this maximum if a callback or retained cursor changes that law.
        // Vec backings and full NEW growth remain separately funded.
        // Child construction starts after projection returns; the moved parent
        // and later child reuse the common transient Frame header. Their Vec
        // backing, including full NEW growth, is separately paid before reserve.
        common
            + projection.max(
                scalar_caller
                    .max(temporal_caller)
                    .max(formatted_null_caller)
                    .max(array_child.max(map_child)),
            )
    }
}
pub(crate) use comparison::{EqualityWork, PreparedItems, fixed_headers};

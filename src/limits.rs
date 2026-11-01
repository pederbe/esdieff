//! Finite ceilings keep source acquisition, decoding and report presentation predictable.
//! Follows ADR0009: Bounded processing and reporting, and
//! ADR0013: Shared workflow and report guarantees.
//! Pattern reducers follow ADR0028: Bounded effective model pattern syntax.

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Limits {
    pub(crate) document_bytes: usize,
    pub(crate) catalogue_bytes: usize,
    pub(crate) documents: usize,
    pub(crate) depth: usize,
    pub(crate) document_nodes: usize,
    pub(crate) nodes: usize,
    pub(crate) allocation: usize,
    pub(crate) work: usize,
    pub(crate) number_work: usize,
    pub(crate) number_token: usize,
    pub(crate) coefficient: usize,
    pub(crate) exponent: i64,
    pub(crate) conditions: usize,
    pub(crate) detail_bytes: usize,
    pub(crate) report_bytes: usize,
    pub(crate) pointer_bytes: usize,
    pub(crate) pointer_total: usize,
    pub(crate) effective_nodes: usize,
    pub(crate) effective_allocation: usize,
    pub(crate) references: usize,
    pub(crate) reference_depth: usize,
    pub(crate) pattern_bytes: usize,
    pub(crate) pattern_nesting: usize,
    pub(crate) pattern_alternatives: usize,
    pub(crate) pattern_subject: usize,
    pub(crate) equality_pairs: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            document_bytes: 4 * 1024 * 1024,
            catalogue_bytes: 32 * 1024 * 1024,
            documents: 256,
            depth: 64,
            document_nodes: 100_000,
            nodes: 500_000,
            allocation: 64 * 1024 * 1024,
            work: 10_000_000,
            number_work: 2_000_000,
            number_token: 1024,
            coefficient: 512,
            exponent: 1_000_000,
            conditions: 1000,
            detail_bytes: 1024 * 1024,
            report_bytes: 8 * 1024 * 1024,
            pointer_bytes: 8192,
            pointer_total: 8 * 1024 * 1024,
            effective_nodes: 250_000,
            effective_allocation: 64 * 1024 * 1024,
            references: 100_000,
            reference_depth: 256,
            pattern_bytes: 4096,
            pattern_nesting: 64,
            pattern_alternatives: 512,
            pattern_subject: 1024 * 1024,
            equality_pairs: 100_000,
        }
    }
}
impl Limits {
    #[cfg(test)]
    pub(crate) fn document_bytes(mut self, value: usize) -> Self {
        self.document_bytes = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn catalogue_bytes(mut self, value: usize) -> Self {
        self.catalogue_bytes = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn documents(mut self, value: usize) -> Self {
        self.documents = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn depth(mut self, value: usize) -> Self {
        self.depth = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn document_nodes(mut self, value: usize) -> Self {
        self.document_nodes = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn parsed_nodes(mut self, value: usize) -> Self {
        self.nodes = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn parsed_allocation_bytes(mut self, value: usize) -> Self {
        self.allocation = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn decoding_work(mut self, value: usize) -> Self {
        self.work = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn numeric_work(mut self, value: usize) -> Self {
        self.number_work = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn number_token_bytes(mut self, value: usize) -> Self {
        self.number_token = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn coefficient_digits(mut self, value: usize) -> Self {
        self.coefficient = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn exponent_magnitude(mut self, value: i64) -> Self {
        self.exponent = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn conditions(mut self, value: usize) -> Self {
        self.conditions = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn diagnostic_bytes(mut self, value: usize) -> Self {
        self.detail_bytes = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn report_bytes(mut self, value: usize) -> Self {
        self.report_bytes = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn pointer_bytes(mut self, value: usize) -> Self {
        self.pointer_bytes = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn pointer_output_bytes(mut self, value: usize) -> Self {
        self.pointer_total = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn effective_nodes(mut self, value: usize) -> Self {
        self.effective_nodes = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn effective_allocation_bytes(mut self, value: usize) -> Self {
        self.effective_allocation = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn reference_applications(mut self, value: usize) -> Self {
        self.references = value;
        self
    }
    #[cfg(test)]
    pub(crate) fn reference_depth(mut self, value: usize) -> Self {
        self.reference_depth = value;
        self
    }
    /// Reduce the UTF-8 byte ceiling for an effective pattern string.
    #[cfg(test)]
    pub(crate) fn pattern_bytes(mut self, value: usize) -> Self {
        self.pattern_bytes = value;
        self
    }
    /// Reduce the group nesting ceiling for pattern syntax.
    #[cfg(test)]
    pub(crate) fn pattern_nesting(mut self, value: usize) -> Self {
        self.pattern_nesting = value;
        self
    }
    /// Reduce the total alternative ceiling for pattern syntax.
    #[cfg(test)]
    pub(crate) fn pattern_alternatives(mut self, value: usize) -> Self {
        self.pattern_alternatives = value;
        self
    }
    /// Reduce the UTF-8 subject byte ceiling for each pattern match.
    #[cfg(test)]
    pub(crate) fn pattern_subject_bytes(mut self, value: usize) -> Self {
        self.pattern_subject = value;
        self
    }
    /// Reduce the request-wide ceiling for constant and uniqueness comparisons.
    #[cfg(test)]
    pub(crate) fn equality_pairs(mut self, value: usize) -> Self {
        self.equality_pairs = value;
        self
    }
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if std::mem::size_of::<serde_json::Value>() > 128
            || std::mem::size_of::<serde_json::Map<String, serde_json::Value>>() > 256
        {
            return Err("JSON metadata backend exceeds supported allocation accounting");
        }
        let counts = [
            self.document_bytes,
            self.catalogue_bytes,
            self.documents,
            self.depth,
            self.document_nodes,
            self.nodes,
            self.allocation,
            self.work,
            self.number_work,
            self.number_token,
            self.coefficient,
            self.conditions,
            self.detail_bytes,
            self.report_bytes,
            self.pointer_bytes,
            self.pointer_total,
            self.effective_nodes,
            self.effective_allocation,
            self.references,
            self.reference_depth,
            self.pattern_bytes,
            self.pattern_nesting,
            self.pattern_alternatives,
            self.pattern_subject,
            self.equality_pairs,
        ];
        if counts.iter().any(|n| *n == 0 || *n > u32::MAX as usize)
            || self.documents > 256
            || self.depth > 64
            || self.exponent <= 0
            || self.exponent > 1_000_000
            || self.number_token > 1024
            || self.coefficient > 512
            || self.pointer_bytes > 8192
            || self.reference_depth > 256
            || self.pattern_bytes > 4096
            || self.pattern_nesting > 64
            || self.pattern_alternatives > 512
            || self.pattern_subject > 1024 * 1024
            || self.equality_pairs > 100_000
        {
            return Err("configuration exceeds supported finite ceilings");
        }
        if self.nodes > 1_000_000
            || self.effective_nodes > 1_000_000
            || self.effective_allocation > 256 * 1024 * 1024
            || self.references > 1_000_000
            || self.document_nodes > 500_000
            || self.document_bytes > 16 * 1024 * 1024
            || self.catalogue_bytes > 128 * 1024 * 1024
            || self.allocation > 256 * 1024 * 1024
            || self.work > 100_000_000
            || self.number_work > 100_000_000
            || self.conditions > 10_000
            || self.detail_bytes > 8 * 1024 * 1024
            || self.report_bytes > 32 * 1024 * 1024
            || self.pointer_total > 32 * 1024 * 1024
        {
            return Err("configuration exceeds supported storage or count ceilings");
        }
        Ok(())
    }
}

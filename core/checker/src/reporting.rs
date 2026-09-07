//! Ordered semantic findings and non-cascading recovery outcomes.
//!
//! A check appends a root problem with [`Findings::report`] and carries the
//! returned [`Reported`] proof through dependent analysis. Independent checks
//! use the same collector and remain visible in insertion order. The
//! collector has no sorting, grouping, or deduplication step: callers establish
//! deterministic order by running checks and visiting their source facts in
//! deterministic order.

/// Opaque proof that a root finding has already been reported.
///
/// The private field prevents callers from manufacturing proof from ordinary
/// public data. The proof is copyable because one root finding can invalidate
/// more than one dependent analysis path without emitting another finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reported {
    _private: (),
}

/// The result of resolving one semantic input.
///
/// `Unknown` means that analysis has not established a useful answer and does
/// not promise that a finding exists. `Invalid` carries proof that its root
/// finding was reported, allowing dependent analysis to propagate the outcome
/// without producing a cascade. Neither recovery outcome is a semantic type;
/// only `Resolved` can be stored in a valid semantic artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeResolution<T> {
    Resolved(T),
    Unknown,
    Invalid(Reported),
}

/// Phase-local findings accumulated in deterministic FIFO order.
///
/// [`Findings::report`] records a generation-blocking root finding and returns
/// its opaque proof. [`Findings::report_warning`] records an independent
/// non-blocking finding. Repeated values are retained; the collector does not
/// infer ownership or deduplicate reports.
#[derive(Debug, PartialEq, Eq)]
pub struct Findings<F> {
    values: Vec<F>,
    generation_blocked: bool,
}

impl<F> Findings<F> {
    /// Creates an empty finding collection whose generation gate is open.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            generation_blocked: false,
        }
    }

    /// Records a generation-blocking root finding and returns its proof.
    pub fn report(&mut self, finding: F) -> Reported {
        self.values.push(finding);
        self.generation_blocked = true;
        Reported { _private: () }
    }

    /// Records an independent finding that does not block generation.
    pub fn report_warning(&mut self, finding: F) {
        self.values.push(finding);
    }

    /// Returns findings in the order in which they were reported.
    #[must_use]
    pub fn as_slice(&self) -> &[F] {
        &self.values
    }

    /// Returns whether generation may proceed from this collection.
    #[must_use]
    pub const fn generation_allowed(&self) -> bool {
        !self.generation_blocked
    }
}

impl<F> Default for Findings<F> {
    fn default() -> Self {
        Self::new()
    }
}
